import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { publishPresentationCapture } from "./presentation-capture-publication.mjs";
import { presentationCapturePngHeader } from "./presentation-capture-test-fixtures.mjs";

test("publishes a validated Presentation Capture atomically", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-publication-test."),
  );
  const finalCapturePath = path.join(directory, "capture.png");
  await writeFile(finalCapturePath, "previous capture");

  try {
    await assert.rejects(
      publishPresentationCapture({
        finalCapturePath,
        width: 1280,
        height: 720,
        produce: (temporaryCapturePath) =>
          writeFile(temporaryCapturePath, "invalid capture"),
      }),
      /is not a PNG capture/,
    );
    assert.equal(await readFile(finalCapturePath, "utf8"), "previous capture");

    await publishPresentationCapture({
      finalCapturePath,
      width: 1280,
      height: 720,
      produce: (temporaryCapturePath) =>
        writeFile(
          temporaryCapturePath,
          presentationCapturePngHeader(1280, 720),
        ),
    });
    assert.deepEqual(
      await readFile(finalCapturePath),
      presentationCapturePngHeader(1280, 720),
    );
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});
