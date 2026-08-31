import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { assertSnapshotPublishable } from "../src/bridge/dist/src/fixture-publisher.js";
import { validateSnapshot } from "../src/bridge/dist/src/snapshot.js";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

export async function validatePresentationCaptureSnapshot(candidate) {
  const snapshot = await validateSnapshot(candidate);
  assertSnapshotPublishable(snapshot);
  return snapshot;
}

export async function loadPresentationCaptureSnapshot(fixture) {
  const candidate = JSON.parse(
    await readFile(path.join(repositoryRoot, fixture), "utf8"),
  );
  return validatePresentationCaptureSnapshot(candidate);
}
