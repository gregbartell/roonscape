import { randomUUID } from "node:crypto";
import {
  existsSync,
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
const recoveryDirectoryName = ".recovering";
const recoverySuccessorName = "successor";
const ownershipFileName = "session.json";
const candidatePrefix = ".owner-candidate-";

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
  mkdirSync(runtimeDirectory, { mode: 0o700, recursive: true });
  validatePrivateDirectory(
    runtimeDirectory,
    userId,
    "RoonScape runtime directory",
  );

  const owner: RuntimeOwner = { processId, token: randomUUID() };
  acquireOwnership(runtimeDirectory, owner, processExists);

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
  processExists: (processId: number) => boolean,
): void {
  const ownershipDirectory = path.join(
    runtimeDirectory,
    ownershipDirectoryName,
  );
  const recoveryDirectory = path.join(runtimeDirectory, recoveryDirectoryName);

  for (;;) {
    if (existsSync(recoveryDirectory)) {
      recoverOwnership(
        runtimeDirectory,
        ownershipDirectory,
        recoveryDirectory,
        owner,
        processExists,
      );
      return;
    }

    if (publishOwnedDirectory(ownershipDirectory, owner)) {
      try {
        removeDeadCandidates(runtimeDirectory, processExists);
        const unknownArtifacts = runtimeArtifactNames(runtimeDirectory);
        if (unknownArtifacts.length === 0) {
          return;
        }
        throw new Error(
          `Cannot verify runtime artifacts at ${runtimeDirectory}; remove them only after confirming no RoonScape process is running`,
        );
      } catch (error) {
        releaseOwnedDirectory(ownershipDirectory, owner);
        throw error;
      }
    }

    const previousOwner = tryReadRuntimeOwner(ownershipDirectory);
    if (previousOwner === undefined) {
      continue;
    }
    if (processExists(previousOwner.processId)) {
      throw new Error(
        `RoonScape is already running for this user (process ${previousOwner.processId})`,
      );
    }
    recoverOwnership(
      runtimeDirectory,
      ownershipDirectory,
      recoveryDirectory,
      owner,
      processExists,
    );
    return;
  }
}

function recoverOwnership(
  runtimeDirectory: string,
  ownershipDirectory: string,
  recoveryDirectory: string,
  owner: RuntimeOwner,
  processExists: (processId: number) => boolean,
): void {
  const recoveryLeaf = acquireRecoveryLeaf(
    recoveryDirectory,
    owner,
    processExists,
  );
  const previousOwner = tryReadRuntimeOwner(ownershipDirectory);
  if (previousOwner !== undefined) {
    if (processExists(previousOwner.processId)) {
      throw new Error(
        `Cannot reclaim runtime state still owned by process ${previousOwner.processId}`,
      );
    }
    rmSync(ownershipDirectory, { force: true, recursive: true });
  }

  if (!publishOwnedDirectory(ownershipDirectory, owner)) {
    throw new Error(
      "RoonScape runtime ownership changed during recovery; refusing to reclaim artifacts",
    );
  }
  removeRuntimeArtifacts(runtimeDirectory);
  releaseRecoveryChain(recoveryDirectory, recoveryLeaf, owner);
}

function acquireRecoveryLeaf(
  recoveryDirectory: string,
  owner: RuntimeOwner,
  processExists: (processId: number) => boolean,
): string {
  if (publishOwnedDirectory(recoveryDirectory, owner)) {
    return recoveryDirectory;
  }

  let recoveryLeaf = recoveryDirectory;
  for (;;) {
    const recoveryOwner = readRuntimeOwner(recoveryLeaf);
    if (processExists(recoveryOwner.processId)) {
      throw new Error(
        `Another RoonScape launch is recovering runtime ownership (process ${recoveryOwner.processId})`,
      );
    }
    const successor = path.join(recoveryLeaf, recoverySuccessorName);
    if (publishOwnedDirectory(successor, owner)) {
      return successor;
    }
    recoveryLeaf = successor;
  }
}

function publishOwnedDirectory(
  ownershipDirectory: string,
  owner: RuntimeOwner,
): boolean {
  const candidateDirectory = path.join(
    path.dirname(ownershipDirectory),
    `${candidatePrefix}${randomUUID()}`,
  );
  mkdirSync(candidateDirectory, { mode: 0o700 });
  try {
    writeRuntimeOwner(candidateDirectory, owner);
    renameSync(candidateDirectory, ownershipDirectory);
    return true;
  } catch (error) {
    rmSync(candidateDirectory, { force: true, recursive: true });
    if (isOccupied(error)) {
      return false;
    }
    throw error;
  }
}

function removeDeadCandidates(
  runtimeDirectory: string,
  processExists: (processId: number) => boolean,
): void {
  for (const candidateName of readdirSync(runtimeDirectory).filter((name) =>
    name.startsWith(candidatePrefix),
  )) {
    const candidateDirectory = path.join(runtimeDirectory, candidateName);
    const candidateOwner = readRuntimeOwner(candidateDirectory);
    if (processExists(candidateOwner.processId)) {
      throw new Error(
        `Another RoonScape launch is acquiring runtime ownership (process ${candidateOwner.processId})`,
      );
    }
    rmSync(candidateDirectory, { force: true, recursive: true });
  }
}

function runtimeArtifactNames(runtimeDirectory: string): string[] {
  return readdirSync(runtimeDirectory).filter(
    (name) => name !== ownershipDirectoryName && name !== recoveryDirectoryName,
  );
}

function removeRuntimeArtifacts(runtimeDirectory: string): void {
  for (const artifactName of runtimeArtifactNames(runtimeDirectory)) {
    rmSync(path.join(runtimeDirectory, artifactName), {
      force: true,
      recursive: true,
    });
  }
}

function releaseRecoveryChain(
  recoveryDirectory: string,
  recoveryLeaf: string,
  owner: RuntimeOwner,
): void {
  const currentOwner = readRuntimeOwner(recoveryLeaf);
  if (currentOwner.token !== owner.token) {
    throw new Error(
      "RoonScape recovery ownership changed; refusing to remove its state",
    );
  }
  rmSync(recoveryDirectory, { force: true, recursive: true });
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

function tryReadRuntimeOwner(
  ownershipDirectory: string,
): RuntimeOwner | undefined {
  try {
    return readRuntimeOwner(ownershipDirectory);
  } catch (error) {
    if (isMissingFile(error)) {
      return undefined;
    }
    throw error;
  }
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

function isOccupied(error: unknown): boolean {
  return ["EEXIST", "ENOTEMPTY"].includes(errorCode(error) ?? "");
}

function isMissingFile(error: unknown): boolean {
  if (errorCode(error) === "ENOENT") {
    return true;
  }
  return error instanceof Error && "cause" in error
    ? isMissingFile(error.cause)
    : false;
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
