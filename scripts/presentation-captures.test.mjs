import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import { buildPresentationCapturePlan } from "./presentation-captures.mjs";

const execFileAsync = promisify(execFile);
const scratchRoot = "/tmp/codex/roonscape";

const REPRESENTATIVE_VIEWPORTS = [
  "1280x720",
  "1600x1200",
  "1920x1200",
  "2560x1080",
  "3840x2160",
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
      REPRESENTATIVE_VIEWPORTS,
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
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
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

test("plans explicit typography and adaptive diagnostics representatives", () => {
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
    "both complete font pairs should visibly exercise Pango glyph fallback",
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
        .filter((capture) => capture.viewport === viewport)
        .map((capture) => capture.scenario),
      [
        "preferred-typography",
        "fallback-typography",
        "identity-baselines",
        "dark-diagnostics",
        "light-diagnostics",
        "fixed-no-art-diagnostics",
      ],
      `${viewport} should receive the same typography and diagnostics review`,
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

test("capture command orchestrates one native fixture capture and records its manifest", async () => {
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const binDirectory = path.join(taskDirectory, "bin");
  const outputDirectory = path.join(taskDirectory, "captures");
  const fakePng = path.join(taskDirectory, "fake.png");
  const rendererEnvironment = path.join(taskDirectory, "renderer-environment");
  const rendererArguments = path.join(taskDirectory, "renderer-arguments");
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
    '#!/bin/sh\nconfiguration_file="$7"\nprintf "%s|%s|%s|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" "$ROONSCAPE_CAPTURE_TYPOGRAPHY" "$ROONSCAPE_DIAGNOSTICS" "${ROONSCAPE_DISPLAY_CONFIG-unset}" > "$ROONSCAPE_CAPTURE_TEST_RENDERER_ENVIRONMENT"\nprintf "%s\\n" "$@" > "$ROONSCAPE_CAPTURE_TEST_RENDERER_ARGUMENTS"\ncp "$configuration_file" "$ROONSCAPE_CAPTURE_TEST_DISPLAY_CONFIGURATION"\ntrap \'exit 0\' TERM INT\nwhile :; do /usr/bin/sleep 1; done\n',
  );
  await executable(
    path.join(binDirectory, "xwininfo"),
    "#!/bin/sh\nprintf 'xwininfo: Window id: 4242 \"RoonScape\"\\n  Width: 1280\\n  Height: 720\\n'\n",
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\n[ "$1" = "--window" ] && [ "$2" = "$ROONSCAPE_CAPTURE_TEST_WINDOW_ID" ] && [ "$3" = "--overwrite" ] || exit 2\ncp "$ROONSCAPE_CAPTURE_TEST_PNG" "$4"\n',
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
      "1280x720||0|unset\n",
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
