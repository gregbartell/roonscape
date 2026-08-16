import { randomUUID } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import type { OwnedRuntime } from "./roonscape-command.js";

interface OpenRuntimeSessionOptions {
  environment: NodeJS.ProcessEnv;
  processId: number;
  userId: number;
  fallbackRuntimeRoot?(userId: number): string;
  processExists?(processId: number): boolean;
}

interface RuntimeOwner {
  processId: number;
  token: string;
}

const runtimeDirectoryName = "roonscape";
const ownershipDirectoryName = "owner";
const acquisitionDirectoryName = ".acquiring";
const ownershipFileName = "session.json";

export function openRuntimeSession({
  environment,
  processId,
  userId,
  fallbackRuntimeRoot = (ownerUserId) => `/run/user/${ownerUserId}`,
  processExists = defaultProcessExists,
}: OpenRuntimeSessionOptions): OwnedRuntime {
  const runtimeRoot = runtimeRootPath(environment, userId, fallbackRuntimeRoot);
  validatePrivateDirectory(runtimeRoot, userId, "runtime directory");

  const runtimeDirectory = path.join(runtimeRoot, runtimeDirectoryName);
  const owner: RuntimeOwner = { processId, token: randomUUID() };
  acquireOwnership(runtimeDirectory, owner, userId, processExists);

  return {
    socketPath: path.join(runtimeDirectory, "roonscape.sock"),
    cleanup: async () => {
      const currentOwner = readRuntimeOwner(
        path.join(runtimeDirectory, ownershipDirectoryName),
      );
      if (currentOwner.token !== owner.token) {
        throw new Error(
          "RoonScape runtime ownership changed before cleanup; refusing to remove it",
        );
      }
      rmSync(runtimeDirectory, { force: true, recursive: true });
    },
  };
}

function runtimeRootPath(
  environment: NodeJS.ProcessEnv,
  userId: number,
  fallbackRuntimeRoot: (userId: number) => string,
): string {
  const configuredRoot = environment.XDG_RUNTIME_DIR;
  const runtimeRoot =
    configuredRoot === undefined || configuredRoot.length === 0
      ? fallbackRuntimeRoot(userId)
      : configuredRoot;
  if (!path.isAbsolute(runtimeRoot)) {
    throw new Error(
      "XDG_RUNTIME_DIR must be an absolute, private directory owned by the current user",
    );
  }
  return path.resolve(runtimeRoot);
}

function acquireOwnership(
  runtimeDirectory: string,
  owner: RuntimeOwner,
  userId: number,
  processExists: (processId: number) => boolean,
): void {
  mkdirSync(runtimeDirectory, { mode: 0o700, recursive: true });
  validatePrivateDirectory(
    runtimeDirectory,
    userId,
    "RoonScape runtime directory",
  );
  const acquisitionDirectory = path.join(
    runtimeDirectory,
    acquisitionDirectoryName,
  );
  acquireAcquisitionGuard(acquisitionDirectory, owner, processExists);
  try {
    acquireSessionOwnership(runtimeDirectory, owner, processExists);
  } finally {
    releaseOwnedDirectory(acquisitionDirectory, owner);
  }
}

function acquireAcquisitionGuard(
  acquisitionDirectory: string,
  owner: RuntimeOwner,
  processExists: (processId: number) => boolean,
): void {
  try {
    mkdirSync(acquisitionDirectory, { mode: 0o700 });
    try {
      writeRuntimeOwner(acquisitionDirectory, owner);
    } catch (error) {
      rmSync(acquisitionDirectory, { force: true, recursive: true });
      throw error;
    }
  } catch (error) {
    if (!isAlreadyExists(error)) {
      throw error;
    }
    const acquisitionOwner = readRuntimeOwner(acquisitionDirectory);
    if (processExists(acquisitionOwner.processId)) {
      throw new Error(
        `Another RoonScape launch is acquiring runtime ownership (process ${acquisitionOwner.processId})`,
        { cause: error },
      );
    }
    throw new Error(
      `Cannot safely recover interrupted runtime acquisition at ${acquisitionDirectory}; remove it only after confirming no RoonScape launch is running`,
      { cause: error },
    );
  }
}

function acquireSessionOwnership(
  runtimeDirectory: string,
  owner: RuntimeOwner,
  processExists: (processId: number) => boolean,
): void {
  for (;;) {
    const ownershipDirectory = path.join(
      runtimeDirectory,
      ownershipDirectoryName,
    );
    try {
      mkdirSync(ownershipDirectory, { mode: 0o700 });
      try {
        writeRuntimeOwner(ownershipDirectory, owner);
        reclaimVerifiedArtifacts(runtimeDirectory, processExists);
      } catch (error) {
        rmSync(ownershipDirectory, { force: true, recursive: true });
        throw error;
      }
      return;
    } catch (error) {
      if (!isAlreadyExists(error)) {
        throw error;
      }
    }

    const previousOwner = readRuntimeOwner(ownershipDirectory);
    if (processExists(previousOwner.processId)) {
      throw new Error(
        `RoonScape is already running for this user (process ${previousOwner.processId})`,
      );
    }

    const staleOwnershipDirectory = path.join(
      runtimeDirectory,
      `.stale-owner-${randomUUID()}`,
    );
    try {
      renameSync(ownershipDirectory, staleOwnershipDirectory);
    } catch (error) {
      if (isMissingFile(error)) {
        continue;
      }
      throw error;
    }
  }
}

function reclaimVerifiedArtifacts(
  runtimeDirectory: string,
  processExists: (processId: number) => boolean,
): void {
  const artifactNames = readdirSync(runtimeDirectory).filter(
    (name) =>
      name !== ownershipDirectoryName && name !== acquisitionDirectoryName,
  );
  if (artifactNames.length === 0) {
    return;
  }

  const staleOwnershipDirectories = artifactNames.filter((name) =>
    name.startsWith(".stale-owner-"),
  );
  if (staleOwnershipDirectories.length === 0) {
    throw new Error(
      `Cannot verify runtime artifacts at ${runtimeDirectory}; remove them only after confirming no RoonScape process is running`,
    );
  }
  for (const staleOwnershipDirectory of staleOwnershipDirectories) {
    const staleOwner = readRuntimeOwner(
      path.join(runtimeDirectory, staleOwnershipDirectory),
    );
    if (processExists(staleOwner.processId)) {
      throw new Error(
        `Cannot reclaim runtime artifacts still owned by process ${staleOwner.processId}`,
      );
    }
  }
  for (const artifactName of artifactNames) {
    rmSync(path.join(runtimeDirectory, artifactName), {
      force: true,
      recursive: true,
    });
  }
}

function writeRuntimeOwner(
  ownershipDirectory: string,
  owner: RuntimeOwner,
): void {
  writeFileSync(
    path.join(ownershipDirectory, ownershipFileName),
    `${JSON.stringify(owner)}\n`,
    { encoding: "utf8", flag: "wx", mode: 0o600 },
  );
}

function releaseOwnedDirectory(
  ownershipDirectory: string,
  owner: RuntimeOwner,
): void {
  const currentOwner = readRuntimeOwner(ownershipDirectory);
  if (currentOwner.token !== owner.token) {
    throw new Error(
      `RoonScape runtime ownership changed at ${ownershipDirectory}; refusing to remove it`,
    );
  }
  rmSync(ownershipDirectory, { force: true, recursive: true });
}

function readRuntimeOwner(ownershipDirectory: string): RuntimeOwner {
  const ownershipFile = path.join(ownershipDirectory, ownershipFileName);
  let candidate: unknown;
  try {
    candidate = JSON.parse(readFileSync(ownershipFile, "utf8")) as unknown;
  } catch (error) {
    throw new Error(
      `Cannot verify existing RoonScape runtime ownership at ${ownershipFile}; remove it only after confirming no RoonScape process is running`,
      { cause: error },
    );
  }
  if (
    typeof candidate !== "object" ||
    candidate === null ||
    !("processId" in candidate) ||
    !("token" in candidate) ||
    !Number.isSafeInteger(candidate.processId) ||
    (candidate.processId as number) <= 0 ||
    typeof candidate.token !== "string" ||
    candidate.token.length === 0
  ) {
    throw new Error(
      `Cannot verify existing RoonScape runtime ownership at ${ownershipFile}; remove it only after confirming no RoonScape process is running`,
    );
  }
  return {
    processId: candidate.processId as number,
    token: candidate.token,
  };
}

function validatePrivateDirectory(
  directory: string,
  userId: number,
  label: string,
): void {
  let details;
  try {
    details = lstatSync(directory);
  } catch (error) {
    throw new Error(
      `${label} is unavailable at ${directory}; set XDG_RUNTIME_DIR to a private runtime directory`,
      { cause: error },
    );
  }
  if (
    !details.isDirectory() ||
    details.isSymbolicLink() ||
    details.uid !== userId ||
    (details.mode & 0o777) !== 0o700
  ) {
    throw new Error(
      `${label} must be a mode-0700 directory owned by the current user: ${directory}`,
    );
  }
}

function defaultProcessExists(processId: number): boolean {
  try {
    process.kill(processId, 0);
    return true;
  } catch (error) {
    if (isMissingProcess(error)) {
      return false;
    }
    if (isPermissionDenied(error)) {
      return true;
    }
    throw error;
  }
}

function isAlreadyExists(error: unknown): boolean {
  return errorCode(error) === "EEXIST";
}

function isMissingFile(error: unknown): boolean {
  return errorCode(error) === "ENOENT";
}

function isMissingProcess(error: unknown): boolean {
  return errorCode(error) === "ESRCH";
}

function isPermissionDenied(error: unknown): boolean {
  return errorCode(error) === "EPERM";
}

function errorCode(error: unknown): string | undefined {
  return error instanceof Error && "code" in error
    ? (error as NodeJS.ErrnoException).code
    : undefined;
}
