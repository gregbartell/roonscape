import { once } from "node:events";
import { constants as fileConstants } from "node:fs";
import {
  access,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  rmdir,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  formatRelativeTimestamp,
  parseResolution,
} from "../.agents/skills/capture-live-session/scripts/live-capture-session.mjs";
import { loadPresentationCaptureSnapshot } from "./presentation-snapshot.mjs";
import { createNativeSession, waitForNativeWindow } from "./native-session.mjs";
import {
  assertProcessRunning,
  processFailure,
  runMonitoredProcess,
  startMonitoredProcess,
  stopProcess,
  waitFor,
} from "./process-harness.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const scratchRoot = "/var/tmp/codex/roonscape";
const framesPerSecond = 20;
const lyricEntryLeadSeconds = 1.1;
const lyricFinalHoldSeconds = 3;
export const lyricMotionReviewViewports = Object.freeze([
  Object.freeze({ width: 1280, height: 720 }),
  Object.freeze({ width: 1600, height: 900 }),
  Object.freeze({ width: 1600, height: 1200 }),
  Object.freeze({ width: 1920, height: 1200 }),
  Object.freeze({ width: 2560, height: 1080 }),
  Object.freeze({ width: 3840, height: 2160 }),
  Object.freeze({ width: 3840, height: 2400 }),
]);
const defaultResolution = lyricMotionReviewViewports[0];
const defaultRendererExecutable = path.join(
  repositoryRoot,
  "target/debug/roonscape-renderer",
);

const examples = {
  "short-blanks": {
    durationSeconds: 22,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-short-blanks.json",
        positionSeconds: 0,
      },
    ],
    reviewFrames: [
      reviewFrame(3.6, "no-blank-baseline", "Uninterrupted compact promotion."),
      reviewFrame(
        5.7,
        "100ms-blank",
        "Normal promotion across a 100 ms blank.",
      ),
      reviewFrame(
        7.9,
        "300ms-blank",
        "Normal promotion across the selected 300 ms example.",
      ),
      reviewFrame(
        10.1,
        "500ms-blank",
        "Normal promotion across a 500 ms blank.",
      ),
      reviewFrame(
        14.4,
        "tall-to-short-300ms-blank",
        "A tall cue departs across a short blank without a forced dwell.",
      ),
      reviewFrame(
        17,
        "longer-blank-context",
        "A longer blank settles with context and an empty focal position.",
      ),
      reviewFrame(
        17.5,
        "longer-blank-promotion",
        "Anticipation lifts out of the longer blank.",
      ),
      reviewFrame(
        21.5,
        "single-final-exit",
        "Ordinary Now Playing returns after the final hold.",
      ),
    ],
  },
  "blank-lifecycle": {
    durationSeconds: 17,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-all-blank.json",
        positionSeconds: 0,
      },
      {
        atSeconds: 2,
        fixture: "src/shared/fixtures/lyrics-long-blank.json",
        positionSeconds: 6,
      },
      {
        atSeconds: 4,
        fixture: "src/shared/fixtures/playing.json",
        positionSeconds: 8,
      },
      {
        atSeconds: 4.2,
        fixture: "src/shared/fixtures/lyrics-long-blank.json",
        positionSeconds: 50,
      },
      {
        atSeconds: 6,
        fixture: "src/shared/fixtures/lyrics-long-blank.json",
        positionSeconds: 94,
      },
    ],
    reviewFrames: [
      reviewFrame(
        1.5,
        "all-blank-ordinary",
        "An all-blank timeline retains ordinary Now Playing.",
      ),
      reviewFrame(
        2.3,
        "late-entry-into-blank",
        "Artwork dominates entry directly into a settled blank.",
      ),
      reviewFrame(
        3,
        "settled-blank-context",
        "Both neighbors remain contextual around an empty focal position.",
      ),
      reviewFrame(
        4.4,
        "blank-entry-reversal",
        "Timeline return reverses exit into a long internal blank.",
      ),
      reviewFrame(
        5.5,
        "long-blank-context",
        "The 90-second blank retains the same composition and context.",
      ),
      reviewFrame(
        6.7,
        "advance-out-of-blank",
        "Normal anticipation out of the internal blank after a seek.",
      ),
      reviewFrame(
        12.8,
        "trailing-blank-context",
        "Trailing blanks retain Previous Cue and no Next Cue.",
      ),
      reviewFrame(
        14,
        "consecutive-trailing-blank",
        "Consecutive trailing blanks do not restart motion.",
      ),
      reviewFrame(
        16.3,
        "final-entry-hold-exit",
        "One exit after the hold following the final blank entry.",
      ),
    ],
  },
  "reel-lift-tour": {
    durationSeconds: 12,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-reel-lift-tour.json",
        positionSeconds: 0,
      },
    ],
    reviewFrames: [
      reviewFrame(
        0.4,
        "ordinary-now-playing",
        "Ordinary Now Playing before lyrics enter.",
      ),
      reviewFrame(
        1.25,
        "lyrics-entry-before",
        "Last frame expected before lyric entry.",
      ),
      reviewFrame(1.35, "lyrics-entry-early", "Early lyric composition entry."),
      reviewFrame(
        1.6,
        "lyrics-entry-midpoint",
        "Midpoint of lyric composition entry.",
      ),
      reviewFrame(
        1.85,
        "lyrics-entry-settled",
        "First lyric composition settled.",
      ),
      reviewFrame(
        3.25,
        "natural-cue-handoff-before",
        "First cue before the Natural Cue Handoff.",
      ),
      reviewFrame(
        3.4,
        "natural-cue-handoff-outgoing",
        "Outgoing first cue during the Natural Cue Handoff.",
      ),
      reviewFrame(
        3.62,
        "natural-cue-handoff-midpoint",
        "Incoming cue owns focus at the midpoint of the Natural Cue Handoff.",
      ),
      reviewFrame(
        3.78,
        "natural-cue-handoff-incoming",
        "Incoming second cue during the Natural Cue Handoff.",
      ),
      reviewFrame(
        3.95,
        "natural-cue-handoff-settled",
        "Second cue settled after the Natural Cue Handoff.",
      ),
      reviewFrame(
        5.25,
        "intentional-blank-before",
        "Second cue before the Intentional Blank.",
      ),
      reviewFrame(
        5.55,
        "intentional-blank-midpoint",
        "Transition into the Intentional Blank.",
      ),
      reviewFrame(
        5.85,
        "intentional-blank-settled",
        "Intentional Blank settled.",
      ),
      reviewFrame(
        7.25,
        "intentional-blank-exit-before",
        "Intentional Blank before the next cue.",
      ),
      reviewFrame(
        7.55,
        "intentional-blank-exit-midpoint",
        "Transition out of the Intentional Blank.",
      ),
      reviewFrame(
        7.85,
        "intentional-blank-exit-settled",
        "Final lyric cue settled.",
      ),
      reviewFrame(
        10.95,
        "lyrics-exit-before",
        "Final lyric cue before composition exit.",
      ),
      reviewFrame(
        11.35,
        "lyrics-exit-midpoint",
        "Midpoint of lyric composition exit.",
      ),
      reviewFrame(
        11.7,
        "lyrics-exit-settled",
        "Ordinary Now Playing restored after lyrics.",
      ),
    ],
  },
  "wrapping-progression": {
    durationSeconds: 14,
    lineCountEvidenceByResolution: {
      "1600x900":
        "Native Pango measurement: height-aware source 2 lines; destination 3 lines.",
      "1600x1200":
        "Native Pango measurement: height-aware source 3 lines; destination 4 lines.",
    },
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-wrapping-progression.json",
        positionSeconds: 0,
      },
    ],
    reviewFrames: [
      reviewFrame(
        0.4,
        "ordinary-now-playing",
        "Ordinary Now Playing before lyrics enter.",
      ),
      reviewFrame(
        1.55,
        "one-line-cue-entry",
        "One-line cue during lyric composition entry.",
      ),
      reviewFrame(
        1.85,
        "one-line-cue-settled",
        "One-line cue and both neighbors settled.",
      ),
      reviewFrame(
        3.01,
        "one-to-one-midpoint",
        "Natural Cue Handoff between one-line cues.",
      ),
      reviewFrame(
        3.4,
        "one-to-one-settled",
        "Second one-line cue hierarchy settled.",
      ),
      reviewFrame(
        4.21,
        "short-to-longer-midpoint",
        "Natural Cue Handoff from a short cue to a wrapping cue.",
      ),
      reviewFrame(4.6, "longer-cue-settled", "Wrapping cue hierarchy settled."),
      reviewFrame(
        5.41,
        "longer-to-short-midpoint",
        "Natural Cue Handoff from a wrapping cue to a short cue.",
      ),
      reviewFrame(
        6.61,
        "short-to-next-wrapping-midpoint",
        "Natural Cue Handoff toward the next wrapping cue.",
      ),
      reviewFrame(
        7.81,
        "wrapping-to-wrapping-midpoint",
        "Two wrapping cues exchange focal ownership.",
      ),
      reviewFrame(
        9.01,
        "short-to-height-aware-midpoint",
        "Natural Cue Handoff into a taller Pango-wrapped cue.",
      ),
      reviewFrame(
        9.4,
        "height-aware-cue-settled",
        "Taller focal cue settled without Previous Cue.",
      ),
      reviewFrame(
        10.21,
        "height-aware-to-short-midpoint",
        "Taller cue leaving through the abbreviated path.",
      ),
      reviewFrame(
        11.41,
        "short-to-capped-midpoint",
        "Natural Cue Handoff into the capped-height cue.",
      ),
      reviewFrame(
        11.8,
        "capped-cue-settled",
        "Capped-height focal cue settled without neighbors.",
      ),
      reviewFrame(
        12.61,
        "capped-to-short-midpoint",
        "Capped-height cue leaving through the abbreviated path.",
      ),
      reviewFrame(
        13,
        "final-short-settled",
        "Short destination cue settled after a capped-height source.",
      ),
    ],
  },
  "timeline-edge-cases": {
    durationSeconds: 12,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-timeline-edge-cases.json",
        positionSeconds: 0,
      },
    ],
    reviewFrames: [
      reviewFrame(
        2.5,
        "leading-intentional-blank-settled",
        "Leading blanks retain ordinary Now Playing until preparation for the first nonblank cue.",
      ),
      reviewFrame(
        3,
        "consecutive-intentional-blank-settled",
        "Consecutive leading blanks do not advance composition entry.",
      ),
      reviewFrame(
        4.3,
        "first-cue-settled",
        "First nonblank cue after the leading Intentional Blanks.",
      ),
      reviewFrame(
        4.98,
        "identical-natural-cue-handoff",
        "Identical adjacent text still performs Reel Lift.",
      ),
      reviewFrame(
        5.15,
        "interrupted-natural-cue-handoff-result",
        "A newer cue wins before the identical-text lift can settle.",
      ),
      reviewFrame(
        5.95,
        "middle-intentional-blank-midpoint",
        "A middle Intentional Blank retains context around an empty focal position.",
      ),
      reviewFrame(
        6.95,
        "intentional-blank-exit-midpoint",
        "A nonblank cue returns from the Intentional Blank interval.",
      ),
      reviewFrame(
        9.8,
        "long-gap-retains-cue",
        "An unmarked timestamp gap retains the selected cue.",
      ),
      reviewFrame(
        11.55,
        "trailing-intentional-blank-settled",
        "A trailing Intentional Blank remains inside the Synchronized Lyric Composition.",
      ),
    ],
  },
  "availability-reversal": {
    durationSeconds: 5.5,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-seek-tour.json",
        positionSeconds: 1.5,
      },
      {
        atSeconds: 2.4,
        fixture: "src/shared/fixtures/playing.json",
        positionSeconds: 1.5,
      },
      {
        atSeconds: 2.7,
        fixture: "src/shared/fixtures/lyrics-seek-tour.json",
        positionSeconds: 1.5,
      },
      {
        atSeconds: 4.4,
        fixture: "src/shared/fixtures/playing.json",
        positionSeconds: 1.5,
      },
    ],
    reviewFrames: [
      reviewFrame(
        0.75,
        "late-timeline-entry",
        "Late lyric availability begins in-place entry.",
      ),
      reviewFrame(
        1.2,
        "late-timeline-settled",
        "Late lyric composition settled.",
      ),
      reviewFrame(
        2.55,
        "timeline-loss-exit",
        "Lyric availability loss begins in-place exit.",
      ),
      reviewFrame(
        2.85,
        "entry-exit-reversal",
        "Availability returns before exit settles.",
      ),
      reviewFrame(
        3.35,
        "reversal-settled",
        "Reversed composition entry settles on lyrics.",
      ),
      reviewFrame(
        4.7,
        "final-timeline-loss",
        "Final availability loss returns toward ordinary metadata.",
      ),
      reviewFrame(
        5.1,
        "ordinary-restored",
        "Ordinary Now Playing restored after lyric loss.",
      ),
    ],
  },
  "timeline-revision": {
    durationSeconds: 3.6,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-revision-before.json",
        positionSeconds: 1.5,
      },
      {
        atSeconds: 2.4,
        fixture: "src/shared/fixtures/lyrics-revision-after.json",
        positionSeconds: 1.5,
      },
    ],
    reviewFrames: [
      reviewFrame(
        1.3,
        "original-current-settled",
        "Original timeline entry is focal.",
      ),
      reviewFrame(
        2.35,
        "timeline-revision-before",
        "Stable frame immediately before correction.",
      ),
      reviewFrame(
        2.55,
        "revised-current-installed",
        "Corrected current cue installs directly.",
      ),
      reviewFrame(
        2.9,
        "revision-remains-settled",
        "Corrected timeline remains at its endpoint.",
      ),
    ],
  },
  "external-seek": {
    durationSeconds: 6.5,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
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
    ],
    reviewFrames: [
      reviewFrame(
        0.35,
        "ordinary-before-boundary-seek",
        "Ordinary Now Playing before the seek across the first cue boundary.",
      ),
      reviewFrame(
        0.65,
        "seek-across-composition-start-entry",
        "The seek across the first cue boundary begins lyric composition entry.",
      ),
      reviewFrame(
        1.2,
        "first-cue-settled",
        "The first cue settles directly after the boundary seek.",
      ),
      reviewFrame(
        2.85,
        "external-seek-before",
        "Stable frame immediately before the external seek window.",
      ),
      reviewFrame(
        3.15,
        "external-seek-result",
        "First expected frame after the external seek.",
      ),
      reviewFrame(
        3.35,
        "external-seek-settled",
        "Skipped-cue destination settled.",
      ),
      reviewFrame(
        4.35,
        "seek-past-composition-end",
        "A seek past the final hold begins lyric composition exit.",
      ),
      reviewFrame(
        4.9,
        "seek-past-composition-end-settled",
        "The after-lyrics destination is ordinary Now Playing.",
      ),
      reviewFrame(
        5.55,
        "seek-across-composition-start",
        "A seek back across the first cue begins lyric composition entry.",
      ),
      reviewFrame(
        6.15,
        "seek-across-composition-start-settled",
        "The destination cue is focal without cue travel.",
      ),
    ],
  },
  "paused-handoff": {
    durationSeconds: 5.2,
    initialFixture: "src/shared/fixtures/playing.json",
    publications: [
      {
        atSeconds: 0.5,
        fixture: "src/shared/fixtures/lyrics-reel-lift-tour.json",
        positionSeconds: 0,
      },
      {
        atSeconds: 3.7,
        fixture: "src/shared/fixtures/lyrics-reel-lift-tour-paused.json",
        positionSeconds: 3.68,
      },
    ],
    reviewFrames: [
      reviewFrame(
        3.55,
        "handoff-before-pause",
        "Natural Cue Handoff is in flight before playback pauses.",
      ),
      reviewFrame(
        3.85,
        "paused-handoff-completing",
        "The active handoff completes after playback pauses.",
      ),
      reviewFrame(
        4.25,
        "paused-handoff-settled",
        "The paused destination cue remains at the settled endpoint.",
      ),
      reviewFrame(
        4.9,
        "paused-handoff-remains-settled",
        "No new cue motion starts while the playback clock is paused.",
      ),
    ],
  },
};

export function parseLyricMotionCaptureRequest(arguments_) {
  const request = {
    example: "reel-lift-tour",
    output: undefined,
    reducedAnimation: false,
    resolution: defaultResolution,
  };
  const seen = new Set();
  for (let index = 0; index < arguments_.length; index += 1) {
    const option = arguments_[index];
    if (seen.has(option)) {
      throw new Error(`duplicate lyric motion capture option: ${option}`);
    }
    seen.add(option);
    switch (option) {
      case "--example":
        request.example = requiredValue(arguments_, ++index, option);
        break;
      case "--output":
        request.output = requiredValue(arguments_, ++index, option);
        break;
      case "--reduced-animation":
        request.reducedAnimation = true;
        break;
      case "--resolution":
        request.resolution = parseResolution(
          requiredValue(arguments_, ++index, option),
        );
        break;
      default:
        throw new Error(`unknown lyric motion capture option: ${option}`);
    }
  }
  if (!(request.example in examples)) {
    throw new Error(`unknown lyric motion example: ${request.example}`);
  }
  return request;
}

export function buildLyricMotionCapturePlan(
  request,
  { workingDirectory = process.cwd() } = {},
) {
  const example = examples[request.example];
  if (example === undefined) {
    throw new Error(`unknown lyric motion example: ${request.example}`);
  }
  return {
    example: request.example,
    reducedAnimation: request.reducedAnimation,
    resolution: { ...request.resolution },
    durationSeconds: example.durationSeconds,
    initialFixture: example.initialFixture,
    publications: example.publications.map((publication) => ({
      ...publication,
    })),
    reviewFrames: example.reviewFrames.map((frame) => ({ ...frame })),
    lineCountEvidence:
      example.lineCountEvidenceByResolution?.[
        `${request.resolution.width}x${request.resolution.height}`
      ],
    outputDirectory:
      request.output === undefined
        ? undefined
        : path.resolve(workingDirectory, request.output),
  };
}

export function buildLyricMotionCaptureEnvironment(
  environment,
  display,
  resolution,
  socketPath,
  reducedAnimation = false,
) {
  const captureEnvironment = {
    ...environment,
    DISPLAY: display,
    GDK_BACKEND: "x11",
    NO_AT_BRIDGE: "1",
    ROONSCAPE_CAPTURE_VIEWPORT: `${resolution.width}x${resolution.height}`,
    ROONSCAPE_SOCKET: socketPath,
    ROONSCAPE_WINDOWED: "1",
  };
  for (const name of [
    "ROONSCAPE_CAPTURE_CONTROL",
    "ROONSCAPE_FIXTURE",
    "ROONSCAPE_FIXTURE_AUTO_CLOSE_MS",
    "ROONSCAPE_FIXTURE_CONTROL",
    "ROONSCAPE_STATIC_FIXTURE",
    "ROONSCAPE_CAPTURE_REDUCED_ANIMATION",
  ]) {
    delete captureEnvironment[name];
  }
  if (reducedAnimation) {
    captureEnvironment.ROONSCAPE_CAPTURE_REDUCED_ANIMATION = "1";
  }
  return captureEnvironment;
}

export function executeLyricMotionCapturePlan(plan, { sessionAdapter }) {
  return sessionAdapter.execute(plan);
}

export function createNativeLyricMotionCaptureSessionAdapter(options = {}) {
  return {
    execute: (plan) => runNativeLyricMotionCapture(plan, options),
  };
}

export function reanchorLyricMotionSnapshot(
  fixture,
  { positionSeconds, revision, sampledAt },
) {
  return {
    ...structuredClone(fixture),
    revision,
    timing: {
      ...fixture.timing,
      position: {
        seconds: positionSeconds,
        sampledAt,
      },
    },
  };
}

export function recordingElapsedSeconds(
  recordingStartedAtMilliseconds,
  observedAtMilliseconds = performance.now(),
) {
  return Math.max(
    0,
    (observedAtMilliseconds - recordingStartedAtMilliseconds) / 1_000,
  );
}

export function assertPublicationChronology(publications, framesPerSecond) {
  const toleranceSeconds = 1 / framesPerSecond + 0.02;
  const paintToleranceFrames = 10;
  for (const publication of publications) {
    if (
      Math.abs(publication.publishedAtSeconds - publication.atSeconds) >
      toleranceSeconds
    ) {
      throw new Error(
        `publication scheduled at ${publication.atSeconds.toFixed(2)}s was issued at ${publication.publishedAtSeconds.toFixed(2)}s`,
      );
    }
    if (publication.expectedPaintAtSeconds === undefined) {
      continue;
    }
    if (publication.acknowledgedAtSeconds === undefined) {
      throw new Error("renderer did not acknowledge an expected lyric paint");
    }
    if (publication.acknowledgedAtSeconds < publication.publishedAtSeconds) {
      throw new Error(
        "renderer acknowledged a publication before it was issued",
      );
    }
    const expectedFrame = Math.round(
      publication.expectedPaintAtSeconds * framesPerSecond,
    );
    const acknowledgedFrame = Math.round(
      publication.acknowledgedAtSeconds * framesPerSecond,
    );
    if (Math.abs(acknowledgedFrame - expectedFrame) > paintToleranceFrames) {
      throw new Error(
        `lyric paint expected near encoded frame ${expectedFrame} was acknowledged at frame ${acknowledgedFrame}`,
      );
    }
  }
}

export function expectedLyricPaintAtSeconds(snapshot, publication) {
  const cues = snapshot.lyrics?.cues;
  const firstNonblank = cues?.find((cue) => cue.text.trim().length > 0);
  if (!Array.isArray(cues) || firstNonblank === undefined) {
    return undefined;
  }
  if (
    publication.positionSeconds >
    cues.at(-1).atSeconds + lyricFinalHoldSeconds
  ) {
    return undefined;
  }
  const firstSelectionSeconds = firstNonblank.atSeconds - lyricEntryLeadSeconds;
  if (
    snapshot.playback !== "playing" &&
    publication.positionSeconds < firstSelectionSeconds
  ) {
    return undefined;
  }
  return (
    publication.atSeconds +
    Math.max(0, firstSelectionSeconds - publication.positionSeconds)
  );
}

export function alignEntryFramesToPaint(reviewFrames, publications) {
  const firstPaint = publications.find(
    ({ expectedPaintAtSeconds, acknowledgedAtSeconds }) =>
      expectedPaintAtSeconds !== undefined &&
      acknowledgedAtSeconds !== undefined,
  );
  if (firstPaint === undefined) {
    return reviewFrames.map((frame) => ({ ...frame }));
  }
  const offsets = new Map([
    ["lyrics-entry-before", -0.05],
    ["lyrics-entry-early", 0.05],
    ["lyrics-entry-midpoint", 0.29],
    ["lyrics-entry-settled", 0.58],
  ]);
  return reviewFrames.map((frame) => {
    const offset = offsets.get(frame.name);
    return offset === undefined
      ? { ...frame }
      : {
          ...frame,
          atSeconds:
            Math.round((firstPaint.acknowledgedAtSeconds + offset) * 100) / 100,
        };
  });
}

export function renderLyricMotionCaptureReadme({
  example,
  resolution,
  framesPerSecond,
  durationSeconds,
  reducedAnimation,
  lineCountEvidence,
  frames,
}) {
  const lines = [
    `# Lyric motion capture — ${example}`,
    "",
    `Captured from RoonScape Fixture Mode at ${resolution.width}x${resolution.height}, losslessly at ${framesPerSecond} frames per second for ${durationSeconds.toFixed(2)} seconds.`,
    reducedAnimation
      ? "Reduced animation was enabled; semantic updates should appear only at complete endpoints."
      : "Dynamic animation was enabled for full-rate motion review.",
    ...(lineCountEvidence === undefined ? [] : [lineCountEvidence]),
    "",
    "These are review targets, not automated visual verdicts. Inspect the full-rate sheets for brief states between the selected frames.",
    "",
    "| File | Recording time | Review target |",
    "| --- | ---: | --- |",
    ...frames.map(
      ({ atSeconds, fileName, observation }) =>
        `| \`${fileName}\` | ${formatRelativeTimestamp(atSeconds, "R")} | ${tableText(observation)} |`,
    ),
    "",
  ];
  return lines.join("\n");
}

export async function createLyricMotionReviewArtifacts(
  sessionDirectory,
  plan,
  signal,
) {
  const state = JSON.parse(
    await readFile(path.join(sessionDirectory, "session.json"), "utf8"),
  );
  await createFullRateReviewSheets(sessionDirectory, state, signal);

  const frames = [];
  for (const [index, review] of plan.reviewFrames.entries()) {
    signal?.throwIfAborted();
    if (
      !Number.isFinite(review.atSeconds) ||
      review.atSeconds < 0 ||
      review.atSeconds > state.durationSeconds
    ) {
      throw new Error("review frame lies outside the recorded interval");
    }
    const fileName = `${String(index).padStart(2, "0")}-${semanticSlug(review.name)}.png`;
    await runMonitoredProcess(
      "ffmpeg",
      [
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        String(review.atSeconds),
        "-i",
        path.join(sessionDirectory, "capture.mkv"),
        "-frames:v",
        "1",
        "-y",
        path.join(sessionDirectory, fileName),
      ],
      {
        description: "lyric motion review frame",
        timeoutMilliseconds: 30_000,
        signal,
      },
    );
    frames.push({ ...review, fileName });
  }
  await createOverview(
    sessionDirectory,
    frames.map(({ fileName }) => fileName),
    signal,
  );
  await writeFile(
    path.join(sessionDirectory, "README.md"),
    renderLyricMotionCaptureReadme({
      ...plan,
      durationSeconds: state.durationSeconds,
      framesPerSecond: state.framesPerSecond,
      reducedAnimation: plan.reducedAnimation,
      frames,
    }),
  );
  await writeFile(
    path.join(sessionDirectory, "manifest.json"),
    `${JSON.stringify(
      {
        formatVersion: 1,
        source: "Fixture Mode",
        example: plan.example,
        resolution: plan.resolution,
        framesPerSecond: state.framesPerSecond,
        durationSeconds: state.durationSeconds,
        reducedAnimation: plan.reducedAnimation,
        initialFixture: plan.initialFixture,
        publications: plan.publications,
        frames,
      },
      null,
      2,
    )}\n`,
  );
  return { frames };
}

function reviewFrame(atSeconds, name, observation) {
  return { atSeconds, name, observation };
}

function requiredValue(arguments_, index, option) {
  const value = arguments_[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}

function tableText(value) {
  return value.replace(/\s+/g, " ").replace(/\|/g, "\\|").trim();
}

function semanticSlug(value) {
  const slug = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (slug.length === 0) {
    throw new Error(`review frame name cannot form a filename: ${value}`);
  }
  return slug;
}

async function createOverview(sessionDirectory, fileNames, signal) {
  const work = await mkdtemp(path.join(sessionDirectory, ".overview."));
  try {
    for (const [index, fileName] of fileNames.entries()) {
      await runMonitoredProcess(
        "ffmpeg",
        [
          "-hide_banner",
          "-loglevel",
          "error",
          "-i",
          path.join(sessionDirectory, fileName),
          "-vf",
          "scale=384:216:force_original_aspect_ratio=decrease,pad=384:216:(ow-iw)/2:(oh-ih)/2:black",
          "-frames:v",
          "1",
          "-y",
          path.join(work, `${String(index).padStart(3, "0")}.png`),
        ],
        {
          description: "lyric motion overview thumbnail",
          timeoutMilliseconds: 30_000,
          signal,
        },
      );
    }
    const columns = Math.min(5, fileNames.length);
    const rows = Math.ceil(fileNames.length / columns);
    await runMonitoredProcess(
      "ffmpeg",
      [
        "-hide_banner",
        "-loglevel",
        "error",
        "-framerate",
        "1",
        "-start_number",
        "0",
        "-i",
        path.join(work, "%03d.png"),
        "-vf",
        `tile=${columns}x${rows}:nb_frames=${fileNames.length}:padding=2:margin=0:color=black`,
        "-frames:v",
        "1",
        "-y",
        path.join(sessionDirectory, "overview.png"),
      ],
      {
        description: "lyric motion overview",
        timeoutMilliseconds: 120_000,
        signal,
      },
    );
  } finally {
    await rm(work, { force: true, recursive: true });
  }
}

async function createFullRateReviewSheets(sessionDirectory, state, signal) {
  const reviewDirectory = path.join(sessionDirectory, "review");
  await mkdir(reviewDirectory);
  const framesPerPage = 100;
  const columns = 10;
  const frameCount = Math.max(
    1,
    Math.round(state.durationSeconds * state.framesPerSecond),
  );
  const index = { framesPerSecond: state.framesPerSecond, pages: [] };
  for (
    let firstFrame = 0;
    firstFrame < frameCount;
    firstFrame += framesPerPage
  ) {
    const count = Math.min(framesPerPage, frameCount - firstFrame);
    const rows = Math.ceil(count / columns);
    const pageNumber = firstFrame / framesPerPage + 1;
    const file = `full-rate-page-${String(pageNumber).padStart(3, "0")}.png`;
    await runMonitoredProcess(
      "ffmpeg",
      [
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        (firstFrame / state.framesPerSecond).toFixed(3),
        "-i",
        path.join(sessionDirectory, "capture.mkv"),
        "-vf",
        `scale=192:108,drawtext=font=monospace:text='%{n}':fontcolor=white:fontsize=16:box=1:boxcolor=black@0.78:boxborderw=3:x=4:y=h-th-4,tile=${columns}x${rows}:nb_frames=${count}:padding=1:margin=0:color=black`,
        "-frames:v",
        "1",
        "-y",
        path.join(reviewDirectory, file),
      ],
      {
        cwd: repositoryRoot,
        description: "full-rate lyric motion review sheet",
        timeoutMilliseconds: 120_000,
        signal,
      },
    );
    index.pages.push({
      file,
      firstFrame,
      count,
      columns,
      startSeconds:
        Math.round((firstFrame / state.framesPerSecond) * 1000) / 1000,
    });
  }
  await writeFile(
    path.join(reviewDirectory, "review-index.json"),
    `${JSON.stringify(index, null, 2)}\n`,
  );
}

async function runNativeLyricMotionCapture(
  plan,
  {
    environment = process.env,
    rendererExecutable = defaultRendererExecutable,
    signal,
  },
) {
  const fixtures = await preflightNativeCapture(
    plan,
    environment,
    rendererExecutable,
  );
  await mkdir(scratchRoot, { recursive: true });
  const outputDirectory = await reserveOutputDirectory(plan);
  const sessionDirectory = await mkdtemp(
    path.join(scratchRoot, "task.lyric-motion."),
  );
  const runtimeDirectory = path.join(sessionDirectory, "runtime");
  const socketPath = path.join(runtimeDirectory, "roonscape.sock");
  const configurationPath = path.join(runtimeDirectory, "display.json");
  const videoPath = path.join(sessionDirectory, "capture.mkv");
  await mkdir(runtimeDirectory, { mode: 0o700 });
  await writeFile(
    configurationPath,
    `${JSON.stringify({
      trackedOutputId: "lyric-motion-capture",
      trackedOutputName: "Speaker System",
      inactivity: {
        gracePeriodSeconds: 3600,
        dimmedOpacity: 0.35,
        repositionCadenceSeconds: 60,
      },
    })}\n`,
    { mode: 0o600 },
  );

  const firstFixture = [...fixtures.values()].find((fixture) =>
    fixture.lyrics?.cues.some((cue) => cue.text.trim().length > 0),
  );
  if (firstFixture === undefined) {
    throw new Error(
      "lyric motion review requires a nonblank cue for native warm-up",
    );
  }
  let visibleWarmupRevision;
  const visibleRevisionWaiters = new Map();
  const visibleRevisionTimes = new Map();
  let recordingStartedAtMilliseconds;
  let reportWarmupVisible;
  const warmupVisible = new Promise((resolve) => {
    reportWarmupVisible = resolve;
  });
  const warmupSnapshot = reanchorLyricMotionSnapshot(firstFixture, {
    positionSeconds: firstFixture.lyrics.cues.find(
      (cue) => cue.text.trim().length > 0,
    ).atSeconds,
    revision: 1,
    sampledAt: new Date().toISOString(),
  });
  let publisher;
  let renderer;
  let recorder;
  let nativeSession;
  let failure;
  let executedPlan = plan;
  const cleanupFailures = [];
  try {
    const { startSnapshotPublisher } =
      await import("../src/bridge/dist/src/fixture-publisher.js");
    publisher = await startSnapshotPublisher(warmupSnapshot, socketPath, {
      onLyricsVisible(revision) {
        visibleWarmupRevision = revision;
        reportWarmupVisible();
        if (recordingStartedAtMilliseconds !== undefined) {
          visibleRevisionTimes.set(
            revision,
            recordingElapsedSeconds(recordingStartedAtMilliseconds),
          );
          visibleRevisionWaiters.get(revision)?.();
        }
      },
    });
    nativeSession = await createNativeSession({
      ...plan.resolution,
      environment,
      signal,
    });
    const display = nativeSession.environment.DISPLAY;
    const rendererEnvironment = buildLyricMotionCaptureEnvironment(
      nativeSession.environment,
      display,
      plan.resolution,
      socketPath,
      plan.reducedAnimation,
    );
    renderer = startMonitoredProcess(
      rendererExecutable,
      ["--config", configurationPath],
      { cwd: repositoryRoot, environment: rendererEnvironment },
    );
    await renderer.spawned;
    await waitForNativeWindow(
      renderer,
      rendererEnvironment,
      plan.resolution.width,
      plan.resolution.height,
      { signal },
    );
    await waitForPromise(
      warmupVisible,
      renderer,
      "the warm-up lyric frame",
      10_000,
      signal,
    );
    if (visibleWarmupRevision !== 1) {
      throw new Error(
        `renderer reported unexpected warm-up lyric revision ${visibleWarmupRevision}`,
      );
    }

    const initialFixture = fixtures.get(plan.initialFixture);
    publisher.publish(
      reanchorLyricMotionSnapshot(initialFixture, {
        positionSeconds: 0,
        revision: 2,
        sampledAt: new Date().toISOString(),
      }),
    );
    await delay(1_500);
    signal?.throwIfAborted();
    assertProcessRunning(renderer, "lyric motion capture renderer");

    recorder = startLosslessRecorder(
      display,
      plan.resolution,
      videoPath,
      rendererEnvironment,
      plan.durationSeconds,
    );
    const recorderCompletion = once(recorder, "close");
    await recorder.spawned;
    recordingStartedAtMilliseconds = performance.now();
    assertProcessRunning(recorder, "lyric motion capture recorder");

    const actualPublications = [];
    for (const [index, publication] of plan.publications.entries()) {
      await waitUntilRecordingTime(
        recordingStartedAtMilliseconds,
        publication.atSeconds,
        recorder,
        renderer,
        signal,
      );
      const publishedAtSeconds =
        Math.round(
          recordingElapsedSeconds(recordingStartedAtMilliseconds) * 100,
        ) / 100;
      const revision = index + 3;
      const fixture = fixtures.get(publication.fixture);
      const expectedPaintAtSeconds = expectedLyricPaintAtSeconds(
        fixture,
        publication,
      );
      let visible;
      if (expectedPaintAtSeconds !== undefined) {
        visible = new Promise((resolve) => {
          visibleRevisionWaiters.set(revision, resolve);
        });
      }
      publisher.publish(
        reanchorLyricMotionSnapshot(fixture, {
          positionSeconds: publication.positionSeconds,
          revision,
          sampledAt: new Date().toISOString(),
        }),
      );
      if (visible !== undefined) {
        await waitForPromise(
          visible,
          renderer,
          `painted lyric publication revision ${revision}`,
          5_000,
          signal,
        );
        visibleRevisionWaiters.delete(revision);
      }
      actualPublications.push({
        ...publication,
        publishedAtSeconds,
        ...(expectedPaintAtSeconds !== undefined && { expectedPaintAtSeconds }),
        ...(visibleRevisionTimes.has(revision) && {
          acknowledgedAtSeconds:
            Math.round(visibleRevisionTimes.get(revision) * 100) / 100,
        }),
      });
    }

    await waitForRecordingCompletion({
      recorder,
      recorderCompletion,
      renderer,
      timeoutMilliseconds: (plan.durationSeconds + 120) * 1000,
      signal,
    });
    recorder = undefined;
    const video = await probeCaptureVideo(videoPath, plan.resolution, signal);
    if (video.durationSeconds + 0.1 < plan.durationSeconds) {
      throw new Error(
        `lyric motion recording ended at ${video.durationSeconds.toFixed(2)} seconds; expected ${plan.durationSeconds.toFixed(2)}`,
      );
    }
    assertPublicationChronology(actualPublications, framesPerSecond);
    executedPlan = {
      ...plan,
      publications: actualPublications,
      reviewFrames: alignEntryFramesToPaint(
        plan.reviewFrames,
        actualPublications,
      ),
    };
    await writeFile(
      path.join(sessionDirectory, "session.json"),
      `${JSON.stringify({
        status: "recorded",
        durationSeconds: video.durationSeconds,
        framesPerSecond,
        resolution: plan.resolution,
        publications: actualPublications,
      })}\n`,
    );
  } catch (error) {
    failure = error;
    await writeFile(
      path.join(sessionDirectory, "diagnostics.log"),
      `${errorMessage(error)}\n`,
    ).catch(() => {});
  } finally {
    for (const [child, description] of [
      [recorder, "lyric motion capture recorder"],
      [renderer, "lyric motion capture renderer"],
    ]) {
      try {
        await stopProcess(child, { description, signal });
      } catch (error) {
        cleanupFailures.push(error);
      }
    }
    if (publisher !== undefined) {
      try {
        await publisher.close();
      } catch (error) {
        cleanupFailures.push(error);
      }
    }
    try {
      await nativeSession?.close(failure);
    } catch (error) {
      cleanupFailures.push(error);
    }
    await rm(runtimeDirectory, { force: true, recursive: true });
  }
  if (failure !== undefined || cleanupFailures.length > 0) {
    if (plan.outputDirectory === undefined) await rmdir(outputDirectory);
    const errors = [failure, ...cleanupFailures].filter(
      (error) => error !== undefined,
    );
    await writeFile(
      path.join(sessionDirectory, "diagnostics.log"),
      `${errors.map(errorMessage).join("\n")}\n`,
    ).catch(() => {});
    throw new AggregateError(
      errors,
      `Lyric motion capture failed; diagnostics remain at ${sessionDirectory}`,
      { cause: errors[0] },
    );
  }
  try {
    await createLyricMotionReviewArtifacts(
      sessionDirectory,
      executedPlan,
      signal,
    );
    await moveCaptureOutput(
      sessionDirectory,
      outputDirectory,
      plan.outputDirectory === undefined,
    );
  } catch (error) {
    if (plan.outputDirectory === undefined)
      await rmdir(outputDirectory).catch(() => {});
    await writeFile(
      path.join(sessionDirectory, "diagnostics.log"),
      `${errorMessage(error)}\n`,
    ).catch(() => {});
    throw new Error(
      `Lyric motion review generation failed; diagnostics remain at ${sessionDirectory}`,
      { cause: error },
    );
  }
  return outputDirectory;
}

async function preflightNativeCapture(plan, environment, rendererExecutable) {
  const missing = [];
  for (const executable of [
    "Xvfb",
    "dbus-daemon",
    "ffmpeg",
    "ffprobe",
    "xwininfo",
  ]) {
    if (!(await executableOnPath(executable, environment))) {
      missing.push(executable);
    }
  }
  if (missing.length > 0) {
    throw new Error(
      `required executable is unavailable: ${missing.join(", ")}`,
    );
  }
  await access(rendererExecutable, fileConstants.X_OK);
  const fixtures = await loadCaptureFixtures(plan);
  if (plan.outputDirectory !== undefined) {
    await assertPathDoesNotExist(plan.outputDirectory);
  }
  return fixtures;
}

async function loadCaptureFixtures(plan) {
  const paths = new Set([
    plan.initialFixture,
    ...plan.publications.map(({ fixture }) => fixture),
  ]);
  const fixtures = new Map();
  for (const fixturePath of paths) {
    fixtures.set(
      fixturePath,
      await loadPresentationCaptureSnapshot(fixturePath),
    );
  }
  return fixtures;
}

async function reserveOutputDirectory(plan) {
  if (plan.outputDirectory !== undefined) {
    await assertPathDoesNotExist(plan.outputDirectory);
    return plan.outputDirectory;
  }
  const base = path.join(scratchRoot, `${plan.example}-${localDate()}`);
  return mkdtemp(`${base}.`);
}

async function moveCaptureOutput(sessionDirectory, outputDirectory, reserved) {
  await mkdir(path.dirname(outputDirectory), { recursive: true });
  if (!reserved) await assertPathDoesNotExist(outputDirectory);
  try {
    await rename(sessionDirectory, outputDirectory);
  } catch (error) {
    if (error?.code !== "EXDEV") {
      throw error;
    }
    await cp(sessionDirectory, outputDirectory, {
      errorOnExist: true,
      force: false,
      recursive: true,
    });
    await rm(sessionDirectory, { recursive: true });
  }
}

function startLosslessRecorder(
  display,
  resolution,
  videoPath,
  environment,
  durationSeconds,
) {
  return startMonitoredProcess(
    "nice",
    [
      "-n",
      "10",
      "ffmpeg",
      "-hide_banner",
      "-loglevel",
      "warning",
      "-stats_period",
      "0.05",
      "-progress",
      "pipe:1",
      "-nostdin",
      "-f",
      "x11grab",
      "-draw_mouse",
      "0",
      "-framerate",
      String(framesPerSecond),
      "-video_size",
      `${resolution.width}x${resolution.height}`,
      "-i",
      `${display}.0+0,0`,
      "-vf",
      "setpts=PTS-STARTPTS",
      "-c:v",
      "utvideo",
      "-pix_fmt",
      "gbrp",
      "-fps_mode",
      "cfr",
      "-frames:v",
      String(Math.round(durationSeconds * framesPerSecond)),
      "-y",
      videoPath,
    ],
    { cwd: repositoryRoot, environment },
  );
}

async function probeCaptureVideo(videoPath, expectedResolution, signal) {
  const output = await runMonitoredProcess(
    "ffprobe",
    [
      "-v",
      "error",
      "-show_entries",
      "format=duration:stream=width,height",
      "-of",
      "json",
      videoPath,
    ],
    {
      cwd: repositoryRoot,
      description: "lyric motion recording validation",
      signal,
      timeoutMilliseconds: 30_000,
    },
  );
  const report = JSON.parse(output);
  const durationSeconds = Number(report.format?.duration);
  const stream = report.streams?.[0];
  if (
    !Number.isFinite(durationSeconds) ||
    durationSeconds <= 0 ||
    stream?.width !== expectedResolution.width ||
    stream?.height !== expectedResolution.height
  ) {
    throw new Error("lyric motion recording is invalid");
  }
  return { durationSeconds };
}

async function waitUntilRecordingTime(
  recordingStartedAtMilliseconds,
  atSeconds,
  recorder,
  renderer,
  signal,
) {
  while (recordingElapsedSeconds(recordingStartedAtMilliseconds) < atSeconds) {
    signal?.throwIfAborted();
    assertProcessRunning(recorder, "lyric motion capture recorder");
    assertProcessRunning(renderer, "lyric motion capture renderer");
    await delay(10);
  }
}

async function waitForRecordingCompletion({
  recorder,
  recorderCompletion,
  renderer,
  timeoutMilliseconds,
  signal,
}) {
  const deadline = performance.now() + timeoutMilliseconds;
  while (performance.now() < deadline) {
    signal?.throwIfAborted();
    const outcome = await Promise.race([
      recorderCompletion.then((value) => ({ kind: "closed", value })),
      delay(100).then(() => ({ kind: "waiting" })),
    ]);
    if (outcome.kind === "closed") {
      const [exitCode, signal] = outcome.value;
      if (exitCode !== 0) {
        throw processFailure(
          "lyric motion capture recorder",
          recorder,
          exitCode,
          signal,
        );
      }
      return;
    }
    assertProcessRunning(renderer, "lyric motion capture renderer");
  }
  throw new Error("timed out waiting for the lyric motion recording");
}

async function waitForPromise(
  promise,
  child,
  description,
  timeoutMilliseconds,
  signal,
) {
  let ready = false;
  promise.then(() => {
    ready = true;
  });
  await waitFor(
    () => {
      if (!ready) throw new Error(`${description} is not ready`);
      return true;
    },
    child,
    description,
    { timeoutMilliseconds, signal },
  );
}

async function executableOnPath(executable, environment) {
  for (const directory of (environment.PATH ?? "").split(path.delimiter)) {
    if (directory.length === 0) {
      continue;
    }
    try {
      await access(path.join(directory, executable), fileConstants.X_OK);
      return true;
    } catch {
      // Continue searching PATH.
    }
  }
  return false;
}

async function assertPathDoesNotExist(candidate) {
  try {
    await lstat(candidate);
  } catch (error) {
    if (error?.code === "ENOENT") {
      return;
    }
    throw error;
  }
  throw new Error(`capture output already exists: ${candidate}`);
}

function localDate(now = new Date()) {
  return [now.getFullYear(), now.getMonth() + 1, now.getDate()]
    .map((part, index) => String(part).padStart(index === 0 ? 4 : 2, "0"))
    .join("-");
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
