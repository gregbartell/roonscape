import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import {
  buildPresentationCapturePlan,
  listFixtureScenarios,
  selectFocusedPresentationCapture,
} from "./presentation-captures.mjs";
import { validatePresentationCaptureSnapshot } from "./presentation-snapshot.mjs";

const execFileAsync = promisify(execFile);
const repositoryRoot = new URL("..", import.meta.url);
const REPRESENTATIVE_VIEWPORTS = [
  "1280x720",
  "1600x900",
  "1600x1200",
  "1920x1200",
  "2560x1080",
  "3840x2160",
  "3840x2400",
];
const REQUIRED_SCENARIOS = [
  "playing",
  "paused",
  "lyrics-one-line",
  "lyrics-two-line",
  "lyrics-three-line",
  "lyrics-four-lines",
  "lyrics-blank-cue",
  "lyrics-long-masthead",
  "lyrics-missing-artwork",
  "loading-with-content",
  "loading-without-content",
  "idle",
  "pairing-required",
  "disconnected",
  "output-unavailable",
  "playing-without-content",
  "paused-without-content",
  "missing-metadata",
  "missing-artist",
  "missing-album",
  "missing-artwork",
  "long-metadata",
  "extreme-metadata",
  "indeterminate-progress",
  "non-square-artwork",
  "light-artwork",
];

test("Presentation Capture enforces shared Presentation Snapshot limits", async () => {
  const snapshot = JSON.parse(
    await readFile(
      new URL("../src/shared/fixtures/playing.json", import.meta.url),
      "utf8",
    ),
  );
  const oversizedTitle = structuredClone(snapshot);
  oversizedTitle.nowPlaying.title = "🌌".repeat(1_025);
  await assert.rejects(
    validatePresentationCaptureSnapshot(oversizedTitle),
    /Invalid presentation snapshot/,
  );

  const oversizedMessage = structuredClone(snapshot);
  oversizedMessage.artwork.path = "x".repeat(64 * 1024);
  await assert.rejects(
    validatePresentationCaptureSnapshot(oversizedMessage),
    /Snapshot exceeds 64 KiB/,
  );
});

test("plans every maintained Fixture Scenario at every representative viewport", () => {
  const plan = buildPresentationCapturePlan();

  for (const viewport of REPRESENTATIVE_VIEWPORTS) {
    const captures = plan.filter(
      (capture) =>
        capture.variant === "matrix" && capture.viewport === viewport,
    );
    assert.deepEqual(
      captures.map(({ scenario }) => scenario),
      REQUIRED_SCENARIOS,
    );
    assert.deepEqual(
      captures.map(({ fileName }) => fileName),
      REQUIRED_SCENARIOS.map((scenario) => `${viewport}--${scenario}.png`),
    );
  }
  assert.equal(new Set(plan.map(({ fileName }) => fileName)).size, plan.length);
});

test("derives the maintained matrix from the Fixture Scenario catalog", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-presentation-plan-test."),
  );
  const catalogPath = path.join(directory, "fixture-scenario-catalog.json");

  try {
    const catalog = JSON.parse(
      await readFile(
        new URL(
          "../src/shared/fixtures/fixture-scenario-catalog.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    [catalog.scenarios[1], catalog.scenarios[2]] = [
      catalog.scenarios[2],
      catalog.scenarios[1],
    ];
    await writeFile(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`);

    const plan = buildPresentationCapturePlan({ catalogPath });
    for (const viewport of REPRESENTATIVE_VIEWPORTS) {
      assert.deepEqual(
        plan
          .filter(
            (capture) =>
              capture.variant === "matrix" && capture.viewport === viewport,
          )
          .map(({ scenario }) => scenario),
        catalog.scenarios.map(({ scenario }) => scenario),
      );
    }
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

test("plans typography, diagnostics, progress, and palette representatives", async () => {
  const representatives = buildPresentationCapturePlan().filter(
    ({ variant }) => variant === "representative",
  );
  const at1280 = representatives.filter(
    ({ viewport }) => viewport === "1280x720",
  );

  assert.deepEqual(
    at1280.map(({ scenario }) => scenario),
    [
      "preferred-typography",
      "fallback-typography",
      "identity-baselines",
      "progress-early",
      "progress-middle",
      "progress-near-complete",
      "dark-diagnostics",
      "light-diagnostics",
      "fixed-no-art-diagnostics",
      "light-matte-restraint",
      "dark-matte-ownership",
    ],
  );
  assert.deepEqual(
    [
      ...new Set(
        at1280
          .filter(({ diagnostics }) => diagnostics)
          .map(({ palette }) => palette),
      ),
    ].sort(),
    ["dark", "fixed-no-art", "light"],
  );
  assert.deepEqual(
    at1280
      .filter(({ typography }) => typography !== "automatic")
      .map(({ typography }) => typography)
      .sort(),
    ["fallback", "preferred"],
  );

  const progress = at1280.filter(({ scenario }) =>
    ["progress-early", "progress-middle", "progress-near-complete"].includes(
      scenario,
    ),
  );
  const fractions = await Promise.all(
    progress.map(async ({ fixture }) => {
      const snapshot = JSON.parse(
        await readFile(new URL(`../${fixture}`, import.meta.url), "utf8"),
      );
      return snapshot.timing.position.seconds / snapshot.timing.durationSeconds;
    }),
  );
  assert.ok(fractions[0] < 0.2);
  assert.ok(fractions[1] > 0.4 && fractions[1] < 0.8);
  assert.ok(fractions[2] > 0.9 && fractions[2] < 1);
});

test("lists stable Fixture Scenario identifiers without launching tools", () => {
  assert.deepEqual(
    listFixtureScenarios().map(({ scenario }) => scenario),
    REQUIRED_SCENARIOS,
  );
});

test("focused selection is exact and unambiguous", () => {
  const plan = buildPresentationCapturePlan();
  assert.equal(
    selectFocusedPresentationCapture(plan, "playing").fileName,
    "3840x2160--playing.png",
  );
  assert.throws(
    () => selectFocusedPresentationCapture(plan, "not-maintained"),
    /unknown Fixture Scenario identifier/,
  );
  assert.throws(
    () =>
      selectFocusedPresentationCapture(
        [...plan, selectFocusedPresentationCapture(plan, "playing")],
        "playing",
      ),
    /ambiguous Fixture Scenario identifier/,
  );
});

test("capture command wires valid extensionless artwork through the native pipeline", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-valid-artwork-test."),
  );
  const artwork = path.join(directory, "Proposed cover.unknown");
  const artworkContents = await readFile(
    new URL("../src/shared/fixtures/artwork/light.svg", import.meta.url),
  );
  await writeFile(artwork, artworkContents);
  const artworkHash = createHash("sha256")
    .update(artworkContents)
    .digest("hex")
    .slice(0, 12);
  const capturePath = path.join(
    directory,
    `1280x720--playing--proposed-cover-unknown--${artworkHash}.png`,
  );

  try {
    const { stdout, stderr } = await execFileAsync(
      process.execPath,
      [
        path.join(repositoryRoot.pathname, "scripts/capture-presentations.mjs"),
        "--scenario",
        "playing",
        "--artwork",
        artwork,
        "--resolution",
        "1280x720",
      ],
      { cwd: directory, env: process.env, timeout: 300_000 },
    );
    assert.equal(stdout, `${capturePath}\n`);
    assert.match(stderr, /Capturing Fixture Scenario playing at 1280x720/);
    const capture = await readFile(capturePath);
    assert.equal(capture.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
    assert.equal(capture.readUInt32BE(16), 1280);
    assert.equal(capture.readUInt32BE(20), 720);
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});
