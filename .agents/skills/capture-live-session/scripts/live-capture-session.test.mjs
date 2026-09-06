import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { randomUUID } from "node:crypto";
import test from "node:test";
import { promisify } from "node:util";

import {
  buildLiveEnvironment,
  extractCandidates,
  finalizeSession,
  formatRelativeTimestamp,
  inspectRecordedFrame,
  parseRecordOptions,
  parseResolution,
  publishSession,
  retractPublication,
  reviewSession,
  renderReadme,
  validateSelection,
} from "./live-capture-session.mjs";

import { findExecutable } from "../../../../scripts/native-test-environment.mjs";

const executeFile = promisify(execFile);
const scratchRoot = "/var/tmp/codex/roonscape";

test("record options forward an explicit Roon Server Host", () => {
  const options = parseRecordOptions([
    "--event",
    "lyrics begin",
    "--roon-server",
    "roon-server.example",
  ]);

  assert.equal(options.roonServerHost, "roon-server.example");
});

test("resolution validation keeps captures small but supports overrides", () => {
  assert.deepEqual(parseResolution("1280x720"), { width: 1280, height: 720 });
  assert.deepEqual(parseResolution("1920x1200"), {
    width: 1920,
    height: 1200,
  });
  assert.throws(() => parseResolution("960x540"), /at least 1280x720/);
  assert.throws(() => parseResolution("1280x1280"), /must be landscape/);
});

test("Live Mode environment defaults to windowed and strips fixture controls", () => {
  const environment = buildLiveEnvironment(
    {
      KEEP: "yes",
      ROONSCAPE_CAPTURE_CONTROL: "/capture",
      ROONSCAPE_FIXTURE: "fixture.json",
      ROONSCAPE_STATIC_FIXTURE: "1",
    },
    ":91",
    { width: 1280, height: 720 },
    false,
  );

  assert.equal(environment.KEEP, "yes");
  assert.equal(environment.DISPLAY, ":91");
  assert.equal(environment.ROONSCAPE_WINDOWED, "1");
  assert.equal(environment.ROONSCAPE_CAPTURE_VIEWPORT, "1280x720");
  assert.equal(environment.ROONSCAPE_CAPTURE_CONTROL, undefined);
  assert.equal(environment.ROONSCAPE_FIXTURE, undefined);
  assert.equal(environment.ROONSCAPE_STATIC_FIXTURE, undefined);
});

test("fullscreen Live Mode leaves viewport selection to the display", () => {
  const environment = buildLiveEnvironment(
    {
      ROONSCAPE_CAPTURE_VIEWPORT: "1600x900",
      ROONSCAPE_WINDOWED: "1",
    },
    ":92",
    { width: 1280, height: 720 },
    true,
  );

  assert.equal(environment.ROONSCAPE_CAPTURE_VIEWPORT, undefined);
  assert.equal(environment.ROONSCAPE_WINDOWED, undefined);
});

test("relative timestamps use fixed hundredths", () => {
  assert.equal(formatRelativeTimestamp(0), "T+000.00s");
  assert.equal(formatRelativeTimestamp(1.25), "T+001.25s");
  assert.equal(formatRelativeTimestamp(12), "T+012.00s");
  assert.equal(formatRelativeTimestamp(-0.05), "T−000.05s");
});

test("complete selection requires ordered pre-event and concluding frames", () => {
  const state = { durationSeconds: 4 };
  assert.throws(
    () =>
      validateSelection(
        {
          title: "Event",
          complete: true,
          summary: "Summary",
          frames: [{ at: 1, name: "only", observation: "Only one frame." }],
        },
        state,
      ),
    /requires pre-event and concluding frames/,
  );
  assert.throws(
    () =>
      validateSelection(
        {
          title: "Event",
          complete: true,
          summary: "Summary",
          frames: [
            { at: 2, name: "later", observation: "Later." },
            { at: 1, name: "earlier", observation: "Earlier." },
          ],
        },
        state,
      ),
    /strictly increasing/,
  );
});

test("README anchors time to the first retained frame", () => {
  const readme = renderReadme(
    {
      title: "Track A → Track B",
      complete: true,
      summary: "Metadata precedes artwork.",
      frames: [],
    },
    {
      fullscreen: false,
      resolution: { width: 1280, height: 720 },
      date: "2026-09-02",
    },
    [
      {
        at: 3.25,
        fileName: "00-track-a.png",
        observation: "Track A is stable.",
      },
      {
        at: 4.55,
        fileName: "01-track-b.png",
        observation: "Track B appears.",
      },
    ],
  );

  assert.match(readme, /windowed mode at 1280x720/);
  assert.match(readme, /00-track-a\.png` \| T\+000\.00s/);
  assert.match(readme, /01-track-b\.png` \| T\+001\.30s/);
  assert.match(readme, /Metadata precedes artwork\./);
});

test("candidate extraction keeps meaningful changes and a final stability frame", async (context) => {
  await mkdir(scratchRoot, { recursive: true });
  const sessionDirectory = await mkdtemp(path.join(scratchRoot, "task.test."));
  context.after(() => rm(sessionDirectory, { force: true, recursive: true }));
  await executeFile("ffmpeg", [
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "color=c=red:size=1280x720:rate=20:duration=0.5",
    "-f",
    "lavfi",
    "-i",
    "color=c=blue:size=1280x720:rate=20:duration=0.5",
    "-filter_complex",
    "[0:v][1:v]concat=n=2:v=1:a=0",
    "-c:v",
    "ffv1",
    "-level",
    "3",
    "-g",
    "1",
    "-y",
    path.join(sessionDirectory, "capture.mkv"),
  ]);

  const candidates = await extractCandidates(sessionDirectory, {
    durationSeconds: 1,
  });

  assert.equal(candidates[0].capturedSeconds, 0);
  assert.ok(
    candidates.some(({ capturedSeconds }) => capturedSeconds === 0.5),
    JSON.stringify(candidates),
  );
  assert.ok(
    candidates.some(({ capturedSeconds }) => capturedSeconds === 0.45),
    JSON.stringify(candidates),
  );
  assert.equal(candidates.at(-1).capturedSeconds, 0.95);
  assert.equal(candidates.length, 4, JSON.stringify(candidates));

  await writeFile(
    path.join(sessionDirectory, "session.json"),
    `${JSON.stringify({
      status: "recorded",
      durationSeconds: 1,
      framesPerSecond: 20,
      resolution: { width: 1280, height: 720 },
    })}\n`,
  );
  const { fullRatePages } = await reviewSession(sessionDirectory);
  assert.equal(fullRatePages.length, 1);
  const reviewIndex = JSON.parse(
    await readFile(
      path.join(sessionDirectory, "review", "review-index.json"),
      "utf8",
    ),
  );
  assert.deepEqual(reviewIndex.pages, [
    {
      file: "full-rate-page-001.png",
      firstFrame: 0,
      count: 20,
      columns: 10,
      startSeconds: 0,
    },
  ]);
  const inspectionPath = await inspectRecordedFrame(sessionDirectory, "0.5");
  await readFile(inspectionPath);
});

test("publication writes frames, timeline, and timestamped overview without overwriting", async (context) => {
  await mkdir(scratchRoot, { recursive: true });
  const sessionDirectory = await mkdtemp(path.join(scratchRoot, "task.test."));
  const unique = `skill-test-${randomUUID()}`;
  const date = "2099-01-02";
  const collision = path.join(scratchRoot, `${unique}-${date}`);
  let outputDirectory;
  context.after(async () => {
    await rm(sessionDirectory, { force: true, recursive: true });
    await rm(collision, { force: true, recursive: true });
    if (outputDirectory !== undefined) {
      await rm(outputDirectory, { force: true, recursive: true });
    }
  });

  await executeFile("ffmpeg", [
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "testsrc=size=1280x720:rate=20:duration=0.25",
    "-c:v",
    "ffv1",
    "-level",
    "3",
    "-g",
    "1",
    "-y",
    path.join(sessionDirectory, "capture.mkv"),
  ]);
  await writeFile(
    path.join(sessionDirectory, "session.json"),
    `${JSON.stringify({
      version: 1,
      status: "recorded",
      event: "Skill test",
      eventSlug: unique,
      date,
      resolution: { width: 1280, height: 720 },
      fullscreen: false,
      framesPerSecond: 20,
      durationSeconds: 0.25,
    })}\n`,
  );
  const selectionPath = path.join(sessionDirectory, "selection.json");
  await writeFile(
    selectionPath,
    `${JSON.stringify({
      title: "Synthetic transition",
      complete: true,
      summary: "The synthetic source remains valid.",
      frames: [
        { at: 0, name: "before", observation: "Initial frame." },
        { at: 0.1, name: "after", observation: "Concluding frame." },
      ],
    })}\n`,
  );
  await mkdir(collision);

  outputDirectory = await publishSession(sessionDirectory, selectionPath);

  await readFile(path.join(sessionDirectory, "capture.mkv"));
  await retractPublication(sessionDirectory);
  await assert.rejects(readFile(path.join(outputDirectory, "README.md")));
  outputDirectory = await publishSession(sessionDirectory, selectionPath);

  assert.equal(outputDirectory, `${collision}-02`);
  const readme = await readFile(
    path.join(outputDirectory, "README.md"),
    "utf8",
  );
  assert.match(readme, /T\+000\.10s/);
  await readFile(path.join(outputDirectory, "00-before.png"));
  await readFile(path.join(outputDirectory, "01-after.png"));
  const { stdout } = await executeFile("ffprobe", [
    "-v",
    "error",
    "-select_streams",
    "v:0",
    "-show_entries",
    "stream=width,height",
    "-of",
    "csv=p=0:s=x",
    path.join(outputDirectory, "overview.png"),
  ]);
  assert.equal(stdout.trim(), "1928x216");
  await finalizeSession(sessionDirectory);
  await assert.rejects(readFile(path.join(sessionDirectory, "session.json")));
});

for (const recorderFailure of [false, true]) {
  test(
    recorderFailure
      ? "recorder failure prevents application startup"
      : "record captures a baseline before application startup",
    async (context) => {
      await mkdir(scratchRoot, { recursive: true });
      const directory = await mkdtemp(path.join(scratchRoot, "task.test."));
      let sessionDirectory;
      context.after(async () => {
        await rm(directory, { recursive: true, force: true });
        if (sessionDirectory !== undefined)
          await rm(sessionDirectory, { recursive: true, force: true });
      });
      const source = path.join(directory, "source");
      const helperDirectory = path.join(
        source,
        ".agents/skills/capture-live-session/scripts",
      );
      const bin = path.join(directory, "bin");
      for (const subdirectory of [
        helperDirectory,
        bin,
        path.join(source, "scripts"),
        path.join(source, "src/launcher"),
      ])
        await mkdir(subdirectory, { recursive: true });
      await cp(
        new URL("./live-capture-session.mjs", import.meta.url),
        path.join(helperDirectory, "live-capture-session.mjs"),
      );
      await cp(
        new URL("../../../../scripts/process-harness.mjs", import.meta.url),
        path.join(source, "scripts/process-harness.mjs"),
      );
      await writeFile(
        path.join(directory, "display.json"),
        JSON.stringify({ trackedOutputId: "test-output" }),
      );
      await writeFile(
        path.join(directory, "authorization.json"),
        JSON.stringify({ synthetic: true }),
      );
      const ffmpegPath = findExecutable("ffmpeg");
      const video = path.join(directory, "synthetic.mkv");
      await executeFile(ffmpegPath, [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:size=1280x720:rate=20:duration=0.15",
        "-c:v",
        "ffv1",
        "-level",
        "3",
        "-g",
        "1",
        video,
      ]);
      // Exercise the real record command with synthetic native boundaries.
      // The delayed first frame must precede application launch; process spawn
      // or an initial zero-frame progress report is not sufficient readiness.
      const stub = `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const directory = process.env.CAPTURE_TEST_DIRECTORY;
const trace = (event) => fs.appendFileSync(path.join(directory, "trace"), event + "\\n");
switch (path.basename(process.argv[1])) {
  case "npm": case "cargo": break;
  case "Xvfb":
    process.stdout.write("91\\n");
    setInterval(() => {}, 1000);
    break;
  case "xwininfo":
    process.stdout.write("Width: 1280\\nHeight: 720\\n");
    break;
  case "roonscape":
    trace("application-started");
    setTimeout(() => {
      const session = fs.readFileSync(path.join(directory, "session-path"), "utf8");
      fs.writeFileSync(path.join(session, "stop-requested"), "");
    }, 800);
    setInterval(() => {}, 1000);
    break;
  case "ffmpeg":
    if (!process.argv.includes("x11grab")) {
      const result = spawnSync(process.env.CAPTURE_TEST_FFMPEG, process.argv.slice(2), { stdio: "inherit" });
      process.exit(result.status ?? 1);
    }
    trace("recorder-started");
    if (process.env.CAPTURE_TEST_RECORDER_FAILURE === "1") process.exit(17);
    fs.writeFileSync(path.join(directory, "session-path"), path.dirname(process.argv.at(-1)));
    fs.copyFileSync(path.join(directory, "synthetic.mkv"), process.argv.at(-1));
    process.stdout.write("frame=0\\nprogress=continue\\n");
    setTimeout(() => {
      trace("baseline-captured");
      process.stdout.write("frame=1\\nprogress=continue\\n");
    }, 600);
    setInterval(() => {}, 1000);
    break;
}
`;
      for (const name of [
        "npm",
        "cargo",
        "Xvfb",
        "xwininfo",
        "ffmpeg",
        "roonscape",
      ]) {
        await writeFile(
          name === "roonscape"
            ? path.join(source, "src/launcher/roonscape")
            : path.join(bin, name),
          stub,
          { mode: 0o755 },
        );
      }
      let result;
      try {
        result = await executeFile(
          process.execPath,
          [
            path.join(helperDirectory, "live-capture-session.mjs"),
            "record",
            "--event",
            "synthetic startup",
            "--config",
            path.join(directory, "display.json"),
          ],
          {
            cwd: source,
            env: {
              ...process.env,
              PATH: `${bin}${path.delimiter}${process.env.PATH}`,
              XDG_RUNTIME_DIR: path.join(directory, "runtime"),
              ROONSCAPE_AUTHORIZATION_FILE: path.join(
                directory,
                "authorization.json",
              ),
              CAPTURE_TEST_DIRECTORY: directory,
              CAPTURE_TEST_FFMPEG: ffmpegPath,
              CAPTURE_TEST_RECORDER_FAILURE: recorderFailure ? "1" : "0",
            },
            timeout: 15_000,
          },
        );
      } catch (error) {
        result = error;
      }
      const events = result.stdout
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      sessionDirectory = events.find(({ type }) => type === "session")?.path;
      const trace = (await readFile(path.join(directory, "trace"), "utf8"))
        .trim()
        .split("\n");
      if (recorderFailure) {
        assert.equal(result.code, 1, result.stderr);
        assert.deepEqual(trace, ["recorder-started"]);
        assert.ok(!events.some(({ type }) => type === "runtime-ready"));
      } else {
        assert.equal(result.code, undefined, result.stderr);
        assert.deepEqual(trace, [
          "recorder-started",
          "baseline-captured",
          "application-started",
        ]);
        assert.equal(events.at(-1).type, "recorded");
      }
    },
  );
}
