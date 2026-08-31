import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import path from "node:path";

import { Ajv2020, type ValidateFunction } from "ajv/dist/2020.js";
import type { FormatsPlugin } from "ajv-formats";

import { repositoryRoot } from "./repository-root.js";

export type Availability =
  "pairingRequired" | "disconnected" | "outputUnavailable" | "available";

export type Playback = "playing" | "paused" | "loading" | "stopped";

export const MAX_TRACKED_IDENTITY_CODE_POINTS = 256;
export const MAX_NOW_PLAYING_CODE_POINTS = 1_024;

export type SnapshotDisplayStringViolationCode =
  | "trackedOutputNameTooLong"
  | "trackedOutputNameInvalidUnicode"
  | "trackedZoneNameTooLong"
  | "trackedZoneNameInvalidUnicode"
  | "titleTooLong"
  | "titleInvalidUnicode"
  | "artistTooLong"
  | "artistInvalidUnicode"
  | "albumTooLong"
  | "albumInvalidUnicode";

export interface SnapshotDisplayStringViolation {
  code: SnapshotDisplayStringViolationCode;
  message: string;
}

export interface PresentationSnapshot {
  schemaVersion: 2;
  revision: number;
  availability: Availability;
  playback: Playback | null;
  trackedOutput: { name: string } | null;
  trackedZone: { name: string } | null;
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
    repositoryPath("src/shared/schema/presentation-snapshot.schema.json"),
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
  return validateSnapshot(candidate);
}

export async function validateSnapshot(
  candidate: unknown,
): Promise<PresentationSnapshot> {
  const validate = await validator();

  if (!validate(candidate)) {
    throw new Error(
      `Invalid presentation snapshot: ${ajvErrors(validate.errors)}`,
    );
  }

  const displayStringViolation = findSnapshotDisplayStringViolation(candidate);
  if (displayStringViolation !== undefined) {
    throw new Error(
      `Invalid presentation snapshot: ${displayStringViolation.message}`,
    );
  }

  return candidate;
}

export function findSnapshotDisplayStringViolation(
  snapshot: PresentationSnapshot,
): SnapshotDisplayStringViolation | undefined {
  const fields: Array<{
    label: string;
    value: string | null | undefined;
    limit: number;
    tooLong: SnapshotDisplayStringViolationCode;
    invalidUnicode: SnapshotDisplayStringViolationCode;
  }> = [
    {
      label: "Tracked Output name",
      value: snapshot.trackedOutput?.name,
      limit: MAX_TRACKED_IDENTITY_CODE_POINTS,
      tooLong: "trackedOutputNameTooLong",
      invalidUnicode: "trackedOutputNameInvalidUnicode",
    },
    {
      label: "Tracked Zone name",
      value: snapshot.trackedZone?.name,
      limit: MAX_TRACKED_IDENTITY_CODE_POINTS,
      tooLong: "trackedZoneNameTooLong",
      invalidUnicode: "trackedZoneNameInvalidUnicode",
    },
    {
      label: "Title",
      value: snapshot.nowPlaying?.title,
      limit: MAX_NOW_PLAYING_CODE_POINTS,
      tooLong: "titleTooLong",
      invalidUnicode: "titleInvalidUnicode",
    },
    {
      label: "Artist",
      value: snapshot.nowPlaying?.artist,
      limit: MAX_NOW_PLAYING_CODE_POINTS,
      tooLong: "artistTooLong",
      invalidUnicode: "artistInvalidUnicode",
    },
    {
      label: "Album",
      value: snapshot.nowPlaying?.album,
      limit: MAX_NOW_PLAYING_CODE_POINTS,
      tooLong: "albumTooLong",
      invalidUnicode: "albumInvalidUnicode",
    },
  ];

  for (const field of fields) {
    if (field.value === null || field.value === undefined) {
      continue;
    }
    if (hasUnpairedSurrogate(field.value)) {
      return {
        code: field.invalidUnicode,
        message: `${field.label} contains invalid Unicode`,
      };
    }
    if ([...field.value].length > field.limit) {
      return {
        code: field.tooLong,
        message: `${field.label} exceeds ${field.limit.toLocaleString("en-US")} Unicode code points`,
      };
    }
  }

  return undefined;
}

function hasUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const nextCodeUnit = value.charCodeAt(index + 1);
      if (
        index + 1 >= value.length ||
        nextCodeUnit < 0xdc00 ||
        nextCodeUnit > 0xdfff
      ) {
        return true;
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      return true;
    }
  }
  return false;
}

function ajvErrors(errors: ValidateFunction["errors"]): string {
  return (
    errors
      ?.map((error) => `${error.instancePath || "/"} ${error.message}`)
      .join("; ") ?? "unknown validation error"
  );
}
