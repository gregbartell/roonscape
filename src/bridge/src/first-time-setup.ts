import type { DiscoverableTrackedOutput } from "./display-configuration-command.js";
import type {
  DisplayConfiguration,
  InactivityConfiguration,
} from "./display-configuration.js";

export type SetupKey = "up" | "down" | "enter" | "customize";

export interface SetupDependencies {
  authorizationFile(): string;
  configurationFileExists(configurationFile: string): boolean;
  discoverTrackedOutputs(
    authorizationFile: string,
    signal: AbortSignal,
  ): Promise<DiscoverableTrackedOutput[]>;
  readSetupKey(signal: AbortSignal): Promise<SetupKey>;
  readSetupValue(
    prompt: string,
    initialValue: string,
    signal: AbortSignal,
  ): Promise<string>;
  saveConfiguration(
    configurationFile: string,
    configuration: DisplayConfiguration,
  ): void;
  delay(milliseconds: number): Promise<void>;
  writeSetupLines(lines: readonly string[], replacePrevious: boolean): void;
  writeOutput(line: string): void;
  writeError(line: string): void;
}

const defaultInactivity = {
  gracePeriodSeconds: 300,
  dimmedOpacity: 0.35,
  repositionCadenceSeconds: 60,
} as const;

export async function runSetup(
  configurationFile: string,
  dependencies: SetupDependencies,
  existingConfiguration: DisplayConfiguration | null,
  signal: AbortSignal,
): Promise<void> {
  if (
    existingConfiguration === null &&
    dependencies.configurationFileExists(configurationFile)
  ) {
    dependencies.writeError(
      `Display Configuration is invalid: ${configurationFile}`,
    );
    dependencies.writeOutput("Press Enter to repair it with setup.");
    await readAllowedSetupKey(dependencies, ["enter"], signal);
  }

  dependencies.writeOutput(
    existingConfiguration === null
      ? "RoonScape first-time setup"
      : "RoonScape setup",
  );
  dependencies.writeOutput(
    "Press Ctrl-C at any time to cancel without saving.",
  );
  dependencies.writeOutput(
    "In an official Roon client, open Settings → Extensions and enable RoonScape.",
  );
  dependencies.writeOutput("Waiting for Roon Authorization.");

  const outputs = await waitForTrackedOutputs(dependencies, signal);

  const selected = await chooseTrackedOutput(
    outputs,
    dependencies,
    signal,
    existingConfiguration?.trackedOutputId,
  );

  const inactivity = existingConfiguration?.inactivity ?? defaultInactivity;
  dependencies.writeOutput(
    existingConfiguration === null
      ? "OLED protection defaults:"
      : "OLED protection settings:",
  );
  dependencies.writeOutput(
    `  Dim after ${formatDuration(inactivity.gracePeriodSeconds)}`,
  );
  dependencies.writeOutput(
    `  Use ${formatPercentage(inactivity.dimmedOpacity)} percent dimmed opacity`,
  );
  dependencies.writeOutput(
    `  Reposition every ${formatDuration(inactivity.repositionCadenceSeconds)}`,
  );
  dependencies.writeOutput(
    existingConfiguration === null
      ? "Press Enter to accept all defaults, or C to customize."
      : "Press Enter to keep these settings, or C to customize.",
  );
  const inactivityChoice = await readAllowedSetupKey(
    dependencies,
    ["enter", "customize"],
    signal,
  );
  const completedInactivity =
    inactivityChoice === "customize"
      ? await customizeInactivity(inactivity, dependencies, signal)
      : inactivity;

  dependencies.saveConfiguration(configurationFile, {
    trackedOutputId: selected.trackedOutputId,
    inactivity: completedInactivity,
  });
  dependencies.writeOutput(`Display Configuration saved: ${configurationFile}`);
}

async function waitForTrackedOutputs(
  dependencies: SetupDependencies,
  signal: AbortSignal,
): Promise<DiscoverableTrackedOutput[]> {
  const discovery = dependencies
    .discoverTrackedOutputs(dependencies.authorizationFile(), signal)
    .catch((error: unknown) => {
      if (isAbortError(error)) {
        throw error;
      }
      throw new Error(
        `Roon discovery failed: ${error instanceof Error ? error.message : String(error)}. Rerun roonscape --setup to try again.`,
        { cause: error },
      );
    });
  const firstOutcome = await Promise.race([
    discovery.then((outputs) => ({ kind: "outputs" as const, outputs })),
    dependencies
      .delay(15_000)
      .then(() => ({ kind: "troubleshooting" as const })),
  ]);

  let outputs: DiscoverableTrackedOutput[];
  if (firstOutcome.kind === "outputs") {
    outputs = firstOutcome.outputs;
  } else {
    dependencies.writeOutput(
      "Still waiting. Confirm Roon is running, this host is on the same network, and RoonScape is enabled under Settings → Extensions.",
    );
    outputs = await discovery;
  }
  if (outputs.length === 0) {
    throw new Error(
      "No Tracked Outputs are available. Make an output available in Roon, then rerun roonscape --setup.",
    );
  }
  return outputs;
}

async function chooseTrackedOutput(
  outputs: DiscoverableTrackedOutput[],
  dependencies: SetupDependencies,
  signal: AbortSignal,
  trackedOutputId?: string,
): Promise<DiscoverableTrackedOutput> {
  const savedIndex = outputs.findIndex(
    (output) => output.trackedOutputId === trackedOutputId,
  );
  let selectedIndex = savedIndex < 0 ? 0 : savedIndex;
  const duplicateLabels = duplicateOutputLabels(outputs);
  let selectionRendered = false;

  while (true) {
    const lines = ["Choose the Tracked Output with ↑/↓ and Enter:"];
    for (const [index, output] of outputs.entries()) {
      const baseLabel = trackedOutputLabel(output);
      const label = duplicateLabels.has(baseLabel)
        ? `${baseLabel} (${output.trackedOutputId})`
        : baseLabel;
      lines.push(`${index === selectedIndex ? ">" : " "} ${label}`);
    }
    dependencies.writeSetupLines(lines, selectionRendered);
    selectionRendered = true;
    const key = await readAllowedSetupKey(
      dependencies,
      ["up", "down", "enter"],
      signal,
    );
    if (key === "enter") {
      const selected = outputs[selectedIndex];
      if (selected === undefined) {
        throw new Error("Tracked Output selection is unavailable");
      }
      return selected;
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

function formatPercentage(opacity: number): string {
  return String(Number((opacity * 100).toPrecision(15)));
}

async function customizeInactivity(
  inactivity: InactivityConfiguration,
  dependencies: SetupDependencies,
  signal: AbortSignal,
): Promise<InactivityConfiguration> {
  const gracePeriodSeconds = await readValidatedSetupValue(
    dependencies,
    "Grace period in minutes:",
    String(inactivity.gracePeriodSeconds / 60),
    parseMinutesAsSeconds,
    "Grace period must be a positive number of minutes.",
    signal,
  );

  const dimmedOpacity = await readValidatedSetupValue(
    dependencies,
    "Dimmed opacity in percent:",
    formatPercentage(inactivity.dimmedOpacity),
    parsePercentageAsOpacity,
    "Dimmed opacity must be greater than 0 and less than 100 percent.",
    signal,
  );

  const repositionCadenceSeconds = await readValidatedSetupValue(
    dependencies,
    "Reposition cadence in seconds:",
    String(inactivity.repositionCadenceSeconds),
    parsePositiveWholeNumber,
    "Reposition cadence must be a positive whole number of seconds.",
    signal,
  );

  return {
    gracePeriodSeconds,
    dimmedOpacity,
    repositionCadenceSeconds,
  };
}

async function readValidatedSetupValue(
  dependencies: SetupDependencies,
  prompt: string,
  initialValue: string,
  parse: (value: string) => number | null,
  validationMessage: string,
  signal: AbortSignal,
): Promise<number> {
  while (true) {
    const value = await dependencies.readSetupValue(
      prompt,
      initialValue,
      signal,
    );
    const parsed = parse(value);
    if (parsed !== null) {
      return parsed;
    }
    dependencies.writeError(validationMessage);
  }
}

function parseMinutesAsSeconds(value: string): number | null {
  const seconds = Number(value) * 60;
  return value.trim() !== "" && Number.isSafeInteger(seconds) && seconds > 0
    ? seconds
    : null;
}

function parsePercentageAsOpacity(value: string): number | null {
  const percentage = Number(value);
  return value.trim() !== "" &&
    Number.isFinite(percentage) &&
    percentage > 0 &&
    percentage < 100
    ? percentage / 100
    : null;
}

function parsePositiveWholeNumber(value: string): number | null {
  const parsed = Number(value);
  return value.trim() !== "" && Number.isSafeInteger(parsed) && parsed > 0
    ? parsed
    : null;
}

async function readAllowedSetupKey(
  dependencies: SetupDependencies,
  allowed: readonly SetupKey[],
  signal: AbortSignal,
): Promise<SetupKey> {
  while (true) {
    const key = await dependencies.readSetupKey(signal);
    if (allowed.includes(key)) {
      return key;
    }
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError";
}
