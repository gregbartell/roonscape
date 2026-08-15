import type { DisplayConfigurationStore } from "./display-configuration.js";

export interface DiscoverableDisplayOutput {
  outputId: string;
  displayName: string;
  displayZoneName: string;
}

interface DisplayConfigurationCommandDependencies {
  configurationStore: DisplayConfigurationStore;
  discoverOutputs(): Promise<DiscoverableDisplayOutput[]>;
  writeLine(line: string): void;
}

export async function runDisplayConfigurationCommand(
  arguments_: string[],
  dependencies: DisplayConfigurationCommandDependencies,
): Promise<number> {
  const [command, ...operands] = arguments_;

  if (command === "list" && operands.length === 0) {
    const outputs = await dependencies.discoverOutputs();
    dependencies.writeLine("OUTPUT ID\tDISPLAY OUTPUT\tDISPLAY ZONE");
    for (const output of outputs) {
      dependencies.writeLine(
        `${output.outputId}\t${output.displayName}\t${output.displayZoneName}`,
      );
    }
    return 0;
  }

  if (command === "select" && operands.length === 1 && operands[0]) {
    const displayOutputId = operands[0];
    dependencies.configurationStore.save({ displayOutputId });
    dependencies.writeLine(`Selected Display Output: ${displayOutputId}`);
    return 0;
  }

  dependencies.writeLine(
    "Usage: npm run configure -- list | select <display-output-id>",
  );
  return 2;
}
