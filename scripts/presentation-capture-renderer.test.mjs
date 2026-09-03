import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { executePresentationCapturePlan } from "./presentation-capture-execution.mjs";
import { publishPresentationCapture } from "./presentation-capture-publication.mjs";
import { createControlledRendererSessionAdapter } from "./presentation-capture-renderer.mjs";
import {
  installPresentationCaptureFixtures,
  presentationCapturePngHeader,
} from "./presentation-capture-test-fixtures.mjs";

test("controlled Renderer sessions reuse compatible captures and wait for painted readiness", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-renderer-session-test."),
  );
  const { environment, pngDirectory, processLog, renderer } =
    await installRendererSessionFixture(directory, "profile");
  const snapshot = JSON.parse(
    await readFile(
      new URL("../src/shared/fixtures/playing.json", import.meta.url),
      "utf8",
    ),
  );
  const capture = (scenario, typography, diagnostics) => ({
    scenario,
    viewport: "1280x720",
    width: 1280,
    height: 720,
    typography,
    diagnostics,
    finalCapturePath: path.join(directory, `${scenario}.png`),
    snapshot,
  });
  const captures = [
    capture("playing", "automatic", false),
    capture("paused", "automatic", false),
    capture("preferred-typography", "preferred", false),
    capture("fallback-typography", "fallback", false),
    capture("dark-diagnostics", "automatic", true),
  ];
  const plan = {
    captures,
    sessions: [
      captures.slice(0, 2),
      ...captures.slice(2).map((capture) => [capture]),
    ],
  };

  try {
    const completedPaths = await executePresentationCapturePlan(plan, {
      sessionAdapter: createControlledRendererSessionAdapter({
        environment,
        publishCapture: publishPresentationCapture,
        rendererExecutable: renderer,
      }),
    });

    assert.deepEqual(
      completedPaths,
      captures.map(({ finalCapturePath }) => finalCapturePath),
    );
    for (const { finalCapturePath } of captures) {
      assert.deepEqual(
        await readFile(finalCapturePath),
        await readFile(path.join(pngDirectory, "1280x720.png")),
      );
    }
    const processes = await readFile(processLog, "utf8");
    assert.deepEqual(
      [...processes.matchAll(/^renderer\|(.+)$/gm)].map((match) => match[1]),
      [
        "1280x720|automatic|0",
        "1280x720|preferred|0",
        "1280x720|fallback|0",
        "1280x720|automatic|1",
      ],
    );
    assert.deepEqual(
      processes
        .split("\n")
        .filter((line) => /^(selection|painted|scrot)\|/.test(line))
        .map((line) => line.slice(0, line.indexOf("|"))),
      Array.from({ length: captures.length }, () => [
        "selection",
        "painted",
        "scrot",
      ]).flat(),
    );
    assert.equal((processes.match(/^renderer-stopped$/gm) ?? []).length, 4);
    assert.equal((processes.match(/^Xvfb-stopped$/gm) ?? []).length, 4);
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

test("controlled Renderer session gives custom artwork a private validated copy", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-custom-artwork-session-test."),
  );
  const { environment, processLog, renderer } =
    await installRendererSessionFixture(directory, "focused");
  const snapshotPath = new URL(
    "../src/shared/fixtures/playing.json",
    import.meta.url,
  );
  const canonicalSnapshot = await readFile(snapshotPath);
  const customArtwork = Buffer.from("private artwork contents");
  const capture = {
    scenario: "playing",
    viewport: "1280x720",
    width: 1280,
    height: 720,
    typography: "automatic",
    diagnostics: false,
    finalCapturePath: path.join(directory, "playing.png"),
    snapshot: JSON.parse(canonicalSnapshot),
    customArtwork: { contents: customArtwork },
  };

  try {
    await executePresentationCapturePlan(
      { captures: [capture], sessions: [[capture]] },
      {
        sessionAdapter: createControlledRendererSessionAdapter({
          environment,
          publishCapture: publishPresentationCapture,
          rendererExecutable: renderer,
        }),
      },
    );

    const selection = JSON.parse(
      (await readFile(processLog, "utf8")).match(/^selection\|(.*)$/m)[1],
    );
    assert.equal(
      selection.observedArtworkHash,
      createHash("sha256").update(customArtwork).digest("hex").slice(0, 12),
    );
    assert.match(selection.snapshot.artwork.path, /custom-artwork-1$/);
    await assert.rejects(readFile(selection.snapshot.artwork.path), /ENOENT/);
    assert.deepEqual(await readFile(snapshotPath), canonicalSnapshot);
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

async function installRendererSessionFixture(directory, logStyle) {
  const binDirectory = path.join(directory, "bin");
  const pngDirectory = path.join(directory, "pngs");
  const processLog = path.join(directory, "processes");
  await mkdir(pngDirectory);
  await writeFile(
    path.join(pngDirectory, "1280x720.png"),
    presentationCapturePngHeader(1280, 720),
  );
  const { renderer } = await installPresentationCaptureFixtures(binDirectory);
  return {
    environment: {
      ...process.env,
      PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
      ROONSCAPE_CAPTURE_TEST_LOG_STYLE: logStyle,
      ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY: pngDirectory,
      ROONSCAPE_CAPTURE_TEST_PROCESS_LOG: processLog,
    },
    pngDirectory,
    processLog,
    renderer,
  };
}
