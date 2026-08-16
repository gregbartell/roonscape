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

import { buildGalleryCapturePlan } from "./gallery-captures.mjs";

const execFileAsync = promisify(execFile);
const scratchRoot = "/tmp/codex/roonscape";

const REQUIRED_VIEWPORTS = ["1600x900", "3840x2160", "3840x2400"];
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

test("plans every visual acceptance scenario at every supported viewport", () => {
  const plan = buildGalleryCapturePlan();

  for (const scenario of REQUIRED_SCENARIOS) {
    const captures = plan.filter(
      (capture) =>
        capture.scenario === scenario && capture.variant === "matrix",
    );
    assert.deepEqual(
      captures.map((capture) => capture.viewport).sort(),
      REQUIRED_VIEWPORTS,
      `${scenario} should be captured at every supported viewport`,
    );
  }

  assert.equal(
    new Set(plan.map((capture) => capture.fileName)).size,
    plan.length,
    "capture artifact names should be unique",
  );
});

test("plans explicit typography and adaptive diagnostics representatives", () => {
  const representatives = buildGalleryCapturePlan().filter(
    (capture) => capture.variant === "representative",
  );

  assert.deepEqual(
    representatives
      .filter((capture) => capture.typography !== "automatic")
      .map((capture) => capture.typography)
      .sort(),
    ["fallback", "preferred"],
  );
  assert.deepEqual(
    representatives
      .filter((capture) => capture.diagnostics)
      .map((capture) => capture.palette)
      .sort(),
    ["dark", "fixed-no-art", "light"],
  );
  assert.ok(
    representatives.every((capture) => capture.viewport === "3840x2160"),
    "representative captures should use the Reference Deployment viewport",
  );
});

test("capture command lists the durable plan without launching the renderer", async () => {
  const { stdout, stderr } = await execFileAsync(
    process.execPath,
    ["scripts/capture-gallery.mjs", "--list"],
    { cwd: new URL("..", import.meta.url) },
  );
  const plan = JSON.parse(stdout);

  assert.equal(stderr, "");
  assert.deepEqual(plan, buildGalleryCapturePlan());
});

test("capture command orchestrates one native fixture capture and records its manifest", async () => {
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const binDirectory = path.join(taskDirectory, "bin");
  const outputDirectory = path.join(taskDirectory, "captures");
  const fakePng = path.join(taskDirectory, "fake.png");
  const rendererEnvironment = path.join(taskDirectory, "renderer-environment");
  await mkdir(binDirectory);
  await writeFile(fakePng, pngHeader(1600, 900));
  await executable(
    path.join(binDirectory, "Xvfb"),
    '#!/bin/sh\ndisplay_number="${1#:}"\nsocket="/tmp/.X11-unix/X${display_number}"\nmkdir -p /tmp/.X11-unix\ntouch "$socket"\ntrap \'rm -f "$socket"; exit 0\' TERM INT EXIT\nwhile :; do /usr/bin/sleep 1; done\n',
  );
  await executable(
    path.join(binDirectory, "cargo"),
    '#!/bin/sh\nprintf "%s|%s|%s\\n" "$ROONSCAPE_CAPTURE_VIEWPORT" "$ROONSCAPE_CAPTURE_TYPOGRAPHY" "$ROONSCAPE_DIAGNOSTICS" > "$ROONSCAPE_CAPTURE_TEST_RENDERER_ENVIRONMENT"\ntrap \'exit 0\' TERM INT\nwhile :; do /usr/bin/sleep 1; done\n',
  );
  await executable(
    path.join(binDirectory, "xwininfo"),
    "#!/bin/sh\nprintf 'xwininfo: Window id: 4242 \"RoonScape\"\\n  Width: 1600\\n  Height: 900\\n'\n",
  );
  await executable(
    path.join(binDirectory, "scrot"),
    '#!/bin/sh\n[ "$1" = "--window" ] && [ "$2" = "$ROONSCAPE_CAPTURE_TEST_WINDOW_ID" ] && [ "$3" = "--overwrite" ] || exit 2\ncp "$ROONSCAPE_CAPTURE_TEST_PNG" "$4"\n',
  );

  try {
    const { stderr } = await execFileAsync(
      process.execPath,
      [
        "scripts/capture-gallery.mjs",
        "--output",
        outputDirectory,
        "--only",
        "playing",
        "--viewport",
        "1600x900",
        "--settle-ms",
        "0",
      ],
      {
        cwd: new URL("..", import.meta.url),
        env: {
          ...process.env,
          PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
          ROONSCAPE_CAPTURE_TEST_PNG: fakePng,
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
        ...buildGalleryCapturePlan().find(
          (capture) =>
            capture.scenario === "playing" &&
            capture.viewport === "1600x900" &&
            capture.variant === "matrix",
        ),
        renderer: "native GTK 4/Pango",
      },
    ]);
    assert.deepEqual(
      await readFile(path.join(outputDirectory, manifest.captures[0].fileName)),
      await readFile(fakePng),
    );
    assert.equal(await readFile(rendererEnvironment, "utf8"), "1600x900||0\n");
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
