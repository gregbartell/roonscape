import type {
  DisplayConfigurationStore,
  InactivityConfiguration,
} from "./display-configuration.js";

export interface DiscoverableTrackedOutput {
  trackedOutputId: string;
  trackedOutputName: string;
  trackedZoneName: string;
}

interface DisplayConfigurationCommandDependencies {
  configurationStore: DisplayConfigurationStore;
  discoverTrackedOutputs(): Promise<DiscoverableTrackedOutput[]>;
  writeLine(line: string): void;
}

export async function runDisplayConfigurationCommand(
  arguments_: string[],
  dependencies: DisplayConfigurationCommandDependencies,
): Promise<number> {
  const [command, ...operands] = arguments_;

  if (command === "list" && operands.length === 0) {
    const outputs = await dependencies.discoverTrackedOutputs();
    dependencies.writeLine("TRACKED OUTPUT ID\tTRACKED OUTPUT\tTRACKED ZONE");
    for (const output of outputs) {
      dependencies.writeLine(
        `${output.trackedOutputId}\t${output.trackedOutputName}\t${output.trackedZoneName}`,
      );
    }
    return 0;
  }

  if (command === "select" && operands.length === 1 && operands[0]) {
    const trackedOutputId = operands[0];
    const existing = dependencies.configurationStore.load();
    dependencies.configurationStore.save({
      trackedOutputId,
      ...(existing?.inactivity === undefined
        ? {}
        : { inactivity: existing.inactivity }),
    });
    dependencies.writeLine(`Selected Tracked Output: ${trackedOutputId}`);
    return 0;
  }

  if (command === "inactivity" && operands.length === 3) {
    const existing = dependencies.configurationStore.load();
    const inactivity = parseInactivityConfiguration(operands);
    if (existing !== null && inactivity !== null) {
      dependencies.configurationStore.save({ ...existing, inactivity });
      dependencies.writeLine(
        `OLED inactivity: grace ${inactivity.gracePeriodSeconds}s, opacity ${inactivity.dimmedOpacity}, reposition every ${inactivity.repositionCadenceSeconds}s`,
      );
      return 0;
    }
  }

  dependencies.writeLine(
    "Usage: npm run configure -- list | select <tracked-output-id> | inactivity <grace-seconds> <dimmed-opacity> <reposition-cadence-seconds>",
  );
  return 2;
}

function parseInactivityConfiguration(
  operands: string[],
): InactivityConfiguration | null {
  const gracePeriod = Number(operands[0]);
  const dimmedOpacity = Number(operands[1]);
  const repositionCadence = Number(operands[2]);
  if (
    !Number.isSafeInteger(gracePeriod) ||
    gracePeriod <= 0 ||
    !Number.isFinite(dimmedOpacity) ||
    dimmedOpacity <= 0 ||
    dimmedOpacity >= 1 ||
    !Number.isSafeInteger(repositionCadence) ||
    repositionCadence <= 0
  ) {
    return null;
  }

  return {
    gracePeriodSeconds: gracePeriod,
    dimmedOpacity,
    repositionCadenceSeconds: repositionCadence,
  };
}
