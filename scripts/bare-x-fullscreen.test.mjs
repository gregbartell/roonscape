import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { access, mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  startXvfbDisplay,
  startMonitoredProcess,
  stopProcess,
  waitFor,
} from "./process-harness.mjs";

const executeFile = promisify(execFile);
const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const scratchRoot = "/var/tmp/codex/roonscape";
const rendererPath = path.join(
  repositoryRoot,
  "target/debug/roonscape-renderer",
);

test("Live Mode fills its sole display without a window manager", async () => {
  await assertRendererGeometry({}, "display");
});

test("explicit windowed operation retains the windowed viewport", async () => {
  await assertRendererGeometry(
    { ROONSCAPE_WINDOWED: "1" },
    {
      x: 0,
      y: 0,
      width: 1600,
      height: 900,
    },
  );
});

test("Presentation Capture retains its requested viewport", async () => {
  await assertRendererGeometry(
    { ROONSCAPE_CAPTURE_VIEWPORT: "1280x720" },
    {
      x: 0,
      y: 0,
      width: 1280,
      height: 720,
    },
  );
});

async function assertRendererGeometry(environment, expected) {
  await access(rendererPath);
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const { display, xvfb } = await startXvfbDisplay({
    width: 3840,
    height: 2160,
    cwd: repositoryRoot,
  });
  let renderer;

  try {
    renderer = monitoredProcess(
      rendererPath,
      ["--config", path.join(taskDirectory, "display.json")],
      {
        DISPLAY: display,
        GDK_BACKEND: "x11",
        ROONSCAPE_FIXTURE_AUTO_CLOSE_MS: "10000",
        ROONSCAPE_SOCKET: path.join(taskDirectory, "missing.sock"),
        ...environment,
      },
    );
    await renderer.spawned;

    const root = await waitFor(
      () => xGeometry(display, ["-root"]),
      xvfb,
      "X root window",
    );
    const window = await waitFor(
      async () => {
        const geometry = await xGeometry(display, ["-name", "RoonScape"]);
        if (geometry.width < 1280 || geometry.height < 720) {
          throw new Error("RoonScape window has not reached a usable size");
        }
        return geometry;
      },
      renderer,
      "RoonScape window",
    );

    const expectedGeometry =
      expected === "display"
        ? { x: 0, y: 0, width: root.width, height: root.height }
        : expected;
    assert.deepEqual(window, expectedGeometry);
  } finally {
    await stopProcess(renderer);
    await stopProcess(xvfb);
    await rm(taskDirectory, { recursive: true });
  }
}

function monitoredProcess(command, arguments_, environment = {}) {
  return startMonitoredProcess(command, arguments_, {
    cwd: repositoryRoot,
    environment: { ...process.env, ...environment },
  });
}

async function xGeometry(display, arguments_) {
  const { stdout } = await executeFile("xwininfo", arguments_, {
    env: { ...process.env, DISPLAY: display },
    timeout: 1_000,
  });
  return {
    x: integerField(stdout, "Absolute upper-left X"),
    y: integerField(stdout, "Absolute upper-left Y"),
    width: integerField(stdout, "Width"),
    height: integerField(stdout, "Height"),
  };
}

function integerField(output, name) {
  const match = output.match(new RegExp(`^  ${name}:\\s+(-?\\d+)$`, "m"));
  assert.notEqual(match, null, `xwininfo did not report ${name}`);
  return Number.parseInt(match[1], 10);
}
