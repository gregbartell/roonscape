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

const execFileAsync = promisify(execFile);

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
        return (
          fixture.progress.positionSeconds / fixture.progress.durationSeconds
        );
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
      diagnostic: /--resolution requires --scenario/,
    },
    {
      arguments: ["--overwrite"],
      diagnostic: /--overwrite requires --scenario/,
    },
    {
      arguments: ["--artwork", "cover.png"],
      diagnostic: /--artwork requires --scenario/,
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
      arguments: ["--scenario", "playing", "--settle-ms", "1500"],
      diagnostic: /--scenario cannot be combined with legacy capture options/,
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
        pngHeader(width, height),
      );
    }),
  );
  await installFakeCaptureDisplay(binDirectory);
  await executable(
    path.join(binDirectory, "cargo"),
    '#!/bin/sh\nif [ "$1" = "build" ]; then\n  printf "cargo-preflight|%s\\n" "$*" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\n  exit 0\nfi\nprintf "renderer|%s|%s|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" "${ROONSCAPE_CAPTURE_TYPOGRAPHY-automatic}" "$ROONSCAPE_DIAGNOSTICS" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\nexec "${NODE:-node}" "$ROONSCAPE_CAPTURE_TEST_RENDERER"\n',
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\nprintf "scrot|%s|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" "$4" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\n[ "$1" = "--window" ] && [ "$2" = "4242" ] && [ "$3" = "--overwrite" ] || exit 2\ncp "$ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY/$ROONSCAPE_CAPTURE_VIEWPORT.png" "$4"\n',
  );
  const fakeRenderer = path.join(taskDirectory, "renderer.mjs");
  await writeFile(
    fakeRenderer,
    '#!/usr/bin/env node\nimport { once } from "node:events";\nimport { existsSync } from "node:fs";\nimport { appendFile } from "node:fs/promises";\nimport { createConnection } from "node:net";\nimport { createInterface } from "node:readline";\nconst connection = createConnection(process.env.ROONSCAPE_CAPTURE_CONTROL);\nawait once(connection, "connect");\nfor await (const line of createInterface({ input: connection })) {\n  const selection = JSON.parse(line);\n  await appendFile(process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG, `selection|${selection.scenario}|${selection.revision}\\n`);\n  if (existsSync(process.env.ROONSCAPE_CAPTURE_TEST_FAILURE_MARKER) && selection.scenario === "loading-with-content") process.exit(9);\n  await appendFile(process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG, `painted|${selection.scenario}|${selection.revision}\\n`);\n  connection.write(`${JSON.stringify({ type: "painted", scenario: selection.scenario, revision: selection.revision })}\\n`);\n}\n',
  );
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
      assert.deepEqual(completedPaths, [
        path.join(outputDirectory, "1280x720--playing.png"),
        path.join(outputDirectory, "1280x720--paused.png"),
      ]);
      assert.match(
        error.stderr,
        /Visual-acceptance profile is incomplete \(2\/210 captures completed\)/,
      );
      assert.match(error.stderr, /Completed captures:/);
      assert.match(error.stderr, /1280x720--playing\.png/);
      assert.match(error.stderr, /1280x720--paused\.png/);
      return true;
    });
    assert.deepEqual((await readdir(outputDirectory)).toSorted(), [
      "1280x720--paused.png",
      "1280x720--playing.png",
    ]);
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
      pngHeader(1280, 720),
    ),
    writeFile(
      path.join(fakePngDirectory, "1920x1080.png"),
      pngHeader(1920, 1080),
    ),
    writeFile(
      path.join(fakePngDirectory, "3840x2160.png"),
      pngHeader(3840, 2160),
    ),
    writeFile(
      customArtwork,
      await readFile(
        new URL("../src/shared/fixtures/artwork/light.svg", import.meta.url),
      ),
    ),
  ]);
  await installFakeCaptureDisplay(binDirectory);
  await executable(
    path.join(binDirectory, "cargo"),
    '#!/bin/sh\nif [ "$1" = "build" ]; then exit 0; fi\nprintf "renderer|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\nexec "${NODE:-node}" "$ROONSCAPE_CAPTURE_TEST_RENDERER"\n',
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\nprintf "scrot|%s|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" "$4" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\n[ "$1" = "--window" ] && [ "$2" = "4242" ] && [ "$3" = "--overwrite" ] || exit 2\ncp "$ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY/$ROONSCAPE_CAPTURE_VIEWPORT.png" "$4"\n',
  );
  const fakeRenderer = path.join(taskDirectory, "renderer.mjs");
  await writeFile(
    fakeRenderer,
    '#!/usr/bin/env node\nimport { once } from "node:events";\nimport { existsSync } from "node:fs";\nimport { appendFile } from "node:fs/promises";\nimport { createConnection } from "node:net";\nimport { createInterface } from "node:readline";\nconst connection = createConnection(process.env.ROONSCAPE_CAPTURE_CONTROL);\nawait once(connection, "connect");\nfor await (const line of createInterface({ input: connection })) {\n  const selection = JSON.parse(line);\n  const artwork = selection.snapshot.artwork?.path ?? "none";\n  await appendFile(process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG, `selection|${selection.scenario}|${selection.revision}|${artwork}\\n`);\n  if (existsSync(process.env.ROONSCAPE_CAPTURE_TEST_FAILURE_MARKER) && selection.scenario === "loading-with-content") process.exit(9);\n  await appendFile(process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG, `painted|${selection.scenario}|${selection.revision}\\n`);\n  connection.write(`${JSON.stringify({ type: "painted", scenario: selection.scenario, revision: selection.revision })}\\n`);\n}\n',
  );
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
    assert.equal((processes.match(/^painted\|/gm) ?? []).length, 17);
    assert.equal((processes.match(/^scrot\|/gm) ?? []).length, 17);

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
      "the eight incompatible scenarios should retain canonical fixture content",
    );
    assert.deepEqual(
      processes
        .split("\n")
        .filter((line) => /^(selection|painted|scrot)\|/.test(line))
        .map((line) => line.slice(0, line.indexOf("|"))),
      Array.from({ length: 34 }, () => [
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
      assert.equal(
        error.stdout,
        `${path.join(outputDirectory, "1280x720--playing.png")}\n${path.join(outputDirectory, "1280x720--paused.png")}\n`,
      );
      assert.match(
        error.stderr,
        /All-scenario capture is incomplete \(2\/17 captures completed\)/,
      );
      assert.match(error.stderr, /Completed captures:/);
      return true;
    });
    assert.deepEqual((await readdir(outputDirectory)).toSorted(), [
      "1280x720--paused.png",
      "1280x720--playing.png",
    ]);

    for (const scenario of ORDINARY_SCENARIOS.slice(2)) {
      await writeFile(
        path.join(outputDirectory, `1280x720--${scenario}.png`),
        "stale",
      );
    }
    await assert.rejects(
      runAll("--resolution", "1280x720", "--overwrite"),
      /All-scenario capture is incomplete \(2\/17 captures completed\)/,
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
      pngHeader(3840, 2160),
    ),
    writeFile(
      path.join(fakePngDirectory, "1920x1080.png"),
      pngHeader(1920, 1080),
    ),
    writeFile(
      path.join(fakePngDirectory, "1280x720.png"),
      pngHeader(1280, 720),
    ),
    writeFile(
      customArtwork,
      await readFile(
        new URL("../src/shared/fixtures/artwork/light.svg", import.meta.url),
      ),
    ),
  ]);
  await installFakeCaptureDisplay(binDirectory, { logStop: true });
  await executable(
    path.join(binDirectory, "cargo"),
    '#!/bin/sh\nif [ "$1" = "build" ]; then\n  printf "cargo-preflight|%s\\n" "$*" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\n  if [ -f "$ROONSCAPE_CAPTURE_TEST_BUILD_FAILURE" ]; then\n    printf "missing renderer input\\n" >&2\n    exit 7\n  fi\n  exit 0\nfi\nprintf "renderer|%s|%s|%s|%s|%s|%s|%s\\n" "$ROONSCAPE_STATIC_FIXTURE" "$ROONSCAPE_CAPTURE_VIEWPORT" "$ROONSCAPE_CAPTURE_CONTROL" "${ROONSCAPE_DIAGNOSTICS-unset}" "${ROONSCAPE_CAPTURE_TYPOGRAPHY-unset}" "${ROONSCAPE_FIXTURE_AUTO_CLOSE_MS-unset}" "${ROONSCAPE_DISPLAY_CONFIG-unset}" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\nexec "${NODE:-node}" "$ROONSCAPE_CAPTURE_TEST_RENDERER"\n',
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\nprintf "scrot|%s|%s\\n" "$*" "$(/usr/bin/date +%s%3N)" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\n[ "$1" = "--window" ] && [ "$2" = "4242" ] && [ "$3" = "--overwrite" ] || exit 2\ncp "$ROONSCAPE_CAPTURE_TEST_PNG_DIRECTORY/$ROONSCAPE_CAPTURE_VIEWPORT.png" "$4"\n',
  );
  const fakeRenderer = path.join(taskDirectory, "renderer.mjs");
  await writeFile(
    fakeRenderer,
    '#!/usr/bin/env node\nimport { createHash } from "node:crypto";\nimport { once } from "node:events";\nimport { appendFile, readFile } from "node:fs/promises";\nimport { createConnection } from "node:net";\nimport { createInterface } from "node:readline";\nconst log = process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG;\nconst connection = createConnection(process.env.ROONSCAPE_CAPTURE_CONTROL);\nprocess.once("SIGTERM", async () => { await appendFile(log, "renderer-stopped\\n"); process.exit(0); });\nawait once(connection, "connect");\nconst lines = createInterface({ input: connection })[Symbol.asyncIterator]();\nconst selection = JSON.parse((await lines.next()).value);\nif (selection.type !== "select" || selection.scenario !== "playing" || selection.revision !== selection.snapshot.revision) process.exit(2);\nconst observedArtworkHash = selection.snapshot.artwork === null ? null : createHash("sha256").update(await readFile(selection.snapshot.artwork.path)).digest("hex").slice(0, 12);\nawait appendFile(log, `selection|${JSON.stringify({ ...selection, observedArtworkHash })}\\npainted|${Date.now()}\\n`);\nconnection.write(`${JSON.stringify({ type: "painted", scenario: selection.scenario, revision: selection.revision })}\\n`);\nawait once(connection, "close");\nawait appendFile(log, "renderer-stopped\\n");\n',
  );

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
    assert.match(processes, /scrot\|--window 4242 --overwrite \/.*\.png\|\d+/);
    const temporaryCapturePath = processes.match(
      /scrot\|--window 4242 --overwrite ([^|]+)\|\d+/,
    )?.[1];
    assert.ok(
      temporaryCapturePath?.startsWith(
        `${workDirectory}${path.sep}.roonscape-capture.`,
      ),
      "the validated temporary capture should share the destination filesystem",
    );
    assert.match(processes, /renderer-stopped/);
    assert.match(processes, /Xvfb-stopped/);
    const paintedAt = Number(processes.match(/painted\|(\d+)/)?.[1]);
    const capturedAt = Number(processes.match(/scrot\|[^\n]+\|(\d+)/)?.[1]);
    assert.ok(
      capturedAt - paintedAt >= 0 && capturedAt - paintedAt < 1_000,
      `capture should follow painted readiness without a settle delay; observed ${capturedAt - paintedAt}ms`,
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
      pngHeader(1920, 1080),
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

test("capture command orchestrates one native fixture capture and records its manifest", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-presentation-test."),
  );
  const binDirectory = path.join(taskDirectory, "bin");
  const outputDirectory = path.join(taskDirectory, "captures");
  const fakePng = path.join(taskDirectory, "fake.png");
  const rendererEnvironment = path.join(taskDirectory, "renderer-environment");
  const rendererArguments = path.join(taskDirectory, "renderer-arguments");
  const windowInspectionAttempts = path.join(
    taskDirectory,
    "window-inspection-attempts",
  );
  const displayConfiguration = path.join(
    taskDirectory,
    "display-configuration",
  );
  await mkdir(binDirectory);
  await writeFile(fakePng, pngHeader(1280, 720));
  await executable(
    path.join(binDirectory, "Xvfb"),
    '#!/bin/sh\ndisplay_number="${1#:}"\nsocket="/tmp/.X11-unix/X${display_number}"\nmkdir -p /tmp/.X11-unix\ntouch "$socket"\ntrap \'rm -f "$socket"; exit 0\' TERM INT EXIT\nwhile :; do /usr/bin/sleep 1; done\n',
  );
  await executable(
    path.join(binDirectory, "cargo"),
    '#!/bin/sh\nconfiguration_file="$7"\nif [ -S "$ROONSCAPE_SOCKET" ]; then\n  publisher_state=ready\nelse\n  publisher_state=missing\nfi\nprintf "%s|%s|%s|%s|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" "$ROONSCAPE_CAPTURE_TYPOGRAPHY" "$ROONSCAPE_DIAGNOSTICS" "${ROONSCAPE_DISPLAY_CONFIG-unset}" "$publisher_state" > "$ROONSCAPE_CAPTURE_TEST_RENDERER_ENVIRONMENT"\nprintf "%s\\n" "$@" > "$ROONSCAPE_CAPTURE_TEST_RENDERER_ARGUMENTS"\ncp "$configuration_file" "$ROONSCAPE_CAPTURE_TEST_DISPLAY_CONFIGURATION"\ntrap \'exit 0\' TERM INT\nwhile :; do /usr/bin/sleep 1; done\n',
  );
  await executable(
    path.join(binDirectory, "xwininfo"),
    '#!/bin/sh\nattempts_file="$ROONSCAPE_CAPTURE_TEST_WINDOW_INSPECTION_ATTEMPTS"\nattempt=0\n[ ! -f "$attempts_file" ] || read -r attempt < "$attempts_file"\nattempt=$((attempt + 1))\nprintf "%s\\n" "$attempt" > "$attempts_file"\nif [ "$attempt" -eq 1 ]; then\n  width=1\n  height=1\nelse\n  width=1280\n  height=720\nfi\nprintf \'xwininfo: Window id: 4242 "RoonScape"\\n  Width: %s\\n  Height: %s\\n\' "$width" "$height"\n',
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\n[ "$1" = "--window" ] && [ "$2" = "$ROONSCAPE_CAPTURE_TEST_WINDOW_ID" ] && [ "$3" = "--overwrite" ] || exit 2\n[ -S "$ROONSCAPE_SOCKET" ] || exit 3\ncp "$ROONSCAPE_CAPTURE_TEST_PNG" "$4"\n',
  );

  try {
    const { stderr } = await execFileAsync(
      process.execPath,
      [
        "scripts/capture-presentations.mjs",
        "--output",
        outputDirectory,
        "--only",
        "playing",
        "--viewport",
        "1280x720",
        "--settle-ms",
        "0",
      ],
      {
        cwd: new URL("..", import.meta.url),
        env: {
          ...process.env,
          PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
          ROONSCAPE_CAPTURE_TEST_PNG: fakePng,
          ROONSCAPE_CAPTURE_TEST_DISPLAY_CONFIGURATION: displayConfiguration,
          ROONSCAPE_CAPTURE_TEST_RENDERER_ARGUMENTS: rendererArguments,
          ROONSCAPE_CAPTURE_TEST_RENDERER_ENVIRONMENT: rendererEnvironment,
          ROONSCAPE_CAPTURE_TEST_WINDOW_INSPECTION_ATTEMPTS:
            windowInspectionAttempts,
          ROONSCAPE_CAPTURE_TEST_WINDOW_ID: "4242",
        },
      },
    );
    const manifest = JSON.parse(
      await readFile(path.join(outputDirectory, "manifest.json"), "utf8"),
    );

    assert.equal(stderr, "");
    assert.deepEqual(manifest.captures, [
      {
        ...buildPresentationCapturePlan().find(
          (capture) =>
            capture.scenario === "playing" &&
            capture.viewport === "1280x720" &&
            capture.variant === "matrix",
        ),
        renderer: "native GTK 4/Pango",
      },
    ]);
    assert.deepEqual(
      await readFile(path.join(outputDirectory, manifest.captures[0].fileName)),
      await readFile(fakePng),
    );
    assert.equal(
      await readFile(rendererEnvironment, "utf8"),
      "1280x720||0|unset|missing\n",
      "the renderer should be ready before the fixture progress clock starts",
    );
    assert.equal(
      await readFile(windowInspectionAttempts, "utf8"),
      "2\n",
      "capture readiness should wait for the native window to reach its requested size",
    );
    const rendererArgumentList = (await readFile(rendererArguments, "utf8"))
      .trimEnd()
      .split("\n");
    assert.deepEqual(rendererArgumentList.slice(0, 6), [
      "run",
      "--quiet",
      "--package",
      "roonscape-renderer",
      "--",
      "--config",
    ]);
    assert.equal(path.basename(rendererArgumentList[6]), "display.json");
    assert.deepEqual(JSON.parse(await readFile(displayConfiguration, "utf8")), {
      trackedOutputId: "visual-acceptance-capture",
      inactivity: {
        gracePeriodSeconds: 3600,
        dimmedOpacity: 0.35,
        repositionCadenceSeconds: 60,
      },
    });
  } finally {
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

async function installFakeCaptureDisplay(
  binDirectory,
  { logStop = false } = {},
) {
  const stopAction = logStop
    ? 'printf "Xvfb-stopped\\n" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"; rm -f "$socket"'
    : 'rm -f "$socket"';
  await executable(
    path.join(binDirectory, "Xvfb"),
    [
      "#!/bin/sh",
      'printf "Xvfb|%s\\n" "$*" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"',
      'display_number="${1#:}"',
      'socket="/tmp/.X11-unix/X${display_number}"',
      "mkdir -p /tmp/.X11-unix",
      'touch "$socket"',
      `trap '${stopAction}; exit 0' TERM INT EXIT`,
      "while :; do /usr/bin/sleep 1; done",
      "",
    ].join("\n"),
  );
  await executable(
    path.join(binDirectory, "xwininfo"),
    [
      "#!/bin/sh",
      'width="${ROONSCAPE_CAPTURE_VIEWPORT%x*}"',
      'height="${ROONSCAPE_CAPTURE_VIEWPORT#*x}"',
      `printf 'xwininfo: Window id: 4242 "RoonScape"\\n  Width: %s\\n  Height: %s\\n' "$width" "$height"`,
      "",
    ].join("\n"),
  );
}

async function executable(filePath, contents) {
  await writeFile(filePath, contents);
  await chmod(filePath, 0o755);
}

function pngHeader(width, height) {
  const header = Buffer.alloc(33);
  Buffer.from("89504e470d0a1a0a", "hex").copy(header);
  header.writeUInt32BE(13, 8);
  header.write("IHDR", 12, "ascii");
  header.writeUInt32BE(width, 16);
  header.writeUInt32BE(height, 20);
  header[24] = 8;
  header[25] = 2;
  return header;
}
