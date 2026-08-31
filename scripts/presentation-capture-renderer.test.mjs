import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { publishPresentationCapture } from "./presentation-capture-publication.mjs";
import { runControlledRendererSession } from "./presentation-capture-renderer.mjs";
import {
  installPresentationCaptureFixtures,
  presentationCapturePngHeader,
} from "./presentation-capture-test-fixtures.mjs";

test("controlled renderer session waits for painted readiness before publication", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-renderer-session-test."),
  );
  const binDirectory = path.join(directory, "bin");
  const pngDirectory = path.join(directory, "pngs");
  const processLog = path.join(directory, "processes");
  const finalCapturePath = path.join(directory, "1280x720--playing.png");
  const { renderer } = await installPresentationCaptureFixtures(binDirectory);
  await mkdir(pngDirectory);
  await writeFile(
    path.join(pngDirectory, "1280x720.png"),
    presentationCapturePngHeader(1280, 720),
  );
  const snapshot = JSON.parse(
    await readFile(
      new URL("../src/shared/fixtures/playing.json", import.meta.url),
      "utf8",
    ),
  );
  const captures = [
    {
      scenario: "playing",
      viewport: "1280x720",
      width: 1280,
      height: 720,
      typography: "automatic",
      diagnostics: false,
      finalCapturePath,
      snapshot,
    },
  ];
  const published = [];

  try {
    await runControlledRendererSession(captures, {
      environment: {
        ...process.env,
        PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
        ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY: pngDirectory,
        ROONSCAPE_CAPTURE_TEST_PROCESS_LOG: processLog,
        ROONSCAPE_CAPTURE_TEST_RENDERER: renderer,
      },
      publishCapture: publishPresentationCapture,
      onCapturePublished: (capturePath) => published.push(capturePath),
    });

    assert.deepEqual(published, [finalCapturePath]);
    assert.deepEqual(
      await readFile(finalCapturePath),
      await readFile(path.join(pngDirectory, "1280x720.png")),
    );
    assert.deepEqual(
      (await readFile(processLog, "utf8"))
        .split("\n")
        .filter((line) => /^(selection|painted|scrot)\|/.test(line))
        .map((line) => line.slice(0, line.indexOf("|"))),
      ["selection", "painted", "scrot"],
    );
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});
