import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import { promisify } from "node:util";
import test from "node:test";

import {
  buildPresentationCapturePlan,
  selectFocusedPresentationCapture,
} from "./presentation-captures.mjs";
import {
  installPresentationCaptureFixtures,
  presentationCapturePngHeader,
} from "./presentation-capture-test-fixtures.mjs";
import { validatePresentationCaptureSnapshot } from "./presentation-snapshot.mjs";

const execFileAsync = promisify(execFile);

test("Presentation Capture enforces the shared snapshot field and total limits", async () => {
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
const ORDINARY_SCENARIOS = REQUIRED_SCENARIOS.filter(
  (scenario) => !["light-artwork", "non-square-artwork"].includes(scenario),
);
const CUSTOM_ARTWORK_SCENARIOS = new Set([
  "playing",
  "paused",
  "lyrics-one-line",
  "lyrics-two-line",
  "lyrics-three-line",
  "lyrics-four-lines",
  "lyrics-blank-cue",
  "lyrics-long-masthead",
  "loading-with-content",
  "missing-metadata",
  "missing-artist",
  "missing-album",
  "long-metadata",
  "extreme-metadata",
  "indeterminate-progress",
]);

test("plans every visual acceptance scenario at every representative viewport", () => {
  const plan = buildPresentationCapturePlan();
  for (const scenario of REQUIRED_SCENARIOS) {
    const captures = plan.filter(
      (capture) =>
        capture.scenario === scenario && capture.variant === "matrix",
    );
    assert.deepEqual(
      captures.map((capture) => capture.viewport).sort(),
      REPRESENTATIVE_VIEWPORTS.toSorted(),
      `${scenario} should be captured at every representative viewport`,
    );
  }

  for (const viewport of REPRESENTATIVE_VIEWPORTS) {
    const captures = plan.filter(
      (capture) =>
        capture.variant === "matrix" && capture.viewport === viewport,
    );
    assert.deepEqual(
      captures.map((capture) => capture.scenario),
      REQUIRED_SCENARIOS,
      `${viewport} should cover every Fixture Scenario as a peer viewport`,
    );
    assert.deepEqual(
      captures.map((capture) => capture.fileName),
      REQUIRED_SCENARIOS.map((scenario) => `${viewport}--${scenario}.png`),
    );
  }

  assert.equal(
    new Set(plan.map((capture) => capture.fileName)).size,
    plan.length,
    "capture artifact names should be unique",
  );
});

test("derives the visual acceptance matrix from the Fixture Scenario catalog", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-presentation-test."),
  );
  const catalogPath = path.join(taskDirectory, "fixture-scenario-catalog.json");

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
      const matrixScenarios = plan
        .filter(
          (capture) =>
            capture.variant === "matrix" && capture.viewport === viewport,
        )
        .map((capture) => capture.scenario);

      assert.deepEqual(
        matrixScenarios,
        catalog.scenarios.map((scenario) => scenario.scenario),
      );
    }
  } finally {
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

test("plans explicit typography, progress, and diagnostics representatives", () => {
  const representatives = buildPresentationCapturePlan().filter(
    (capture) => capture.variant === "representative",
  );

  assert.deepEqual(
    [
      ...new Set(
        representatives
          .filter((capture) => capture.typography !== "automatic")
          .map((capture) => capture.typography),
      ),
    ].sort(),
    ["fallback", "preferred"],
  );
  assert.ok(
    representatives
      .filter((capture) => capture.typography !== "automatic")
      .every(
        (capture) =>
          capture.fixture === "src/shared/fixtures/glyph-fallback.json",
      ),
    "both Title paths should visibly exercise Pango glyph fallback while supporting roles stay fixed",
  );
  assert.deepEqual(
    [
      ...new Set(
        representatives
          .filter((capture) => capture.diagnostics)
          .map((capture) => capture.palette),
      ),
    ].sort(),
    ["dark", "fixed-no-art", "light"],
  );
  assert.ok(
    representatives
      .filter((capture) => capture.scenario === "identity-baselines")
      .every(
        (capture) =>
          capture.fixture === "src/shared/fixtures/long-identities.json" &&
          capture.typography === "automatic" &&
          !capture.diagnostics,
      ),
    "the native capture plan should expose long Output and Zone baseline alignment",
  );
  for (const viewport of REPRESENTATIVE_VIEWPORTS) {
    assert.deepEqual(
      representatives
        .filter(
          (capture) =>
            capture.viewport === viewport &&
            !["light-matte-restraint", "dark-matte-ownership"].includes(
              capture.scenario,
            ),
        )
        .map((capture) => capture.scenario),
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
      ],
      `${viewport} should receive the same typography and diagnostics review`,
    );
  }
});

test("plans focused light-restraint and dark-ownership native captures", () => {
  const representatives = buildPresentationCapturePlan().filter(
    (capture) => capture.variant === "representative",
  );

  for (const viewport of REPRESENTATIVE_VIEWPORTS) {
    assert.deepEqual(
      representatives
        .filter(
          (capture) =>
            capture.viewport === viewport &&
            ["light-matte-restraint", "dark-matte-ownership"].includes(
              capture.scenario,
            ),
        )
        .map(({ scenario, fixture, palette }) => ({
          scenario,
          fixture,
          palette,
        })),
      [
        {
          scenario: "light-matte-restraint",
          fixture: "src/shared/fixtures/cellout-direction.json",
          palette: "restrained-light",
        },
        {
          scenario: "dark-matte-ownership",
          fixture: "src/shared/fixtures/forever-direction.json",
          palette: "dark-teal",
        },
      ],
      `${viewport} should include focused light and dark Chromatic Matte evidence`,
    );
  }
});

test("plans early middle and near-complete progress evidence", async () => {
  const representatives = buildPresentationCapturePlan().filter(
    (capture) =>
      capture.variant === "representative" &&
      ["progress-early", "progress-middle", "progress-near-complete"].includes(
        capture.scenario,
      ),
  );

  for (const viewport of REPRESENTATIVE_VIEWPORTS) {
    const progression = representatives.filter(
      (capture) => capture.viewport === viewport,
    );
    assert.deepEqual(
      progression.map((capture) => capture.scenario),
      ["progress-early", "progress-middle", "progress-near-complete"],
      `${viewport} should expose the complete determinate progression`,
    );

    const fractions = await Promise.all(
      progression.map(async (capture) => {
        const fixture = JSON.parse(
          await readFile(new URL(`../${capture.fixture}`, import.meta.url)),
        );
        return fixture.timing.position.seconds / fixture.timing.durationSeconds;
      }),
    );
    assert.ok(fractions[0] < 0.2, "the early rail should remain below 20%");
    assert.ok(
      fractions[1] > 0.4 && fractions[1] < 0.8,
      "the middle rail should remain visibly intermediate",
    );
    assert.ok(
      fractions[2] > 0.9 && fractions[2] < 1,
      "the near-complete rail should retain a visible remaining track",
    );
  }
});

test("capture command lists stable Fixture Scenario identifiers and labels without launching tools", async () => {
  const { stdout, stderr } = await execFileAsync(
    process.execPath,
    ["scripts/capture-presentations.mjs", "--list-scenarios"],
    {
      cwd: new URL("..", import.meta.url),
      env: { ...process.env, PATH: "" },
    },
  );

  assert.equal(stderr, "");
  assert.equal(
    stdout,
    [
      "playing\tPlaying",
      "paused\tPaused",
      "lyrics-one-line\tLyrics — one line",
      "lyrics-two-line\tLyrics — two lines",
      "lyrics-three-line\tLyrics — three lines",
      "lyrics-four-lines\tLyrics — four or more lines",
      "lyrics-blank-cue\tLyrics — blank cue",
      "lyrics-long-masthead\tLyrics — long masthead",
      "lyrics-missing-artwork\tLyrics — missing artwork",
      "loading-with-content\tStarting with content",
      "loading-without-content\tStarting without content",
      "idle\tIdle",
      "pairing-required\tAwaiting Roon Authorization",
      "disconnected\tDisconnected",
      "output-unavailable\tOutput unavailable",
      "playing-without-content\tPlaying without content",
      "paused-without-content\tPaused without content",
      "missing-metadata\tMissing metadata",
      "missing-artist\tMissing Artist",
      "missing-album\tMissing Album",
      "missing-artwork\tMissing artwork",
      "long-metadata\tLong metadata",
      "extreme-metadata\tExtreme metadata",
      "indeterminate-progress\tIndeterminate progress",
      "non-square-artwork\tNon-square artwork",
      "light-artwork\tLight artwork",
      "",
    ].join("\n"),
  );
});

test("focused capture selection rejects an ambiguous exact Fixture Scenario", () => {
  const plan = buildPresentationCapturePlan();
  const playing = selectFocusedPresentationCapture(plan, "playing");

  assert.throws(
    () => selectFocusedPresentationCapture([...plan, playing], "playing"),
    /ambiguous Fixture Scenario identifier: playing/,
  );
});

test("capture command requires one current selector and guides callers away from removed options", async () => {
  const invalidInvocations = [
    {
      arguments: [],
      diagnostic:
        /a Presentation Capture selector is required: use --scenario, --all, or --profile visual-acceptance/,
    },
    {
      arguments: ["--output", "captures"],
      diagnostic:
        /a Presentation Capture selector is required: use --scenario, --all, or --profile visual-acceptance/,
    },
    {
      arguments: ["--list"],
      diagnostic: /--list was removed; use --list-scenarios/,
    },
    {
      arguments: ["--only", "playing"],
      diagnostic: /--only was removed; use --scenario/,
    },
    {
      arguments: ["--viewport", "1280x720"],
      diagnostic: /--viewport was removed; use --resolution/,
    },
    {
      arguments: ["--settle-ms", "0"],
      diagnostic:
        /--settle-ms was removed; Presentation Captures now wait for painted-frame readiness/,
    },
  ];

  for (const invocation of invalidInvocations) {
    await assert.rejects(
      execFileAsync(
        process.execPath,
        ["scripts/capture-presentations.mjs", ...invocation.arguments],
        {
          cwd: new URL("..", import.meta.url),
          env: { ...process.env, PATH: "" },
        },
      ),
      (error) => {
        assert.equal(error.stdout, "");
        assert.match(error.stderr, invocation.diagnostic);
        return true;
      },
      invocation.arguments.join(" "),
    );
  }
});

test("focused capture rejects invalid resolutions and option combinations before launching tools", async () => {
  const invalidInvocations = [
    {
      arguments: ["--scenario", "playing", "--resolution", "wide"],
      diagnostic: /--resolution must use WIDTHxHEIGHT/,
    },
    {
      arguments: ["--scenario", "playing", "--resolution", "0x720"],
      diagnostic: /positive safe integers/,
    },
    {
      arguments: ["--scenario", "playing", "--resolution", "1280x719"],
      diagnostic: /at least 1280x720/,
    },
    {
      arguments: ["--scenario", "playing", "--resolution", "1280x1281"],
      diagnostic: /landscape/,
    },
    {
      arguments: ["--scenario", "playing", "--resolution", "1280x1280"],
      diagnostic: /landscape/,
    },
    {
      arguments: ["--scenario", "playing", "--resolution", "32768x720"],
      diagnostic: /exceeds the supported maximum of 32767/,
    },
    {
      arguments: ["--resolution", "1920x1080"],
      diagnostic: /a Presentation Capture selector is required/,
    },
    {
      arguments: ["--overwrite"],
      diagnostic: /a Presentation Capture selector is required/,
    },
    {
      arguments: ["--artwork", "cover.png"],
      diagnostic: /a Presentation Capture selector is required/,
    },
    {
      arguments: [
        "--scenario",
        "playing",
        "--artwork",
        "first.png",
        "--artwork",
        "second.png",
      ],
      diagnostic: /duplicate capture option: --artwork/,
    },
    {
      arguments: ["--list-scenarios", "--overwrite"],
      diagnostic: /--list-scenarios cannot be combined with capture options/,
    },
    {
      arguments: ["--all", "--scenario", "playing"],
      diagnostic: /--all and --scenario cannot be combined/,
    },
  ];

  for (const invocation of invalidInvocations) {
    await assert.rejects(
      execFileAsync(
        process.execPath,
        ["scripts/capture-presentations.mjs", ...invocation.arguments],
        {
          cwd: new URL("..", import.meta.url),
          env: { ...process.env, PATH: "" },
        },
      ),
      (error) => {
        assert.equal(error.stdout, "");
        assert.match(error.stderr, invocation.diagnostic);
        return true;
      },
      invocation.arguments.join(" "),
    );
  }
});

test("visual-acceptance profile requires its destination and rejects incompatible options", async () => {
  const invalidInvocations = [
    {
      arguments: ["--profile", "visual-acceptance"],
      diagnostic: /--profile visual-acceptance requires --output/,
    },
    {
      arguments: [
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
        "--scenario",
        "playing",
      ],
      diagnostic:
        /--profile visual-acceptance cannot be combined with --scenario/,
    },
    {
      arguments: [
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
        "--all",
      ],
      diagnostic: /--profile visual-acceptance cannot be combined with --all/,
    },
    {
      arguments: [
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
        "--artwork",
        "cover.png",
      ],
      diagnostic:
        /--profile visual-acceptance cannot be combined with --artwork/,
    },
    {
      arguments: [
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
        "--resolution",
        "1920x1080",
      ],
      diagnostic:
        /--profile visual-acceptance cannot be combined with --resolution/,
    },
    {
      arguments: ["--profile", "brief", "--output", "captures"],
      diagnostic: /unknown capture profile: brief/,
    },
  ];

  for (const invocation of invalidInvocations) {
    await assert.rejects(
      execFileAsync(
        process.execPath,
        ["scripts/capture-presentations.mjs", ...invocation.arguments],
        {
          cwd: new URL("..", import.meta.url),
          env: { ...process.env, PATH: "" },
        },
      ),
      (error) => {
        assert.equal(error.stdout, "");
        assert.match(error.stderr, invocation.diagnostic);
        return true;
      },
      invocation.arguments.join(" "),
    );
  }
});

test("visual-acceptance profile publishes its maintained plan through reusable painted sessions", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-profile-capture-test."),
  );
  const binDirectory = path.join(taskDirectory, "bin");
  const outputDirectory = path.join(taskDirectory, "captures");
  const fakePngDirectory = path.join(taskDirectory, "pngs");
  const processLog = path.join(taskDirectory, "processes");
  const failureMarker = path.join(taskDirectory, "fail-profile");
  const runtimeRoot = await mkdtemp(path.join(tmpdir(), "rsc."));
  const plan = buildPresentationCapturePlan();
  await Promise.all([
    mkdir(binDirectory),
    mkdir(outputDirectory),
    mkdir(fakePngDirectory),
  ]);
  await Promise.all(
    REPRESENTATIVE_VIEWPORTS.map(async (viewport) => {
      const [width, height] = viewport.split("x").map(Number);
      await writeFile(
        path.join(fakePngDirectory, `${viewport}.png`),
        presentationCapturePngHeader(width, height),
      );
    }),
  );
  const { renderer: fakeRenderer } =
    await installPresentationCaptureFixtures(binDirectory);
  const firstCapturePath = path.join(outputDirectory, plan[0].fileName);
  await writeFile(firstCapturePath, "stale");
  const runProfile = (...arguments_) =>
    execFileAsync(
      process.execPath,
      [
        path.join(
          new URL("..", import.meta.url).pathname,
          "scripts/capture-presentations.mjs",
        ),
        "--profile",
        "visual-acceptance",
        "--output",
        outputDirectory,
        ...arguments_,
      ],
      {
        cwd: taskDirectory,
        env: {
          ...process.env,
          PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
          TMPDIR: runtimeRoot,
          ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY: fakePngDirectory,
          ROONSCAPE_CAPTURE_TEST_PROCESS_LOG: processLog,
          ROONSCAPE_CAPTURE_TEST_RENDERER: fakeRenderer,
          ROONSCAPE_CAPTURE_TEST_FAILURE_MARKER: failureMarker,
          ROONSCAPE_CAPTURE_TEST_LOG_STYLE: "profile",
        },
      },
    );

  try {
    await assert.rejects(runProfile(), /destination files already exist/);
    await assert.rejects(readFile(processLog), /ENOENT/);

    const { stdout, stderr } = await runProfile("--overwrite");
    const publishedPaths = stdout.trimEnd().split("\n");
    assert.equal(publishedPaths.length, plan.length);
    assert.deepEqual(
      publishedPaths.toSorted(),
      plan
        .map((capture) => path.join(outputDirectory, capture.fileName))
        .toSorted(),
    );
    assert.match(stderr, /Capturing Fixture Scenario playing at 1280x720/);
    assert.deepEqual(
      (await readdir(outputDirectory)).toSorted(),
      plan.map((capture) => capture.fileName).toSorted(),
    );
    assert.equal(
      (await readdir(outputDirectory)).includes("manifest.json"),
      false,
    );
    const processes = await readFile(processLog, "utf8");
    assert.equal((processes.match(/^selection\|/gm) ?? []).length, plan.length);
    assert.equal((processes.match(/^scrot\|/gm) ?? []).length, plan.length);
    const sessions = [...processes.matchAll(/^renderer\|(.+)$/gm)].map(
      (match) => match[1],
    );
    assert.equal(sessions.length, REPRESENTATIVE_VIEWPORTS.length * 4);
    assert.equal(new Set(sessions).size, sessions.length);
    for (const capture of plan) {
      assert.deepEqual(
        await readFile(path.join(outputDirectory, capture.fileName)),
        await readFile(path.join(fakePngDirectory, `${capture.viewport}.png`)),
      );
    }
    assert.deepEqual(await readdir(runtimeRoot), []);

    await rm(outputDirectory, { force: true, recursive: true });
    await mkdir(outputDirectory);
    await writeFile(failureMarker, "fail");
    await assert.rejects(runProfile(), (error) => {
      const completedPaths = error.stdout.trimEnd().split("\n");
      const completedScenarios = REQUIRED_SCENARIOS.slice(
        0,
        REQUIRED_SCENARIOS.indexOf("loading-with-content"),
      );
      assert.deepEqual(
        completedPaths,
        completedScenarios.map((scenario) =>
          path.join(outputDirectory, `1280x720--${scenario}.png`),
        ),
      );
      assert.match(
        error.stderr,
        new RegExp(
          `Visual-acceptance profile is incomplete \\(${completedScenarios.length}\\/${plan.length} captures completed\\)`,
        ),
      );
      assert.match(error.stderr, /Completed captures:/);
      assert.match(error.stderr, /1280x720--playing\.png/);
      assert.match(error.stderr, /1280x720--paused\.png/);
      return true;
    });
    assert.deepEqual(
      (await readdir(outputDirectory)).toSorted(),
      REQUIRED_SCENARIOS.slice(
        0,
        REQUIRED_SCENARIOS.indexOf("loading-with-content"),
      )
        .map((scenario) => `1280x720--${scenario}.png`)
        .toSorted(),
    );
    assert.deepEqual(await readdir(runtimeRoot), []);
  } finally {
    await Promise.all([
      rm(taskDirectory, { force: true, recursive: true }),
      rm(runtimeRoot, { force: true, recursive: true }),
    ]);
  }
});

test("ordinary all-scenario capture publishes its maintained set through one painted session per resolution", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-all-capture-test."),
  );
  const binDirectory = path.join(taskDirectory, "bin");
  const outputDirectory = path.join(taskDirectory, "captures");
  const fakePngDirectory = path.join(taskDirectory, "pngs");
  const processLog = path.join(taskDirectory, "processes");
  const failureMarker = path.join(taskDirectory, "fail-all");
  const customArtwork = path.join(taskDirectory, "Maintainer Cover.svg");
  const runtimeRoot = await mkdtemp(path.join(tmpdir(), "rsc."));
  await Promise.all([
    mkdir(binDirectory),
    mkdir(outputDirectory),
    mkdir(fakePngDirectory),
  ]);
  await Promise.all([
    writeFile(
      path.join(fakePngDirectory, "1280x720.png"),
      presentationCapturePngHeader(1280, 720),
    ),
    writeFile(
      path.join(fakePngDirectory, "1920x1080.png"),
      presentationCapturePngHeader(1920, 1080),
    ),
    writeFile(
      path.join(fakePngDirectory, "3840x2160.png"),
      presentationCapturePngHeader(3840, 2160),
    ),
    writeFile(
      customArtwork,
      await readFile(
        new URL("../src/shared/fixtures/artwork/light.svg", import.meta.url),
      ),
    ),
  ]);
  const { renderer: fakeRenderer } =
    await installPresentationCaptureFixtures(binDirectory);
  const runAll = (...arguments_) =>
    execFileAsync(
      process.execPath,
      [
        path.join(
          new URL("..", import.meta.url).pathname,
          "scripts/capture-presentations.mjs",
        ),
        "--all",
        "--output",
        outputDirectory,
        ...arguments_,
      ],
      {
        cwd: taskDirectory,
        env: {
          ...process.env,
          PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
          TMPDIR: runtimeRoot,
          ROONSCAPE_CAPTURE_TEST_FAILURE_MARKER: failureMarker,
          ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY: fakePngDirectory,
          ROONSCAPE_CAPTURE_TEST_PROCESS_LOG: processLog,
          ROONSCAPE_CAPTURE_TEST_RENDERER: fakeRenderer,
        },
      },
    );

  try {
    const { stdout } = await runAll();
    const canonicalPaths = ORDINARY_SCENARIOS.map((scenario) =>
      path.join(outputDirectory, `3840x2160--${scenario}.png`),
    );
    assert.equal(stdout, `${canonicalPaths.join("\n")}\n`);
    let processes = await readFile(processLog, "utf8");
    const canonicalArtworkByScenario = new Map(
      await Promise.all(
        buildPresentationCapturePlan()
          .filter(
            (capture) =>
              capture.variant === "matrix" &&
              capture.viewport === "3840x2160" &&
              ORDINARY_SCENARIOS.includes(capture.scenario),
          )
          .map(async (capture) => [
            capture.scenario,
            JSON.parse(
              await readFile(
                new URL(`../${capture.fixture}`, import.meta.url),
                "utf8",
              ),
            ).artwork?.path ?? "none",
          ]),
      ),
    );
    assert.deepEqual(
      [...processes.matchAll(/^selection\|([^|]+)\|(\d+)\|(.+)$/gm)].map(
        ([, scenario, revision, artwork]) => ({
          scenario,
          revision: Number(revision),
          artwork,
        }),
      ),
      ORDINARY_SCENARIOS.map((scenario, index) => ({
        scenario,
        revision: index + 1,
        artwork: canonicalArtworkByScenario.get(scenario),
      })),
    );
    assert.equal((processes.match(/^renderer\|/gm) ?? []).length, 1);
    assert.equal(
      (processes.match(/^painted\|/gm) ?? []).length,
      ORDINARY_SCENARIOS.length,
    );
    assert.equal(
      (processes.match(/^scrot\|/gm) ?? []).length,
      ORDINARY_SCENARIOS.length,
    );

    await rm(outputDirectory, { force: true, recursive: true });
    await mkdir(outputDirectory);
    await writeFile(processLog, "");
    const artworkHash = createHash("sha256")
      .update(await readFile(customArtwork))
      .digest("hex")
      .slice(0, 12);
    const { stdout: customStdout } = await runAll(
      "--artwork",
      customArtwork,
      "--resolution",
      "1280x720",
      "--resolution",
      "1920x1080",
    );
    const expectedCustomPaths = ["1280x720", "1920x1080"].flatMap((viewport) =>
      ORDINARY_SCENARIOS.map((scenario) =>
        path.join(
          outputDirectory,
          `${viewport}--${scenario}${
            CUSTOM_ARTWORK_SCENARIOS.has(scenario)
              ? `--maintainer-cover-svg--${artworkHash}`
              : ""
          }.png`,
        ),
      ),
    );
    assert.equal(customStdout, `${expectedCustomPaths.join("\n")}\n`);
    processes = await readFile(processLog, "utf8");
    assert.deepEqual(
      [...processes.matchAll(/^renderer\|(.+)$/gm)].map((match) => match[1]),
      ["1280x720", "1920x1080"],
    );
    const selections = [
      ...processes.matchAll(/^selection\|([^|]+)\|(\d+)\|(.+)$/gm),
    ].map(([, scenario, revision, artwork]) => ({
      scenario,
      revision: Number(revision),
      artwork,
      customArtwork: artwork.startsWith(runtimeRoot),
    }));
    assert.deepEqual(
      selections.map(({ scenario }) => scenario),
      [ORDINARY_SCENARIOS, ORDINARY_SCENARIOS].flat(),
    );
    assert.deepEqual(
      selections.map(({ revision }) => revision),
      [
        ORDINARY_SCENARIOS.map((_, index) => index + 1),
        ORDINARY_SCENARIOS.map((_, index) => index + 1),
      ].flat(),
    );
    assert.ok(
      selections
        .filter(({ scenario }) => !CUSTOM_ARTWORK_SCENARIOS.has(scenario))
        .every(
          ({ scenario, artwork }) =>
            artwork === canonicalArtworkByScenario.get(scenario),
        ),
      "incompatible scenarios should retain canonical fixture content",
    );
    assert.deepEqual(
      processes
        .split("\n")
        .filter((line) => /^(selection|painted|scrot)\|/.test(line))
        .map((line) => line.slice(0, line.indexOf("|"))),
      Array.from({ length: ORDINARY_SCENARIOS.length * 2 }, () => [
        "selection",
        "painted",
        "scrot",
      ]).flat(),
      "each scenario should paint before its capture is published",
    );
    assert.deepEqual(
      selections
        .filter(({ customArtwork: custom }) => custom)
        .map(({ scenario }) => scenario),
      [
        ORDINARY_SCENARIOS.filter((scenario) =>
          CUSTOM_ARTWORK_SCENARIOS.has(scenario),
        ),
        ORDINARY_SCENARIOS.filter((scenario) =>
          CUSTOM_ARTWORK_SCENARIOS.has(scenario),
        ),
      ].flat(),
    );
    assert.deepEqual(
      (await readdir(outputDirectory)).toSorted(),
      [
        ...expectedCustomPaths.map((capturePath) => path.basename(capturePath)),
      ].toSorted(),
    );

    await rm(outputDirectory, { force: true, recursive: true });
    await mkdir(outputDirectory);
    await writeFile(processLog, "");
    await writeFile(failureMarker, "fail");
    await assert.rejects(runAll("--resolution", "1280x720"), (error) => {
      const completedScenarios = ORDINARY_SCENARIOS.slice(
        0,
        ORDINARY_SCENARIOS.indexOf("loading-with-content"),
      );
      assert.equal(
        error.stdout,
        `${completedScenarios
          .map((scenario) =>
            path.join(outputDirectory, `1280x720--${scenario}.png`),
          )
          .join("\n")}\n`,
      );
      assert.match(
        error.stderr,
        new RegExp(
          `All-scenario capture is incomplete \\(${completedScenarios.length}\\/${ORDINARY_SCENARIOS.length} captures completed\\)`,
        ),
      );
      assert.match(error.stderr, /Completed captures:/);
      return true;
    });
    assert.deepEqual(
      (await readdir(outputDirectory)).toSorted(),
      ORDINARY_SCENARIOS.slice(
        0,
        ORDINARY_SCENARIOS.indexOf("loading-with-content"),
      )
        .map((scenario) => `1280x720--${scenario}.png`)
        .toSorted(),
    );

    for (const scenario of ORDINARY_SCENARIOS.slice(2)) {
      await writeFile(
        path.join(outputDirectory, `1280x720--${scenario}.png`),
        "stale",
      );
    }
    await assert.rejects(
      runAll("--resolution", "1280x720", "--overwrite"),
      new RegExp(
        `All-scenario capture is incomplete \\(${ORDINARY_SCENARIOS.indexOf("loading-with-content")}\\/${ORDINARY_SCENARIOS.length} captures completed\\)`,
      ),
    );
    assert.deepEqual(
      await readFile(path.join(outputDirectory, "1280x720--playing.png")),
      await readFile(path.join(fakePngDirectory, "1280x720.png")),
    );
    assert.equal(
      await readFile(
        path.join(outputDirectory, "1280x720--loading-with-content.png"),
        "utf8",
      ),
      "stale",
      "a failed overwrite may leave an intentionally incomplete mixture of refreshed and stale captures",
    );
    assert.deepEqual(await readdir(runtimeRoot), []);
  } finally {
    await Promise.all([
      rm(taskDirectory, { force: true, recursive: true }),
      rm(runtimeRoot, { force: true, recursive: true }),
    ]);
  }
});

test("focused capture waits for its painted revision and publishes one validated 4K PNG", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-focused-capture-test."),
  );
  const binDirectory = path.join(taskDirectory, "bin");
  const workDirectory = path.join(taskDirectory, "work");
  const runtimeRoot = await mkdtemp(path.join(tmpdir(), "rsc."));
  const fakePngDirectory = path.join(taskDirectory, "pngs");
  const buildFailure = path.join(taskDirectory, "build-failure");
  const processLog = path.join(taskDirectory, "processes");
  const customArtwork = path.join(taskDirectory, "My unsafe ! Cover.PNG");
  const canonicalPlayingPath = path.join(
    new URL("..", import.meta.url).pathname,
    "src/shared/fixtures/playing.json",
  );
  const canonicalPlaying = await readFile(canonicalPlayingPath);
  await Promise.all([
    mkdir(binDirectory),
    mkdir(workDirectory),
    mkdir(fakePngDirectory),
  ]);
  await Promise.all([
    writeFile(
      path.join(fakePngDirectory, "3840x2160.png"),
      presentationCapturePngHeader(3840, 2160),
    ),
    writeFile(
      path.join(fakePngDirectory, "1920x1080.png"),
      presentationCapturePngHeader(1920, 1080),
    ),
    writeFile(
      path.join(fakePngDirectory, "1280x720.png"),
      presentationCapturePngHeader(1280, 720),
    ),
    writeFile(
      customArtwork,
      await readFile(
        new URL("../src/shared/fixtures/artwork/light.svg", import.meta.url),
      ),
    ),
  ]);
  const { renderer: fakeRenderer } =
    await installPresentationCaptureFixtures(binDirectory);

  const runFocusedCapture = (...arguments_) =>
    execFileAsync(
      process.execPath,
      [
        path.join(
          new URL("..", import.meta.url).pathname,
          "scripts/capture-presentations.mjs",
        ),
        "--scenario",
        "playing",
        ...arguments_,
      ],
      {
        cwd: workDirectory,
        env: {
          ...process.env,
          PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
          TMPDIR: runtimeRoot,
          ROONSCAPE_CAPTURE_TEST_BUILD_FAILURE: buildFailure,
          ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY: fakePngDirectory,
          ROONSCAPE_CAPTURE_TEST_PROCESS_LOG: processLog,
          ROONSCAPE_CAPTURE_TEST_RENDERER: fakeRenderer,
          ROONSCAPE_CAPTURE_TEST_LOG_STYLE: "focused",
          ROONSCAPE_CAPTURE_TYPOGRAPHY: "fallback",
          ROONSCAPE_DIAGNOSTICS: "1",
          ROONSCAPE_DISPLAY_CONFIG: "/inherited/display.json",
          ROONSCAPE_FIXTURE_AUTO_CLOSE_MS: "1",
        },
      },
    );

  try {
    const { stdout, stderr } = await runFocusedCapture();
    const finalPath = path.join(workDirectory, "3840x2160--playing.png");
    assert.equal(stdout, `${finalPath}\n`);
    assert.match(stderr, /Capturing Fixture Scenario playing at 3840x2160/);
    assert.deepEqual(
      await readFile(finalPath),
      await readFile(path.join(fakePngDirectory, "3840x2160.png")),
    );
    const processes = await readFile(processLog, "utf8");
    assert.match(
      processes,
      /cargo-preflight\|build --locked --package roonscape-renderer/,
    );
    assert.ok(
      processes.indexOf("cargo-preflight|") < processes.indexOf("Xvfb|"),
      "the complete renderer build should be checked before renderer work",
    );
    assert.match(processes, /Xvfb\|.*3840x2160x24/);
    assert.match(
      processes,
      /renderer\|1\|3840x2160\|\S+\|0\|unset\|unset\|unset/,
    );
    assert.match(
      processes,
      /scrot\|--autoselect 0,0,3840,2160 --overwrite \/.*\.png/,
    );
    const temporaryCapturePath = processes.match(
      /scrot\|--autoselect 0,0,3840,2160 --overwrite ([^\n]+)/,
    )?.[1];
    assert.ok(
      temporaryCapturePath?.startsWith(
        `${workDirectory}${path.sep}.roonscape-capture.`,
      ),
      "the validated temporary capture should share the destination filesystem",
    );
    assert.match(processes, /renderer-stopped/);
    assert.match(processes, /Xvfb-stopped/);
    assert.deepEqual(
      processes
        .split("\n")
        .filter((line) => /^(selection|painted|scrot)\|/.test(line))
        .map((line) => line.slice(0, line.indexOf("|"))),
      ["selection", "painted", "scrot"],
      "the focused capture should wait for painted readiness",
    );
    assert.deepEqual(await readdir(workDirectory), ["3840x2160--playing.png"]);
    assert.deepEqual(await readdir(runtimeRoot), []);

    await rm(finalPath);
    const artworkHash = createHash("sha256")
      .update(await readFile(customArtwork))
      .digest("hex")
      .slice(0, 12);
    const customFileName = `1280x720--playing--my-unsafe-cover-png--${artworkHash}.png`;
    const { stdout: customStdout } = await runFocusedCapture(
      "--artwork",
      customArtwork,
      "--resolution",
      "1280x720",
    );
    const customCapturePath = path.join(workDirectory, customFileName);
    assert.equal(customStdout, `${customCapturePath}\n`);
    assert.deepEqual(
      await readFile(customCapturePath),
      await readFile(path.join(fakePngDirectory, "1280x720.png")),
    );
    const customSelection = [
      ...(await readFile(processLog, "utf8")).matchAll(/^selection\|(.*)$/gm),
    ]
      .map((match) => JSON.parse(match[1]))
      .find((selection) => selection.observedArtworkHash === artworkHash);
    assert.ok(
      customSelection,
      "the renderer should receive the custom artwork",
    );
    assert.notEqual(customSelection.snapshot.artwork.path, customArtwork);
    assert.ok(
      customSelection.snapshot.artwork.path.startsWith(runtimeRoot),
      "the renderer should receive a task-owned copy of validated artwork",
    );
    const expectedSnapshot = JSON.parse(canonicalPlaying);
    expectedSnapshot.revision = customSelection.revision;
    expectedSnapshot.artwork = customSelection.snapshot.artwork;
    assert.deepEqual(customSelection.snapshot, expectedSnapshot);
    assert.deepEqual(await readFile(canonicalPlayingPath), canonicalPlaying);
    await rm(customCapturePath);
    const { stdout: repeatedCustomStdout } = await runFocusedCapture(
      "--artwork",
      customArtwork,
      "--resolution",
      "1280x720",
    );
    assert.equal(repeatedCustomStdout, customStdout);
    await rm(customCapturePath);

    const processLogBeforeBuildFailure = await readFile(processLog, "utf8");
    await writeFile(buildFailure, "fail");
    await assert.rejects(
      runFocusedCapture(),
      /renderer build preflight failed:[\s\S]*missing renderer input/,
    );
    const buildFailureProcesses = (await readFile(processLog, "utf8")).slice(
      processLogBeforeBuildFailure.length,
    );
    assert.match(buildFailureProcesses, /^cargo-preflight\|/);
    assert.doesNotMatch(buildFailureProcesses, /Xvfb\||renderer\|/);
    await rm(buildFailure);

    const { stdout: repeatedStdout, stderr: repeatedStderr } =
      await runFocusedCapture(
        "--resolution",
        "1280x720",
        "--resolution",
        "1920x1080",
        "--output",
        "review/captures",
      );
    const outputDirectory = path.join(workDirectory, "review/captures");
    const plannedPaths = [
      path.join(outputDirectory, "1280x720--playing.png"),
      path.join(outputDirectory, "1920x1080--playing.png"),
    ];
    assert.equal(repeatedStdout, `${plannedPaths.join("\n")}\n`);
    assert.match(repeatedStderr, /playing at 1280x720/);
    assert.match(repeatedStderr, /playing at 1920x1080/);
    assert.deepEqual((await readdir(outputDirectory)).sort(), [
      "1280x720--playing.png",
      "1920x1080--playing.png",
    ]);

    const processLogBeforeCollision = await readFile(processLog, "utf8");
    await assert.rejects(
      runFocusedCapture(
        "--resolution",
        "1280x720",
        "--resolution",
        "1920x1080",
        "--output",
        "review/captures",
      ),
      /destination files already exist:[\s\S]*1280x720--playing\.png[\s\S]*1920x1080--playing\.png/,
    );
    assert.equal(await readFile(processLog, "utf8"), processLogBeforeCollision);

    await rm(plannedPaths[0]);
    await mkdir(plannedPaths[0]);
    await assert.rejects(
      runFocusedCapture(
        "--resolution",
        "1280x720",
        "--resolution",
        "1920x1080",
        "--output",
        "review/captures",
        "--overwrite",
      ),
      /destination is not a replaceable file:.*1280x720--playing\.png/,
    );
    assert.equal(await readFile(processLog, "utf8"), processLogBeforeCollision);
    await rm(plannedPaths[0], { recursive: true });
    await writeFile(plannedPaths[0], "stale");
    await runFocusedCapture(
      "--resolution",
      "1280x720",
      "--resolution",
      "1920x1080",
      "--output",
      "review/captures",
      "--overwrite",
    );
    assert.deepEqual(
      await readFile(plannedPaths[0]),
      await readFile(path.join(fakePngDirectory, "1280x720.png")),
    );

    await rm(outputDirectory, { force: true, recursive: true });
    await writeFile(
      path.join(fakePngDirectory, "3840x2160.png"),
      presentationCapturePngHeader(1920, 1080),
    );
    await assert.rejects(
      runFocusedCapture(),
      /is 1920x1080; expected 3840x2160/,
    );
    assert.deepEqual(await readdir(workDirectory), ["review"]);
    assert.deepEqual(await readdir(runtimeRoot), []);
  } finally {
    await Promise.all([
      rm(taskDirectory, { force: true, recursive: true }),
      rm(runtimeRoot, { force: true, recursive: true }),
    ]);
  }
});

test("focused capture rejects an unknown Fixture Scenario before launching tools", async () => {
  await assert.rejects(
    execFileAsync(
      process.execPath,
      ["scripts/capture-presentations.mjs", "--scenario", "not-maintained"],
      { cwd: new URL("..", import.meta.url) },
    ),
    (error) => {
      assert.equal(error.stdout, "");
      assert.match(
        error.stderr,
        /unknown Fixture Scenario identifier: not-maintained/,
      );
      return true;
    },
  );
});

test("focused capture keeps the two ordinary-set omissions individually selectable", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-omitted-scenario-test."),
  );

  try {
    for (const scenario of ["light-artwork", "non-square-artwork"]) {
      await assert.rejects(
        execFileAsync(
          process.execPath,
          [
            path.join(
              new URL("..", import.meta.url).pathname,
              "scripts/capture-presentations.mjs",
            ),
            "--scenario",
            scenario,
          ],
          { cwd: taskDirectory, env: { ...process.env, PATH: "" } },
        ),
        (error) => {
          assert.match(error.stderr, /Presentation Capture preflight failed/);
          assert.doesNotMatch(error.stderr, /unknown Fixture Scenario/);
          return true;
        },
      );
    }
  } finally {
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

test("focused capture rejects custom artwork for incompatible Fixture Scenarios", async () => {
  const artwork = path.join(
    new URL("..", import.meta.url).pathname,
    "src/shared/fixtures/artwork/playing.svg",
  );
  const incompatibleScenarios = [
    "loading-without-content",
    "idle",
    "pairing-required",
    "disconnected",
    "output-unavailable",
    "playing-without-content",
    "paused-without-content",
    "missing-artwork",
    "non-square-artwork",
    "light-artwork",
  ];

  for (const scenario of incompatibleScenarios) {
    await assert.rejects(
      execFileAsync(
        process.execPath,
        [
          "scripts/capture-presentations.mjs",
          "--scenario",
          scenario,
          "--artwork",
          artwork,
        ],
        {
          cwd: new URL("..", import.meta.url),
          env: { ...process.env, PATH: "" },
        },
      ),
      new RegExp(`--artwork is incompatible with Fixture Scenario ${scenario}`),
      scenario,
    );
  }
});

test("focused capture accepts custom artwork for every compatible Fixture Scenario", async () => {
  const artwork = path.join(
    new URL("..", import.meta.url).pathname,
    "src/shared/fixtures/artwork/playing.svg",
  );
  const compatibleScenarios = [
    "playing",
    "paused",
    "loading-with-content",
    "missing-metadata",
    "missing-artist",
    "missing-album",
    "long-metadata",
    "extreme-metadata",
    "indeterminate-progress",
  ];

  for (const scenario of compatibleScenarios) {
    await assert.rejects(
      execFileAsync(
        process.execPath,
        [
          "scripts/capture-presentations.mjs",
          "--scenario",
          scenario,
          "--artwork",
          artwork,
        ],
        {
          cwd: new URL("..", import.meta.url),
          env: { ...process.env, PATH: "" },
        },
      ),
      (error) => {
        assert.match(error.stderr, /required executable is unavailable: Xvfb/);
        assert.doesNotMatch(error.stderr, /--artwork is incompatible/);
        return true;
      },
      scenario,
    );
  }
});

test("focused capture rejects unusable custom artwork before renderer work", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-custom-artwork-test."),
  );
  const missingArtwork = path.join(taskDirectory, "missing.png");
  const emptyArtwork = path.join(taskDirectory, "empty.png");
  const directoryArtwork = path.join(taskDirectory, "directory.png");
  const unreadableArtwork = path.join(taskDirectory, "unreadable.png");
  await writeFile(emptyArtwork, "");
  await mkdir(directoryArtwork);
  await writeFile(unreadableArtwork, "not empty");
  await chmod(unreadableArtwork, 0o000);

  const invalidArtwork = [
    [missingArtwork, /custom artwork does not exist:/],
    [emptyArtwork, /custom artwork is empty:/],
    [directoryArtwork, /custom artwork is not a file:/],
    [unreadableArtwork, /custom artwork is unreadable:/],
  ];

  try {
    for (const [artwork, diagnostic] of invalidArtwork) {
      await assert.rejects(
        execFileAsync(
          process.execPath,
          [
            "scripts/capture-presentations.mjs",
            "--scenario",
            "playing",
            "--artwork",
            artwork,
          ],
          {
            cwd: new URL("..", import.meta.url),
            env: { ...process.env, PATH: "" },
          },
        ),
        (error) => {
          assert.match(error.stderr, diagnostic);
          assert.doesNotMatch(error.stderr, /Capturing Fixture Scenario/);
          return true;
        },
        artwork,
      );
    }
  } finally {
    await chmod(unreadableArtwork, 0o600);
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

test("focused capture rejects artwork unsupported by the native image pipeline before scrot", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-unsupported-artwork-test."),
  );
  const artwork = path.join(taskDirectory, "looks-valid.png");
  await writeFile(artwork, "not an image");

  try {
    await assert.rejects(
      execFileAsync(
        process.execPath,
        [
          path.join(
            new URL("..", import.meta.url).pathname,
            "scripts/capture-presentations.mjs",
          ),
          "--scenario",
          "playing",
          "--artwork",
          artwork,
          "--resolution",
          "1280x720",
        ],
        {
          cwd: taskDirectory,
          env: process.env,
          timeout: 300_000,
        },
      ),
      (error) => {
        assert.match(
          error.stderr,
          /could not decode or derive a palette from artwork/,
        );
        return true;
      },
    );
    assert.deepEqual(await readdir(taskDirectory), ["looks-valid.png"]);
  } finally {
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

test("focused capture publishes a native PNG with valid custom artwork regardless of extension", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-valid-artwork-test."),
  );
  const artwork = path.join(taskDirectory, "Proposed cover.unknown");
  const artworkContents = await readFile(
    new URL("../src/shared/fixtures/artwork/light.svg", import.meta.url),
  );
  await writeFile(artwork, artworkContents);
  const artworkHash = createHash("sha256")
    .update(artworkContents)
    .digest("hex")
    .slice(0, 12);
  const capturePath = path.join(
    taskDirectory,
    `1280x720--playing--proposed-cover-unknown--${artworkHash}.png`,
  );

  try {
    const { stdout } = await execFileAsync(
      process.execPath,
      [
        path.join(
          new URL("..", import.meta.url).pathname,
          "scripts/capture-presentations.mjs",
        ),
        "--scenario",
        "playing",
        "--artwork",
        artwork,
        "--resolution",
        "1280x720",
      ],
      {
        cwd: taskDirectory,
        env: process.env,
        timeout: 300_000,
      },
    );
    assert.equal(stdout, `${capturePath}\n`);
    const capture = await readFile(capturePath);
    assert.equal(capture.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
    assert.equal(capture.readUInt32BE(16), 1280);
    assert.equal(capture.readUInt32BE(20), 720);
  } finally {
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

test("focused capture reports missing dependencies together before renderer work", async () => {
  await assert.rejects(
    execFileAsync(
      process.execPath,
      ["scripts/capture-presentations.mjs", "--scenario", "playing"],
      {
        cwd: new URL("..", import.meta.url),
        env: { ...process.env, PATH: "" },
      },
    ),
    (error) => {
      assert.equal(error.stdout, "");
      assert.match(error.stderr, /required executable is unavailable: Xvfb/);
      assert.match(
        error.stderr,
        /required executable is unavailable: xwininfo/,
      );
      assert.match(error.stderr, /required executable is unavailable: scrot/);
      assert.match(error.stderr, /required executable is unavailable: cargo/);
      assert.doesNotMatch(error.stderr, /Capturing Fixture Scenario/);
      return true;
    },
  );
});
