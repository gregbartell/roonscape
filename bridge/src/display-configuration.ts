import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

import { isMissingFile, writePrivateJsonFile } from "./private-json-file.js";

export interface DisplayConfiguration {
  displayOutputId: string;
}

export interface DisplayConfigurationStore {
  load(): DisplayConfiguration | null;
  save(configuration: DisplayConfiguration): void;
}

export class FileDisplayConfigurationStore implements DisplayConfigurationStore {
  readonly #configurationFile: string;

  constructor(configurationFile: string) {
    this.#configurationFile = configurationFile;
  }

  load(): DisplayConfiguration | null {
    try {
      const candidate: unknown = JSON.parse(
        readFileSync(this.#configurationFile, "utf8"),
      );
      return isDisplayConfiguration(candidate) ? candidate : null;
    } catch (error) {
      if (isMissingFile(error) || error instanceof SyntaxError) {
        return null;
      }

      throw error;
    }
  }

  save(configuration: DisplayConfiguration): void {
    writePrivateJsonFile(
      this.#configurationFile,
      configuration,
      "Could not persist Display Configuration",
    );
  }
}

export function displayConfigurationFilePath(
  environment: NodeJS.ProcessEnv = process.env,
): string {
  if (environment.ROONSCAPE_DISPLAY_CONFIG) {
    return path.resolve(environment.ROONSCAPE_DISPLAY_CONFIG);
  }

  const configRoot =
    environment.XDG_CONFIG_HOME ?? path.join(homedir(), ".config");
  return path.join(configRoot, "roonscape", "display.json");
}

function isDisplayConfiguration(
  candidate: unknown,
): candidate is DisplayConfiguration {
  return (
    typeof candidate === "object" &&
    candidate !== null &&
    !Array.isArray(candidate) &&
    Object.keys(candidate).length === 1 &&
    "displayOutputId" in candidate &&
    typeof candidate.displayOutputId === "string" &&
    candidate.displayOutputId.length > 0
  );
}
