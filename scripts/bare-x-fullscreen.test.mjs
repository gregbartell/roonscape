import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  runMonitoredProcess,
  startXvfbDisplay,
  startMonitoredProcess,
  stopProcess,
  waitFor,
} from "./process-harness.mjs";

const executeFile = promisify(execFile);
const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const rendererPath = path.join(
  repositoryRoot,
  "target/debug/roonscape-renderer",
);
const windowedDisconnectedAccent = {
  left: 1440,
  top: 306,
  width: 6,
  height: 246,
};
const fullscreenDisconnectedAccent = {
  left: 768,
  top: 806,
  width: 15,
  height: 487,
};

test("determinate progress renders without invalid GTK measurements", async () => {
  await access(rendererPath);
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bare-x-test."),
  );
  const socketPath = path.join(taskDirectory, "roonscape.sock");
  const environment = {
    ...process.env,
    GDK_BACKEND: "x11",
    NO_AT_BRIDGE: "1",
    ROONSCAPE_CAPTURE_VIEWPORT: "1600x900",
    ROONSCAPE_FIXTURE: "src/shared/fixtures/playing.json",
    ROONSCAPE_FIXTURE_AUTO_CLOSE_MS: "2000",
    ROONSCAPE_SOCKET: socketPath,
  };
  const { display, xvfb } = await startXvfbDisplay({
    width: 1600,
    height: 900,
    cwd: repositoryRoot,
  });
  environment.DISPLAY = display;
  let publisher;
  let renderer;

  try {
    publisher = monitoredProcess(
      process.execPath,
      ["src/bridge/dist/src/fixture.js"],
      environment,
    );
    await publisher.spawned;
    await waitFor(
      () => access(socketPath),
      publisher,
      "fixture publisher socket",
    );

    renderer = monitoredProcess(
      rendererPath,
      ["--config", path.join(taskDirectory, "display.json")],
      environment,
    );
    const closed = once(renderer, "close");
    await renderer.spawned;
    const [exitCode, signal] = await closed;

    assert.equal(signal, null);
    assert.equal(exitCode, 0);
    assert.doesNotMatch(
      renderer.capturedStandardError,
      /reported min width -2/,
    );
  } finally {
    await stopProcess(renderer);
    await stopProcess(publisher);
    await stopProcess(xvfb);
    await rm(taskDirectory, { recursive: true });
  }
});

test("Live Mode fills its sole display without a window manager", async () => {
  await assertRendererGeometry({}, "display");
});

test("fullscreen startup reveals Disconnected only after its layout settles", async () => {
  await withBareXRenderer(
    {
      setup: async ({ taskDirectory }) => {
        const firstRevealedPaint =
          await startFirstRevealedPaintControl(taskDirectory);
        return {
          environment: {
            ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL:
              firstRevealedPaint.path,
          },
          context: { firstRevealedPaint },
          beforeRendererStop: () => firstRevealedPaint.release(),
          afterRendererStop: () => firstRevealedPaint.close(),
        };
      },
    },
    async ({ taskDirectory, environment, renderer, firstRevealedPaint }) => {
      const firstRevealedPaintControl = await waitFor(
        () => firstRevealedPaint.connection,
        renderer,
        "the renderer first-revealed-paint control connection",
      );
      let firstRevealedPaintError;
      let firstRevealedPaintNotification = "";
      firstRevealedPaintControl.setEncoding("utf8");
      firstRevealedPaintControl.on("error", (error) => {
        firstRevealedPaintError = error;
      });
      firstRevealedPaintControl.on("data", (chunk) => {
        firstRevealedPaintNotification += chunk;
      });
      await waitFor(
        () => {
          if (firstRevealedPaintError !== undefined) {
            throw firstRevealedPaintError;
          }
          if (!firstRevealedPaintNotification.includes("painted\n")) {
            throw new Error("the first revealed frame has not been reported");
          }
        },
        renderer,
        "the renderer first revealed frame",
      );
      const firstFrame = await captureFullscreenFrame(
        path.join(taskDirectory, "first-revealed.ppm"),
        environment,
        "fullscreen first revealed frame",
      );
      assert.equal(
        disconnectedLayout(firstFrame),
        "full-display",
        "the first revealed frame did not use the fullscreen Disconnected layout",
      );
      firstRevealedPaintControl.write("release\n");
      await waitFor(
        () => {
          if (firstRevealedPaintError !== undefined) {
            throw firstRevealedPaintError;
          }
          if (!firstRevealedPaintNotification.includes("repainted\n")) {
            throw new Error("the post-release frame has not been reported");
          }
        },
        renderer,
        "the renderer post-release painted frame",
      );
      const settledFrame = await captureFullscreenFrame(
        path.join(taskDirectory, "settled.ppm"),
        environment,
        "settled fullscreen frame",
      );
      assert.equal(
        firstFrame.compare(settledFrame),
        0,
        "the first revealed Disconnected frame changed after GTK settled",
      );
    },
  );
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
  await withBareXRenderer(
    { environment },
    async ({ display, renderer, xvfb }) => {
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
    },
  );
}

async function withBareXRenderer(
  { environment: environmentOverrides = {}, setup } = {},
  inspect,
) {
  await access(rendererPath);
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bare-x-test."),
  );
  const { display, xvfb } = await startXvfbDisplay({
    width: 3840,
    height: 2160,
    cwd: repositoryRoot,
  });
  let session;
  let renderer;

  try {
    session = await setup?.({ taskDirectory });
    const environment = {
      ...process.env,
      DISPLAY: display,
      GDK_BACKEND: "x11",
      NO_AT_BRIDGE: "1",
      ROONSCAPE_FIXTURE_AUTO_CLOSE_MS: "10000",
      ROONSCAPE_SOCKET: path.join(taskDirectory, "missing.sock"),
      ...environmentOverrides,
      ...session?.environment,
    };
    renderer = monitoredProcess(
      rendererPath,
      ["--config", path.join(taskDirectory, "display.json")],
      environment,
    );
    await renderer.spawned;
    await inspect({
      display,
      environment,
      renderer,
      taskDirectory,
      xvfb,
      ...session?.context,
    });
  } finally {
    await session?.beforeRendererStop?.();
    await stopProcess(renderer);
    await session?.afterRendererStop?.();
    await stopProcess(xvfb);
    await rm(taskDirectory, { recursive: true });
  }
}

async function startFirstRevealedPaintControl(taskDirectory) {
  const server = createServer();
  const controlPath = path.join(taskDirectory, "first-paint-control.sock");
  server.listen(controlPath);
  await once(server, "listening");
  let control;
  const connection = once(server, "connection").then(([connected]) => {
    control = connected;
    return connected;
  });
  return {
    path: controlPath,
    connection,
    release() {
      control?.end("release\n");
    },
    async close() {
      control?.destroy();
      if (server.listening) {
        await new Promise((resolve) => server.close(resolve));
      }
    },
  };
}

async function captureFullscreenFrame(outputPath, environment, description) {
  await runMonitoredProcess(
    "scrot",
    ["--autoselect", "0,0,3840,2160", "--overwrite", outputPath],
    { cwd: repositoryRoot, environment, description },
  );
  return readFile(outputPath);
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

function disconnectedLayout(ppm) {
  const image = parsePpm(ppm);
  assert.equal(image.width, 3840);
  assert.equal(image.height, 2160);
  if (accentCoverage(image, windowedDisconnectedAccent) > 0.9) {
    return "windowed";
  }
  if (accentCoverage(image, fullscreenDisconnectedAccent) > 0.9) {
    return "full-display";
  }
  return "absent";
}

function accentCoverage(image, rectangle) {
  let accentPixels = 0;
  for (let y = rectangle.top; y < rectangle.top + rectangle.height; y += 1) {
    for (let x = rectangle.left; x < rectangle.left + rectangle.width; x += 1) {
      const offset = image.pixelOffset + (y * image.width + x) * 3;
      const red = image.data[offset];
      const green = image.data[offset + 1];
      const blue = image.data[offset + 2];
      if (
        red >= 245 &&
        green >= 90 &&
        green <= 135 &&
        blue >= 65 &&
        blue <= 110
      ) {
        accentPixels += 1;
      }
    }
  }
  return accentPixels / (rectangle.width * rectangle.height);
}

function parsePpm(data) {
  let offset = 0;
  const tokens = [];
  while (tokens.length < 4) {
    while (
      offset < data.length &&
      /\s/.test(String.fromCharCode(data[offset]))
    ) {
      offset += 1;
    }
    if (data[offset] === 0x23) {
      while (offset < data.length && data[offset] !== 0x0a) {
        offset += 1;
      }
      continue;
    }
    const start = offset;
    while (
      offset < data.length &&
      !/\s/.test(String.fromCharCode(data[offset]))
    ) {
      offset += 1;
    }
    tokens.push(data.toString("ascii", start, offset));
  }
  const [format, width, height, maximum] = tokens;
  assert.equal(format, "P6");
  assert.equal(maximum, "255");
  assert.match(String.fromCharCode(data[offset]), /\s/);
  return {
    data,
    width: Number.parseInt(width, 10),
    height: Number.parseInt(height, 10),
    pixelOffset: offset + 1,
  };
}
