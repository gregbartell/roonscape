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

test(
  "concurrent native captures preserve neighboring painted revisions and evidence after cancellation",
  { timeout: 30_000 },
  async () => {
    const { createNativeSession, waitForNativeWindow } =
      await import("./native-session.mjs");
    const { installFixtureWorktree } =
      await import("./native-session-test-fixtures.mjs");
    const { runControlledRendererSession } =
      await import("./presentation-capture-renderer.mjs");
    const { assertProcessRunning } = await import("./process-harness.mjs");
    await mkdir("/var/tmp/codex/roonscape", { recursive: true });
    const directory = await mkdtemp("/var/tmp/codex/roonscape/task.");
    const sentinel = await createNativeSession({ width: 1280, height: 720 });
    try {
      const sentinelRenderer = sentinel.startProcess(
        path.resolve("target/debug/roonscape-renderer"),
        ["--config", sentinel.configurationPath],
        {
          ROONSCAPE_SOCKET: path.join(
            sentinel.runtimeDirectory,
            "missing.sock",
          ),
          ROONSCAPE_CAPTURE_VIEWPORT: "1280x720",
        },
      );
      await sentinelRenderer.spawned;
      const sentinelWindow = await waitForNativeWindow(
        sentinelRenderer,
        sentinel.environment,
        1280,
        720,
      );
      const executables = await Promise.all(
        ["a", "b"].map((name) =>
          installFixtureWorktree(path.join(directory, name)),
        ),
      );
      const snapshot = JSON.parse(
        await readFile(
          new URL("../src/shared/fixtures/playing.json", import.meta.url),
          "utf8",
        ),
      );
      const captures = [1280, 1600].map((width, index) =>
        ["playing", "paused"].map((scenario) => ({
          scenario,
          width,
          height: width === 1280 ? 720 : 900,
          viewport: width === 1280 ? "1280x720" : "1600x900",
          typography: "fallback",
          diagnostics: false,
          finalCapturePath: path.join(directory, `${index}-${scenario}.png`),
          snapshot: { ...snapshot, playback: scenario },
        })),
      );
      const controller = new AbortController();
      const firstReady = Promise.withResolvers();
      const secondReady = Promise.withResolvers();
      const cancellationComplete = Promise.withResolvers();
      const conflictingEnvironment = {
        ...sentinel.environment,
        DISPLAY: sentinel.environment.DISPLAY,
        GDK_BACKEND: "wayland",
        WAYLAND_DISPLAY: "conflicting-wayland",
        ROONSCAPE_SOCKET: "/missing/socket",
        ROONSCAPE_FIXTURE_AUTO_CLOSE_MS: "1",
        ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL: "/missing/paint.sock",
      };
      const first = runControlledRendererSession(captures[0], {
        rendererExecutable: executables[0],
        environment: conflictingEnvironment,
        signal: controller.signal,
        publishCapture: async (request) => {
          await publishPresentationCapture(request);
          firstReady.resolve();
          await secondReady.promise;
          controller.abort(new Error("cancel first native capture"));
        },
      })
        .then(
          () => assert.fail("cancelled session succeeded"),
          (error) => assert.match(error.message, /cancel first native capture/),
        )
        .finally(() => {
          firstReady.resolve();
          cancellationComplete.resolve();
        });
      const second = runControlledRendererSession(captures[1], {
        rendererExecutable: executables[1],
        environment: conflictingEnvironment,
        publishCapture: async (request) => {
          await firstReady.promise;
          await publishPresentationCapture(request);
          secondReady.resolve();
          await cancellationComplete.promise;
        },
      }).finally(() => secondReady.resolve());
      await Promise.all([first, second]);
      const retained = await readFile(captures[0][0].finalCapturePath);
      assert.equal(retained.readUInt32BE(16), 1280);
      await assert.rejects(readFile(captures[0][1].finalCapturePath), {
        code: "ENOENT",
      });
      const neighborFrames = await Promise.all(
        captures[1].map(async ({ finalCapturePath }) => {
          const png = await readFile(finalCapturePath);
          assert.equal(png.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
          assert.equal(png.readUInt32BE(16), 1600);
          assert.equal(png.readUInt32BE(20), 900);
          return png;
        }),
      );
      assert.notDeepEqual(
        neighborFrames[0],
        neighborFrames[1],
        "Playing and Paused must produce different painted PNGs",
      );
      assertProcessRunning(sentinelRenderer, "unrelated sentinel Renderer");
      assert.equal(
        await waitForNativeWindow(
          sentinelRenderer,
          sentinel.environment,
          1280,
          720,
        ),
        sentinelWindow,
      );
      assert.deepEqual(
        await readFile(captures[0][0].finalCapturePath),
        retained,
      );
    } finally {
      await sentinel.close();
      await rm(directory, { recursive: true, force: true });
    }
  },
);
