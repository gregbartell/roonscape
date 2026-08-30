import assert from "node:assert/strict";
import { execFile } from "node:child_process";
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

test("capture command lists the durable plan without launching the renderer", async () => {
  const { stdout, stderr } = await execFileAsync(
    process.execPath,
    ["scripts/capture-presentations.mjs", "--list"],
    { cwd: new URL("..", import.meta.url) },
  );
  const plan = JSON.parse(stdout);

  assert.equal(stderr, "");
  assert.deepEqual(plan, buildPresentationCapturePlan());
});

test("focused capture selection rejects an ambiguous exact Fixture Scenario", () => {
  const plan = buildPresentationCapturePlan();
  const playing = selectFocusedPresentationCapture(plan, "playing");

  assert.throws(
    () => selectFocusedPresentationCapture([...plan, playing], "playing"),
    /ambiguous Fixture Scenario identifier: playing/,
  );
});

test("focused capture waits for its painted revision and publishes one validated 4K PNG", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-focused-capture-test."),
  );
  const binDirectory = path.join(taskDirectory, "bin");
  const workDirectory = path.join(taskDirectory, "work");
  const runtimeRoot = await mkdtemp(path.join(tmpdir(), "rsc."));
  const fakePng = path.join(taskDirectory, "fake.png");
  const processLog = path.join(taskDirectory, "processes");
  await Promise.all([mkdir(binDirectory), mkdir(workDirectory)]);
  await writeFile(fakePng, pngHeader(3840, 2160));
  await executable(
    path.join(binDirectory, "Xvfb"),
    '#!/bin/sh\nprintf "Xvfb|%s\\n" "$*" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\ndisplay_number="${1#:}"\nsocket="/tmp/.X11-unix/X${display_number}"\nmkdir -p /tmp/.X11-unix\ntouch "$socket"\ntrap \'printf "Xvfb-stopped\\n" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"; rm -f "$socket"; exit 0\' TERM INT EXIT\nwhile :; do /usr/bin/sleep 1; done\n',
  );
  await executable(
    path.join(binDirectory, "cargo"),
    '#!/bin/sh\nprintf "renderer|%s|%s|%s|%s|%s|%s|%s\\n" "$ROONSCAPE_STATIC_FIXTURE" "$ROONSCAPE_CAPTURE_VIEWPORT" "$ROONSCAPE_CAPTURE_CONTROL" "${ROONSCAPE_DIAGNOSTICS-unset}" "${ROONSCAPE_CAPTURE_TYPOGRAPHY-unset}" "${ROONSCAPE_FIXTURE_AUTO_CLOSE_MS-unset}" "${ROONSCAPE_DISPLAY_CONFIG-unset}" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\nexec "${NODE:-node}" "$ROONSCAPE_CAPTURE_TEST_RENDERER"\n',
  );
  await executable(
    path.join(binDirectory, "xwininfo"),
    "#!/bin/sh\nprintf 'xwininfo: Window id: 4242 \"RoonScape\"\\n  Width: 3840\\n  Height: 2160\\n'\n",
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\nprintf "scrot|%s|%s\\n" "$*" "$(/usr/bin/date +%s%3N)" >> "$ROONSCAPE_CAPTURE_TEST_PROCESS_LOG"\n[ "$1" = "--window" ] && [ "$2" = "4242" ] && [ "$3" = "--overwrite" ] || exit 2\ncp "$ROONSCAPE_CAPTURE_TEST_PNG" "$4"\n',
  );
  const fakeRenderer = path.join(taskDirectory, "renderer.mjs");
  await writeFile(
    fakeRenderer,
    '#!/usr/bin/env node\nimport { once } from "node:events";\nimport { appendFile } from "node:fs/promises";\nimport { createConnection } from "node:net";\nimport { createInterface } from "node:readline";\nconst log = process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG;\nconst connection = createConnection(process.env.ROONSCAPE_CAPTURE_CONTROL);\nprocess.once("SIGTERM", async () => { await appendFile(log, "renderer-stopped\\n"); process.exit(0); });\nawait once(connection, "connect");\nconst lines = createInterface({ input: connection })[Symbol.asyncIterator]();\nconst selection = JSON.parse((await lines.next()).value);\nif (selection.type !== "select" || selection.scenario !== "playing" || selection.revision !== selection.snapshot.revision) process.exit(2);\nawait appendFile(log, `painted|${Date.now()}\\n`);\nconnection.write(`${JSON.stringify({ type: "painted", scenario: selection.scenario, revision: selection.revision })}\\n`);\nawait once(connection, "close");\nawait appendFile(log, "renderer-stopped\\n");\n',
  );

  const runFocusedCapture = () =>
    execFileAsync(
      process.execPath,
      [
        path.join(
          new URL("..", import.meta.url).pathname,
          "scripts/capture-presentations.mjs",
        ),
        "--scenario",
        "playing",
      ],
      {
        cwd: workDirectory,
        env: {
          ...process.env,
          PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
          TMPDIR: runtimeRoot,
          ROONSCAPE_CAPTURE_TEST_PNG: fakePng,
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
    assert.deepEqual(await readFile(finalPath), await readFile(fakePng));
    const processes = await readFile(processLog, "utf8");
    assert.match(processes, /Xvfb\|.*3840x2160x24/);
    assert.match(
      processes,
      /renderer\|1\|3840x2160\|\S+\|0\|unset\|unset\|unset/,
    );
    assert.match(processes, /scrot\|--window 4242 --overwrite \/.*\.png\|\d+/);
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
    await writeFile(fakePng, pngHeader(1920, 1080));
    await assert.rejects(
      runFocusedCapture(),
      /is 1920x1080; expected 3840x2160/,
    );
    assert.deepEqual(await readdir(workDirectory), []);
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
