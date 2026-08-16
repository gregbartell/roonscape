import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

import type { AuthorizationStore } from "./roon-bridge.js";
import { isMissingFile, writePrivateJsonFile } from "./private-json-file.js";

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
    writePrivateJsonFile(
      this.#authorizationFile,
      state,
      "Could not persist Roon authorization state",
    );
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
