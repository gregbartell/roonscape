import type { DiscoverableTrackedOutput } from "./display-configuration-command.js";
import type { DisplayConfiguration } from "./display-configuration.js";

export type SetupKey = "up" | "down" | "enter" | "retry" | "quit";

export interface FirstTimeSetupDependencies {
  authorizationFile(): string;
  configurationFileExists(configurationFile: string): boolean;
  discoverTrackedOutputs(
    authorizationFile: string,
    signal: AbortSignal,
  ): Promise<DiscoverableTrackedOutput[]>;
  readSetupKey(signal: AbortSignal): Promise<SetupKey>;
  saveConfiguration(
    configurationFile: string,
    configuration: DisplayConfiguration,
  ): void;
  delay(milliseconds: number): Promise<void>;
  writeOutput(line: string): void;
  writeError(line: string): void;
}

const defaultInactivity = {
  gracePeriodSeconds: 300,
  dimmedOpacity: 0.35,
  repositionCadenceSeconds: 60,
} as const;

export async function runFirstTimeSetup(
  configurationFile: string,
  dependencies: FirstTimeSetupDependencies,
): Promise<boolean> {
  if (dependencies.configurationFileExists(configurationFile)) {
    dependencies.writeError(
      `Display Configuration is invalid: ${configurationFile}`,
    );
    dependencies.writeOutput(
      "Press Enter to repair it with setup, or Q to quit without changing it.",
    );
    if (
      (await readAllowedSetupKey(dependencies, ["enter", "quit"])) === "quit"
    ) {
      return false;
    }
  }

  dependencies.writeOutput("RoonScape first-time setup");
  dependencies.writeOutput(
    "In an official Roon client, open Settings → Extensions and enable RoonScape.",
  );
  dependencies.writeOutput(
    "Waiting for Roon Authorization. [R] Retry  [Q] Quit",
  );

  const outputs = await waitForTrackedOutputs(dependencies);
  if (outputs === null) {
    return false;
  }

  const selected = await chooseTrackedOutput(outputs, dependencies);
  if (selected === null) {
    return false;
  }

  dependencies.writeOutput("OLED protection defaults:");
  dependencies.writeOutput(
    `  Dim after ${formatDuration(defaultInactivity.gracePeriodSeconds)}`,
  );
  dependencies.writeOutput(
    `  Use ${defaultInactivity.dimmedOpacity * 100} percent dimmed opacity`,
  );
  dependencies.writeOutput(
    `  Reposition every ${formatDuration(defaultInactivity.repositionCadenceSeconds)}`,
  );
  dependencies.writeOutput("Press Enter to accept all defaults, or Q to quit.");
  if ((await readAllowedSetupKey(dependencies, ["enter", "quit"])) === "quit") {
    return false;
  }

  dependencies.saveConfiguration(configurationFile, {
    trackedOutputId: selected.trackedOutputId,
    inactivity: defaultInactivity,
  });
  dependencies.writeOutput(`Display Configuration saved: ${configurationFile}`);
  return true;
}

type DiscoveryOutcome =
  | { kind: "outputs"; outputs: DiscoverableTrackedOutput[] }
  | { kind: "error"; error: unknown }
  | { kind: "key"; key: SetupKey }
  | { kind: "troubleshooting" };

async function waitForTrackedOutputs(
  dependencies: FirstTimeSetupDependencies,
): Promise<DiscoverableTrackedOutput[] | null> {
  while (true) {
    const discoveryAbort = new AbortController();
    const discovery = dependencies
      .discoverTrackedOutputs(
        dependencies.authorizationFile(),
        discoveryAbort.signal,
      )
      .then<DiscoveryOutcome, DiscoveryOutcome>(
        (outputs) => ({ kind: "outputs", outputs }),
        (error: unknown) => ({ kind: "error", error }),
      );
    let troubleshootingShown = false;

    while (true) {
      const keyAbort = new AbortController();
      const outcomes: Promise<DiscoveryOutcome>[] = [
        discovery,
        dependencies
          .readSetupKey(keyAbort.signal)
          .then<DiscoveryOutcome, DiscoveryOutcome>(
            (key) => ({ kind: "key", key }),
            (error: unknown) => ({ kind: "error", error }),
          ),
      ];
      if (!troubleshootingShown) {
        outcomes.push(
          dependencies
            .delay(15_000)
            .then<DiscoveryOutcome>(() => ({ kind: "troubleshooting" })),
        );
      }

      const outcome = await Promise.race(outcomes);
      if (outcome.kind === "outputs") {
        keyAbort.abort();
        if (outcome.outputs.length > 0) {
          return outcome.outputs;
        }

        dependencies.writeOutput(
          "No Tracked Outputs are available. [R] Refresh  [Q] Quit",
        );
        const key = await readAllowedSetupKey(dependencies, ["retry", "quit"]);
        if (key === "quit") {
          return null;
        }
        break;
      }

      if (outcome.kind === "key") {
        if (outcome.key === "quit") {
          discoveryAbort.abort();
          return null;
        }
        if (outcome.key === "retry") {
          discoveryAbort.abort();
          dependencies.writeOutput("Retrying Roon discovery…");
          break;
        }
        continue;
      }

      if (outcome.kind === "troubleshooting") {
        keyAbort.abort();
        troubleshootingShown = true;
        dependencies.writeOutput(
          "Still waiting. Confirm Roon is running, this host is on the same network, and RoonScape is enabled under Settings → Extensions. [R] Retry  [Q] Quit",
        );
        continue;
      }

      if (isAbortError(outcome.error)) {
        continue;
      }
      keyAbort.abort();
      discoveryAbort.abort();
      dependencies.writeError(
        `Roon discovery failed: ${outcome.error instanceof Error ? outcome.error.message : String(outcome.error)}`,
      );
      dependencies.writeOutput("[R] Retry  [Q] Quit");
      const key = await readAllowedSetupKey(dependencies, ["retry", "quit"]);
      if (key === "quit") {
        return null;
      }
      break;
    }
  }
}

async function chooseTrackedOutput(
  outputs: DiscoverableTrackedOutput[],
  dependencies: FirstTimeSetupDependencies,
): Promise<DiscoverableTrackedOutput | null> {
  let selectedIndex = 0;
  const duplicateLabels = duplicateOutputLabels(outputs);

  while (true) {
    dependencies.writeOutput("Choose the Tracked Output with ↑/↓ and Enter:");
    for (const [index, output] of outputs.entries()) {
      const baseLabel = trackedOutputLabel(output);
      const label = duplicateLabels.has(baseLabel)
        ? `${baseLabel} (${output.trackedOutputId})`
        : baseLabel;
      dependencies.writeOutput(
        `${index === selectedIndex ? ">" : " "} ${label}`,
      );
    }
    dependencies.writeOutput("Press Q to quit without saving.");

    const key = await readAllowedSetupKey(dependencies, [
      "up",
      "down",
      "enter",
      "quit",
    ]);
    if (key === "quit") {
      return null;
    }
    if (key === "enter") {
      return outputs[selectedIndex] ?? null;
    }
    if (key === "up") {
      selectedIndex = (selectedIndex - 1 + outputs.length) % outputs.length;
    } else if (key === "down") {
      selectedIndex = (selectedIndex + 1) % outputs.length;
    }
  }
}

function duplicateOutputLabels(
  outputs: DiscoverableTrackedOutput[],
): ReadonlySet<string> {
  const counts = new Map<string, number>();
  for (const output of outputs) {
    const label = trackedOutputLabel(output);
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return new Set(
    [...counts.entries()]
      .filter(([, count]) => count > 1)
      .map(([label]) => label),
  );
}

function trackedOutputLabel(output: DiscoverableTrackedOutput): string {
  return `${output.trackedOutputName} — ${output.trackedZoneName}`;
}

function formatDuration(seconds: number): string {
  if (seconds % 60 !== 0) {
    return `${seconds} ${seconds === 1 ? "second" : "seconds"}`;
  }
  const minutes = seconds / 60;
  return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
}

async function readAllowedSetupKey(
  dependencies: FirstTimeSetupDependencies,
  allowed: readonly SetupKey[],
): Promise<SetupKey> {
  while (true) {
    const key = await dependencies.readSetupKey(new AbortController().signal);
    if (allowed.includes(key)) {
      return key;
    }
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError";
}
