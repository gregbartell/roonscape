import { randomUUID } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

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
    const configDirectory = path.dirname(this.#configurationFile);
    mkdirSync(configDirectory, { mode: 0o700, recursive: true });
    const temporaryFile = path.join(
      configDirectory,
      `.${path.basename(this.#configurationFile)}.${randomUUID()}.tmp`,
    );

    try {
      writeFileSync(
        temporaryFile,
        `${JSON.stringify(configuration, null, 2)}\n`,
        {
          encoding: "utf8",
          flag: "wx",
          mode: 0o600,
        },
      );
      renameSync(temporaryFile, this.#configurationFile);
    } catch (error) {
      try {
        unlinkSync(temporaryFile);
      } catch (cleanupError) {
        if (!isMissingFile(cleanupError)) {
          throw new AggregateError(
            [error, cleanupError],
            "Could not persist Display Configuration",
            { cause: cleanupError },
          );
        }
      }

      throw error;
    }
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

function isMissingFile(error: unknown): boolean {
  return (
    error instanceof Error &&
    "code" in error &&
    (error as NodeJS.ErrnoException).code === "ENOENT"
  );
}
