import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { Ajv2020, type ValidateFunction } from "ajv/dist/2020.js";
import type { FormatsPlugin } from "ajv-formats";

export type Availability =
  "pairingRequired" | "disconnected" | "outputUnavailable" | "available";

export type Playback = "playing" | "paused" | "loading" | "stopped";

export interface PresentationSnapshot {
  schemaVersion: 1;
  revision: number;
  availability: Availability;
  playback: Playback | null;
  displayZone: { name: string } | null;
  nowPlaying: {
    title: string | null;
    artist: string | null;
    album: string | null;
  } | null;
  progress: {
    positionSeconds: number;
    durationSeconds: number;
    sampledAt: string;
  } | null;
  artwork: { revision: number; path: string } | null;
}

const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));
const loadCommonJsModule = createRequire(import.meta.url);
const addFormats = loadCommonJsModule("ajv-formats") as FormatsPlugin;
let snapshotValidator: ValidateFunction<PresentationSnapshot> | undefined;

function repositoryPath(relativePath: string): string {
  return path.resolve(repositoryRoot, relativePath);
}

async function validator(): Promise<ValidateFunction<PresentationSnapshot>> {
  if (snapshotValidator !== undefined) {
    return snapshotValidator;
  }

  const schemaContents = await readFile(
    repositoryPath("schema/presentation-snapshot.schema.json"),
    "utf8",
  );
  const ajv = new Ajv2020({ allErrors: true });
  addFormats(ajv);
  const compiledValidator = ajv.compile<PresentationSnapshot>(
    JSON.parse(schemaContents) as object,
  );
  snapshotValidator = compiledValidator;
  return compiledValidator;
}

export async function loadSnapshot(
  relativePath: string,
): Promise<PresentationSnapshot> {
  const contents = await readFile(repositoryPath(relativePath), "utf8");
  const candidate: unknown = JSON.parse(contents);
  const validate = await validator();

  if (!validate(candidate)) {
    throw new Error(
      `Invalid presentation snapshot: ${ajvErrors(validate.errors)}`,
    );
  }

  return candidate;
}

function ajvErrors(errors: ValidateFunction["errors"]): string {
  return (
    errors
      ?.map((error) => `${error.instancePath || "/"} ${error.message}`)
      .join("; ") ?? "unknown validation error"
  );
}
