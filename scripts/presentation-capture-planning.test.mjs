import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { parsePresentationCaptureRequest } from "./presentation-capture-options.mjs";
import { planPresentationCaptures } from "./presentation-capture-planning.mjs";

test("preflight resolves the complete request and aggregates applicable failures", async () => {
  const workingDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-planning-test."),
  );
  await writeFile(path.join(workingDirectory, "captures"), "not a directory");
  const request = parsePresentationCaptureRequest([
    "--scenario",
    "playing",
    "--output",
    "captures",
  ]);

  try {
    await assert.rejects(
      planPresentationCaptures(request, {
        workingDirectory,
        environment: { ...process.env, PATH: "" },
      }),
      (error) => {
        assert.match(error.message, /required executable is unavailable: Xvfb/);
        assert.match(
          error.message,
          /required executable is unavailable: xwininfo/,
        );
        assert.match(
          error.message,
          /required executable is unavailable: scrot/,
        );
        assert.match(
          error.message,
          /required executable is unavailable: cargo/,
        );
        assert.match(error.message, /capture output is unavailable:/);
        return true;
      },
    );
  } finally {
    await rm(workingDirectory, { force: true, recursive: true });
  }
});
