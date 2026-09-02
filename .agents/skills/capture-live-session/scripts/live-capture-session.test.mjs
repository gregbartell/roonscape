import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
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

const executeFile = promisify(execFile);
const scratchRoot = "/var/tmp/codex/roonscape";

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
