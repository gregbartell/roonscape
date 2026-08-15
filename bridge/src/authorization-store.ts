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

import type { AuthorizationStore } from "./roon-availability.js";

export class FileAuthorizationStore implements AuthorizationStore {
  readonly #authorizationFile: string;

  constructor(authorizationFile: string) {
    this.#authorizationFile = authorizationFile;
  }

  load(): unknown {
    try {
      return JSON.parse(
        readFileSync(this.#authorizationFile, "utf8"),
      ) as unknown;
    } catch (error) {
      if (isMissingFile(error)) {
        return {};
      }

      throw error;
    }
  }

  save(state: unknown): void {
    const stateDirectory = path.dirname(this.#authorizationFile);
    mkdirSync(stateDirectory, { mode: 0o700, recursive: true });
    const temporaryFile = path.join(
      stateDirectory,
      `.${path.basename(this.#authorizationFile)}.${randomUUID()}.tmp`,
    );

    try {
      writeFileSync(temporaryFile, `${JSON.stringify(state, null, 2)}\n`, {
        encoding: "utf8",
        flag: "wx",
        mode: 0o600,
      });
      renameSync(temporaryFile, this.#authorizationFile);
    } catch (error) {
      try {
        unlinkSync(temporaryFile);
      } catch (cleanupError) {
        if (!isMissingFile(cleanupError)) {
          throw new AggregateError(
            [error, cleanupError],
            "Could not persist Roon authorization state",
            { cause: cleanupError },
          );
        }
      }

      throw error;
    }
  }
}

export function authorizationFilePath(
  environment: NodeJS.ProcessEnv = process.env,
): string {
  if (environment.ROONSCAPE_AUTHORIZATION_FILE) {
    return path.resolve(environment.ROONSCAPE_AUTHORIZATION_FILE);
  }

  const stateRoot =
    environment.XDG_STATE_HOME ?? path.join(homedir(), ".local", "state");
  return path.join(stateRoot, "roonscape", "authorization.json");
}

function isMissingFile(error: unknown): boolean {
  return (
    error instanceof Error &&
    "code" in error &&
    (error as NodeJS.ErrnoException).code === "ENOENT"
  );
}
