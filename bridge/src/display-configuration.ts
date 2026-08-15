import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";
import path from "node:path";

import { Ajv2020 } from "ajv/dist/2020.js";

import { isMissingFile, writePrivateJsonFile } from "./private-json-file.js";

export interface DisplayConfiguration {
  trackedOutputId: string;
  inactivity?: InactivityConfiguration;
}

export interface InactivityConfiguration {
  gracePeriodSeconds: number;
  dimmedOpacity: number;
  repositionCadenceSeconds: number;
}

export interface DisplayConfigurationStore {
  load(): DisplayConfiguration | null;
  save(configuration: DisplayConfiguration): void;
}

const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));
const displayConfigurationValidator = new Ajv2020({
  allErrors: true,
}).compile<DisplayConfiguration>(
  JSON.parse(
    readFileSync(
      path.resolve(repositoryRoot, "schema/display-configuration.schema.json"),
      "utf8",
    ),
  ) as object,
);

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
  return displayConfigurationValidator(candidate);
}
