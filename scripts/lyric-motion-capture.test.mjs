import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

import {
  alignEntryFramesToPaint,
  assertPublicationChronology,
  buildLyricMotionCaptureEnvironment,
  buildLyricMotionCapturePlan,
  createLyricMotionReviewArtifacts,
  executeLyricMotionCapturePlan,
  expectedLyricPaintAtSeconds,
  lyricMotionReviewViewports,
  parseLyricMotionCaptureRequest,
  recordingElapsedSeconds,
  reanchorLyricMotionSnapshot,
  renderLyricMotionCaptureReadme,
} from "./lyric-motion-capture.mjs";

const executeFile = promisify(execFile);
const scratchRoot = "/var/tmp/codex/roonscape";

test("plans the maintained Reel Lift tour", () => {
  const request = parseLyricMotionCaptureRequest([
    "--example",
    "reel-lift-tour",
    "--resolution",
    "1280x720",
    "--output",
    "captures/lyrics",
  ]);

  assert.deepEqual(request, {
    example: "reel-lift-tour",
    output: "captures/lyrics",
    reducedAnimation: false,
    resolution: { width: 1280, height: 720 },
  });

  const plan = buildLyricMotionCapturePlan(request, {
    workingDirectory: "/workspace",
  });

  assert.equal(plan.example, "reel-lift-tour");
  assert.equal(plan.durationSeconds, 12);
  assert.equal(plan.initialFixture, "src/shared/fixtures/playing.json");
  assert.deepEqual(plan.publications, [
    {
      atSeconds: 0.5,
      fixture: "src/shared/fixtures/lyrics-reel-lift-tour.json",
      positionSeconds: 0,
    },
  ]);
  assert.equal(plan.outputDirectory, "/workspace/captures/lyrics");
  assert.deepEqual(
    plan.reviewFrames
      .filter(({ name }) => name.startsWith("natural-cue-handoff-"))
      .map(({ atSeconds, name }) => ({ atSeconds, name })),
    [
      { atSeconds: 3.25, name: "natural-cue-handoff-before" },
      { atSeconds: 3.4, name: "natural-cue-handoff-outgoing" },
      { atSeconds: 3.62, name: "natural-cue-handoff-midpoint" },
      { atSeconds: 3.78, name: "natural-cue-handoff-incoming" },
      { atSeconds: 3.95, name: "natural-cue-handoff-settled" },
    ],
  );
  assert.ok(
    plan.reviewFrames.some(({ name }) => name === "intentional-blank-settled"),
  );
  assert.ok(
    plan.reviewFrames.some(({ name }) => name === "lyrics-exit-settled"),
  );
});

test("capture request defaults to the smallest maintained viewport", () => {
  assert.deepEqual(parseLyricMotionCaptureRequest([]), {
    example: "reel-lift-tour",
    output: undefined,
    reducedAnimation: false,
    resolution: { width: 1280, height: 720 },
  });
});

test("maintains the complete peer viewport review matrix", () => {
  assert.deepEqual(lyricMotionReviewViewports, [
    { width: 1280, height: 720 },
    { width: 1600, height: 900 },
    { width: 1600, height: 1200 },
    { width: 1920, height: 1200 },
    { width: 2560, height: 1080 },
    { width: 3840, height: 2160 },
    { width: 3840, height: 2400 },
  ]);
  for (const { width, height } of lyricMotionReviewViewports) {
    const request = parseLyricMotionCaptureRequest([
      "--resolution",
      `${width}x${height}`,
    ]);
    assert.deepEqual(request.resolution, { width, height });
  }
});

test("plans wrapping progression and external seek examples", () => {
  const wrapping = buildLyricMotionCapturePlan(
    parseLyricMotionCaptureRequest([
      "--example",
      "wrapping-progression",
      "--resolution",
      "1600x1200",
    ]),
  );
  assert.equal(wrapping.durationSeconds, 14);
  assert.equal(
    wrapping.publications[0].fixture,
    "src/shared/fixtures/lyrics-wrapping-progression.json",
  );
  assert.ok(
    wrapping.reviewFrames.some(({ name }) => name === "capped-cue-settled"),
  );
  assert.match(
    wrapping.lineCountEvidence,
    /source 3 lines; destination 4 lines/,
  );

  const seek = buildLyricMotionCapturePlan(
    parseLyricMotionCaptureRequest(["--example", "external-seek"]),
  );
  assert.equal(seek.durationSeconds, 6.5);
  assert.deepEqual(seek.publications, [
    {
      atSeconds: 0.5,
      fixture: "src/shared/fixtures/lyrics-seek-tour.json",
      positionSeconds: 1.5,
    },
    {
      atSeconds: 3,
      fixture: "src/shared/fixtures/lyrics-seek-tour.json",
      positionSeconds: 6,
    },
    {
      atSeconds: 4.2,
      fixture: "src/shared/fixtures/lyrics-seek-tour.json",
      positionSeconds: 12,
    },
    {
      atSeconds: 5.4,
      fixture: "src/shared/fixtures/lyrics-seek-tour.json",
      positionSeconds: 1.5,
    },
  ]);
  assert.ok(
    seek.reviewFrames.some(({ name }) => name === "external-seek-result"),
  );
  assert.ok(
    seek.reviewFrames.some(({ name }) => name === "seek-past-composition-end"),
  );
  assert.ok(
    seek.reviewFrames.some(
      ({ name }) => name === "seek-across-composition-start",
    ),
  );
});

test("plans reduced-animation endpoint review for every maintained example", () => {
  for (const example of [
    "reel-lift-tour",
    "wrapping-progression",
    "timeline-edge-cases",
    "availability-reversal",
    "timeline-revision",
    "external-seek",
    "paused-handoff",
  ]) {
    const plan = buildLyricMotionCapturePlan(
      parseLyricMotionCaptureRequest([
        "--example",
        example,
        "--reduced-animation",
      ]),
    );
    assert.equal(plan.reducedAnimation, true);
    assert.ok(plan.reviewFrames.length > 0);
  }
});

test("plans interruption, Intentional Blank, and availability-reversal evidence", () => {
  const edgeCases = buildLyricMotionCapturePlan(
    parseLyricMotionCaptureRequest(["--example", "timeline-edge-cases"]),
  );
  assert.equal(
    edgeCases.publications[0].fixture,
    "src/shared/fixtures/lyrics-timeline-edge-cases.json",
  );
  for (const frame of [
    "leading-intentional-blank-settled",
    "consecutive-intentional-blank-settled",
    "identical-natural-cue-handoff",
    "interrupted-natural-cue-handoff-result",
    "long-gap-retains-cue",
    "trailing-intentional-blank-settled",
  ]) {
    assert.ok(edgeCases.reviewFrames.some(({ name }) => name === frame));
  }

  const availability = buildLyricMotionCapturePlan(
    parseLyricMotionCaptureRequest(["--example", "availability-reversal"]),
  );
  assert.equal(availability.publications.length, 4);
  assert.deepEqual(
    availability.publications.map(({ fixture }) => fixture),
    [
      "src/shared/fixtures/lyrics-seek-tour.json",
      "src/shared/fixtures/playing.json",
      "src/shared/fixtures/lyrics-seek-tour.json",
      "src/shared/fixtures/playing.json",
    ],
  );
  assert.ok(
    availability.reviewFrames.some(
      ({ name }) => name === "entry-exit-reversal",
    ),
  );

  const revision = buildLyricMotionCapturePlan(
    parseLyricMotionCaptureRequest(["--example", "timeline-revision"]),
  );
  assert.equal(revision.publications.length, 2);
  assert.ok(
    revision.reviewFrames.some(
      ({ name }) => name === "revised-current-installed",
    ),
  );
});

test("plans pause-during-handoff evidence", () => {
  const paused = buildLyricMotionCapturePlan(
    parseLyricMotionCaptureRequest(["--example", "paused-handoff"]),
  );
  assert.deepEqual(paused.publications.at(-1), {
    atSeconds: 3.7,
    fixture: "src/shared/fixtures/lyrics-reel-lift-tour-paused.json",
    positionSeconds: 3.68,
  });
  assert.ok(
    paused.reviewFrames.some(({ name }) => name === "paused-handoff-settled"),
  );
});

test("capture request rejects unknown examples and malformed viewports", () => {
  assert.throws(
    () => parseLyricMotionCaptureRequest(["--example", "unmaintained"]),
    /unknown lyric motion example: unmaintained/,
  );
  assert.throws(
    () => parseLyricMotionCaptureRequest(["--resolution", "wide"]),
    /WIDTHxHEIGHT/,
  );
  assert.throws(
    () => parseLyricMotionCaptureRequest(["--resolution", "1280x1280"]),
    /landscape/,
  );
});

test("reanchors scheduled publications without mutating their fixture", () => {
  const fixture = {
    revision: 40,
    playback: "playing",
    lyrics: { cues: [{ atSeconds: 1.5, text: "A quiet orbit fades" }] },
    timing: {
      position: {
        seconds: 171,
        sampledAt: "2026-08-15T19:20:00Z",
      },
      durationSeconds: 266,
    },
  };

  const snapshot = reanchorLyricMotionSnapshot(fixture, {
    positionSeconds: 0,
    revision: 3,
    sampledAt: "2026-09-02T20:00:00.000Z",
  });

  assert.equal(snapshot.revision, 3);
  assert.deepEqual(snapshot.timing.position, {
    seconds: 0,
    sampledAt: "2026-09-02T20:00:00.000Z",
  });
  assert.equal(snapshot.lyrics.cues[0].text, "A quiet orbit fades");
  assert.equal(fixture.revision, 40);
  assert.equal(fixture.timing.position.seconds, 171);
});

test("schedules publications from the recorder start clock, not delayed progress observations", () => {
  const recordingStartedAtMilliseconds = 10_000;
  assert.equal(
    recordingElapsedSeconds(recordingStartedAtMilliseconds, 10_500),
    0.5,
  );
});

test("rejects capture metadata when publication chronology drifts from the encoded plan", () => {
  assert.doesNotThrow(() =>
    assertPublicationChronology(
      [
        {
          atSeconds: 0.5,
          publishedAtSeconds: 0.46,
          expectedPaintAtSeconds: 1.3,
          acknowledgedAtSeconds: 1.32,
        },
      ],
      20,
    ),
  );
  assert.throws(
    () =>
      assertPublicationChronology(
        [{ atSeconds: 0.5, publishedAtSeconds: 0.7 }],
        20,
      ),
    /scheduled at 0\.50s was issued at 0\.70s/,
  );
  assert.throws(
    () =>
      assertPublicationChronology(
        [
          {
            atSeconds: 0.5,
            publishedAtSeconds: 0.5,
            expectedPaintAtSeconds: 1.3,
            acknowledgedAtSeconds: 2,
          },
        ],
        20,
      ),
    /expected near encoded frame 26 was acknowledged at frame 40/,
  );
});

test("maps expected lyric paints onto the encoded capture timeline", () => {
  const snapshot = {
    playback: "playing",
    lyrics: {
      cues: [
        { atSeconds: 1.5, text: "First" },
        { atSeconds: 7.5, text: "Last" },
      ],
    },
  };
  assert.equal(
    expectedLyricPaintAtSeconds(snapshot, {
      atSeconds: 0.5,
      positionSeconds: 0,
    }),
    0.8999999999999999,
  );
  assert.equal(
    expectedLyricPaintAtSeconds(snapshot, {
      atSeconds: 0.5,
      positionSeconds: 1.5,
    }),
    0.5,
  );
  assert.equal(
    expectedLyricPaintAtSeconds(snapshot, {
      atSeconds: 0.5,
      positionSeconds: 12,
    }),
    undefined,
  );
  const withLeadingBlanks = structuredClone(snapshot);
  withLeadingBlanks.lyrics.cues.unshift({ atSeconds: 0, text: "" });
  assert.equal(
    expectedLyricPaintAtSeconds(withLeadingBlanks, {
      atSeconds: 0.5,
      positionSeconds: 0,
    }),
    expectedLyricPaintAtSeconds(snapshot, {
      atSeconds: 0.5,
      positionSeconds: 0,
    }),
  );
  assert.equal(
    expectedLyricPaintAtSeconds(
      { playback: "playing", lyrics: { cues: [{ atSeconds: 0, text: "" }] } },
      { atSeconds: 0.5, positionSeconds: 0 },
    ),
    undefined,
  );
});

test("aligns entry review frames to the renderer's recorded paint", () => {
  const aligned = alignEntryFramesToPaint(
    [
      { atSeconds: 1.25, name: "lyrics-entry-before" },
      { atSeconds: 1.6, name: "lyrics-entry-midpoint" },
      { atSeconds: 3.62, name: "natural-cue-handoff-midpoint" },
    ],
    [
      {
        expectedPaintAtSeconds: 1.3,
        acknowledgedAtSeconds: 1.7,
      },
    ],
  );
  assert.deepEqual(
    aligned.map(({ atSeconds }) => atSeconds),
    [1.65, 1.99, 3.62],
  );
});

test("renders an artifact index as Fixture Mode evidence", () => {
  const readme = renderLyricMotionCaptureReadme({
    example: "reel-lift-tour",
    resolution: { width: 1280, height: 720 },
    framesPerSecond: 20,
    durationSeconds: 12,
    frames: [
      {
        atSeconds: 0.75,
        fileName: "00-ordinary-now-playing.png",
        observation: "Ordinary Now Playing before lyrics enter.",
      },
      {
        atSeconds: 1.55,
        fileName: "01-lyrics-entry-midpoint.png",
        observation: "Midpoint of lyric composition entry.",
      },
    ],
  });

  assert.match(readme, /Captured from RoonScape Fixture Mode at 1280x720/);
  assert.match(readme, /losslessly at 20 frames per second/);
  assert.match(readme, /00-ordinary-now-playing\.png` \| R\+000\.75s/);
  assert.match(readme, /review targets, not automated visual verdicts/);
});

test("creates complete lyric motion review artifacts", async (context) => {
  await mkdir(scratchRoot, { recursive: true });
  const sessionDirectory = await mkdtemp(
    path.join(scratchRoot, "task.lyric-capture-test."),
  );
  context.after(() => rm(sessionDirectory, { force: true, recursive: true }));
  await executeFile("ffmpeg", [
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "testsrc=size=1280x720:rate=20:duration=0.5",
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
      status: "recorded",
      durationSeconds: 0.5,
      framesPerSecond: 20,
      resolution: { width: 1280, height: 720 },
    })}\n`,
  );
  const plan = {
    example: "artifact-test",
    resolution: { width: 1280, height: 720 },
    durationSeconds: 0.5,
    initialFixture: "initial.json",
    publications: [],
    reviewFrames: [
      reviewFrame(0, "before", "Initial test frame."),
      reviewFrame(0.2, "during", "Middle test frame."),
      reviewFrame(0.45, "after", "Final test frame."),
    ],
  };

  const artifacts = await createLyricMotionReviewArtifacts(
    sessionDirectory,
    plan,
  );

  assert.equal(artifacts.frames.length, 3);
  await readFile(path.join(sessionDirectory, "overview.png"));
  await readFile(
    path.join(sessionDirectory, "review", "full-rate-page-001.png"),
  );
  await assert.rejects(access(path.join(sessionDirectory, "candidates")), {
    code: "ENOENT",
  });
  const readme = await readFile(
    path.join(sessionDirectory, "README.md"),
    "utf8",
  );
  assert.match(readme, /Fixture Mode/);
  const manifest = JSON.parse(
    await readFile(path.join(sessionDirectory, "manifest.json"), "utf8"),
  );
  assert.equal(manifest.example, "artifact-test");
  assert.deepEqual(
    manifest.frames.map(({ fileName }) => fileName),
    ["00-before.png", "01-during.png", "02-after.png"],
  );
});

test("builds an exact dynamic Fixture Mode renderer environment", () => {
  const environment = buildLyricMotionCaptureEnvironment(
    {
      KEEP: "yes",
      ROONSCAPE_CAPTURE_CONTROL: "/capture.sock",
      ROONSCAPE_FIXTURE: "inherited.json",
      ROONSCAPE_FIXTURE_CONTROL: "/navigation.sock",
      ROONSCAPE_STATIC_FIXTURE: "1",
    },
    ":97",
    { width: 1280, height: 720 },
    "/runtime/roonscape.sock",
  );

  assert.equal(environment.KEEP, "yes");
  assert.equal(environment.DISPLAY, ":97");
  assert.equal(environment.GDK_BACKEND, "x11");
  assert.equal(environment.ROONSCAPE_CAPTURE_VIEWPORT, "1280x720");
  assert.equal(environment.ROONSCAPE_SOCKET, "/runtime/roonscape.sock");
  assert.equal(environment.ROONSCAPE_WINDOWED, "1");
  assert.equal(environment.ROONSCAPE_CAPTURE_CONTROL, undefined);
  assert.equal(environment.ROONSCAPE_FIXTURE, undefined);
  assert.equal(environment.ROONSCAPE_FIXTURE_CONTROL, undefined);
  assert.equal(environment.ROONSCAPE_STATIC_FIXTURE, undefined);
  assert.equal(environment.ROONSCAPE_CAPTURE_REDUCED_ANIMATION, undefined);
});

test("builds a reduced-animation dynamic Fixture Mode environment", () => {
  const environment = buildLyricMotionCaptureEnvironment(
    {},
    ":97",
    { width: 1280, height: 720 },
    "/runtime/roonscape.sock",
    true,
  );

  assert.equal(environment.ROONSCAPE_STATIC_FIXTURE, undefined);
  assert.equal(environment.ROONSCAPE_CAPTURE_REDUCED_ANIMATION, "1");
});

test("executes a capture plan through its session adapter", async () => {
  const plan = { example: "reel-lift-tour" };
  const received = [];
  const output = await executeLyricMotionCapturePlan(plan, {
    sessionAdapter: {
      async execute(candidate) {
        received.push(candidate);
        return "/captures/reel-lift-tour";
      },
    },
  });

  assert.deepEqual(received, [plan]);
  assert.equal(output, "/captures/reel-lift-tour");
});

function reviewFrame(atSeconds, name, observation) {
  return { atSeconds, name, observation };
}
