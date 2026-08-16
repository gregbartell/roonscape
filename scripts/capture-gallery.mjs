import { spawn } from "node:child_process";
import { once } from "node:events";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { buildGalleryCapturePlan } from "./gallery-captures.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const scratchRoot = "/tmp/codex/roonscape";
const nativeRenderer = "native GTK 4/Pango";
const captureDisplayConfiguration = {
  trackedOutputId: "visual-acceptance-capture",
  inactivity: {
    gracePeriodSeconds: 3600,
    dimmedOpacity: 0.35,
    repositionCadenceSeconds: 60,
  },
};
const options = parseArguments(process.argv.slice(2));

if (options.list) {
  process.stdout.write(
    `${JSON.stringify(buildGalleryCapturePlan(), null, 2)}\n`,
  );
} else {
  await captureGallery(options);
}

async function captureGallery({ output, only, viewport, settleMilliseconds }) {
  const plan = buildGalleryCapturePlan().filter(
    (capture) =>
      (only === undefined || capture.scenario === only) &&
      (viewport === undefined || capture.viewport === viewport),
  );
  if (plan.length === 0) {
    throw new Error("the capture filters did not select any planned artifacts");
  }

  const outputDirectory = await prepareOutputDirectory(output);
  const viewportGroups = Map.groupBy(plan, (capture) => capture.viewport);
  let completed = 0;

  for (const captures of viewportGroups.values()) {
    const [firstCapture] = captures;
    const displayNumber = await availableDisplayNumber();
    const display = `:${displayNumber}`;
    const displaySocket = `/tmp/.X11-unix/X${displayNumber}`;
    const xvfb = startLongRunning("Xvfb", [
      display,
      "-screen",
      "0",
      `${firstCapture.width}x${firstCapture.height}x24`,
      "-nolisten",
      "tcp",
    ]);

    try {
      await waitForPath(displaySocket, xvfb, "the native capture display");
      for (const capture of captures) {
        completed += 1;
        process.stdout.write(
          `[${completed}/${plan.length}] ${capture.fileName}\n`,
        );
        await captureFixture(
          capture,
          display,
          outputDirectory,
          settleMilliseconds,
        );
      }
    } finally {
      await stop(xvfb);
    }
  }

  const manifest = {
    formatVersion: 1,
    renderer: nativeRenderer,
    captures: plan.map((capture) => ({
      ...capture,
      renderer: nativeRenderer,
    })),
  };
  await writeFile(
    path.join(outputDirectory, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  process.stdout.write(`Gallery captures written to ${outputDirectory}\n`);
}

async function captureFixture(
  capture,
  display,
  outputDirectory,
  settleMilliseconds,
) {
  const runtimeDirectory = await mkdtemp(
    path.join(scratchRoot, "gallery-capture."),
  );
  const socketPath = path.join(runtimeDirectory, "roonscape.sock");
  const displayConfigurationPath = path.join(runtimeDirectory, "display.json");
  await writeFile(
    displayConfigurationPath,
    `${JSON.stringify(captureDisplayConfiguration, null, 2)}\n`,
  );
  const environment = {
    ...process.env,
    DISPLAY: display,
    GDK_BACKEND: "x11",
    NO_AT_BRIDGE: "1",
    ROONSCAPE_CAPTURE_VIEWPORT: capture.viewport,
    ROONSCAPE_DIAGNOSTICS: capture.diagnostics ? "1" : "0",
    ROONSCAPE_DISPLAY_CONFIG: displayConfigurationPath,
    ROONSCAPE_FIXTURE: capture.fixture,
    ROONSCAPE_SOCKET: socketPath,
  };
  delete environment.ROONSCAPE_FIXTURE_CONTROL;
  if (capture.typography !== "automatic") {
    environment.ROONSCAPE_CAPTURE_TYPOGRAPHY = capture.typography;
  } else {
    delete environment.ROONSCAPE_CAPTURE_TYPOGRAPHY;
  }

  let publisher;
  let renderer;
  try {
    renderer = startLongRunning(
      "cargo",
      ["run", "--quiet", "--package", "roonscape-renderer"],
      environment,
    );
    await renderer.spawned;
    const windowId = await waitForRoonScapeWindow(
      renderer,
      environment,
      capture.width,
      capture.height,
    );
    publisher = startLongRunning(
      process.execPath,
      [path.join(repositoryRoot, "bridge/dist/src/fixture.js")],
      environment,
    );
    await waitForPath(socketPath, publisher, "the fixture publisher");
    await delay(settleMilliseconds);
    assertRunning(renderer, "the native renderer");

    const capturePath = path.join(outputDirectory, capture.fileName);
    await run(
      "scrot",
      ["--window", windowId, "--overwrite", capturePath],
      environment,
    );
    await verifyPngDimensions(capturePath, capture.width, capture.height);
  } finally {
    await Promise.all([stop(renderer), stop(publisher)]);
    await rm(runtimeDirectory, { force: true, recursive: true });
  }
}

function startLongRunning(command, arguments_, environment = process.env) {
  const child = spawn(command, arguments_, {
    cwd: repositoryRoot,
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.capturedError = undefined;
  child.capturedStandardOutput = "";
  child.capturedStandardError = "";
  child.spawned = new Promise((resolve, reject) => {
    child.once("spawn", resolve);
    child.once("error", reject);
  });
  child.on("error", (error) => {
    child.capturedError = error;
  });
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    child.capturedStandardOutput += chunk;
  });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    child.capturedStandardError += chunk;
  });
  return child;
}

async function run(command, arguments_, environment) {
  const child = startLongRunning(command, arguments_, environment);
  await child.spawned;
  const [exitCode, signal] = await once(child, "close");
  if (exitCode !== 0) {
    throw childFailure(command, child, exitCode, signal);
  }
  return child.capturedStandardOutput;
}

async function stop(child) {
  if (
    child === undefined ||
    child.exitCode !== null ||
    child.signalCode !== null ||
    child.capturedError !== undefined
  ) {
    return;
  }

  const closed = once(child, "close");
  child.kill("SIGTERM");
  const stopped = await Promise.race([
    closed.then(() => true),
    delay(2_000).then(() => false),
  ]);
  if (!stopped) {
    if (child.exitCode !== null || child.signalCode !== null) {
      return;
    }
    const killed = once(child, "close");
    child.kill("SIGKILL");
    await killed;
  }
}

async function waitForRoonScapeWindow(
  renderer,
  environment,
  expectedWidth,
  expectedHeight,
) {
  for (let attempt = 0; attempt < 300; attempt += 1) {
    assertRunning(renderer, "the native renderer");
    try {
      const output = await run(
        "xwininfo",
        ["-name", "RoonScape", "-int"],
        environment,
      );
      const window = parseWindowInformation(output);
      if (window.width !== expectedWidth || window.height !== expectedHeight) {
        throw new Error(
          `native RoonScape window is ${window.width}x${window.height}; expected ${expectedWidth}x${expectedHeight}`,
        );
      }
      return window.id;
    } catch (error) {
      if (error?.code === "ENOENT") {
        throw new Error("xwininfo is required for native capture readiness", {
          cause: error,
        });
      }
      if (
        error instanceof Error &&
        error.message.startsWith("native RoonScape window is ")
      ) {
        throw error;
      }
    }
    await delay(100);
  }

  throw new Error("timed out waiting for the native RoonScape window");
}

function parseWindowInformation(output) {
  const id = output.match(/Window id:\s+(\d+)/)?.[1];
  const width = Number.parseInt(
    output.match(/^\s*Width:\s+(\d+)/m)?.[1] ?? "",
    10,
  );
  const height = Number.parseInt(
    output.match(/^\s*Height:\s+(\d+)/m)?.[1] ?? "",
    10,
  );
  if (
    id === undefined ||
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height)
  ) {
    throw new Error("xwininfo did not report a usable RoonScape window");
  }

  return { id, width, height };
}

async function waitForPath(filePath, child, description) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    assertRunning(child, description);
    try {
      await access(filePath);
      return;
    } catch (error) {
      if (error?.code !== "ENOENT") {
        throw error;
      }
    }
    await delay(25);
  }

  throw new Error(`timed out waiting for ${description}`);
}

function assertRunning(child, description) {
  if (child.capturedError !== undefined) {
    throw child.capturedError;
  }
  if (child.exitCode !== null || child.signalCode !== null) {
    throw childFailure(description, child, child.exitCode, child.signalCode);
  }
}

function childFailure(description, child, exitCode, signal) {
  const outcome = signal ?? exitCode ?? "unknown status";
  const details = child.capturedStandardError.trim();
  return new Error(
    `${description} exited with ${outcome}${details.length === 0 ? "" : `: ${details}`}`,
  );
}

async function verifyPngDimensions(filePath, expectedWidth, expectedHeight) {
  const header = await readFile(filePath);
  const pngSignature = "89504e470d0a1a0a";
  if (
    header.length < 24 ||
    header.subarray(0, 8).toString("hex") !== pngSignature ||
    header.subarray(12, 16).toString("ascii") !== "IHDR"
  ) {
    throw new Error(`${filePath} is not a PNG capture`);
  }

  const width = header.readUInt32BE(16);
  const height = header.readUInt32BE(20);
  if (width !== expectedWidth || height !== expectedHeight) {
    throw new Error(
      `${filePath} is ${width}x${height}; expected ${expectedWidth}x${expectedHeight}`,
    );
  }
}

async function prepareOutputDirectory(requestedOutput) {
  await mkdir(scratchRoot, { recursive: true });
  if (requestedOutput === undefined) {
    return mkdtemp(path.join(scratchRoot, "gallery-captures."));
  }

  const outputDirectory = path.resolve(repositoryRoot, requestedOutput);
  try {
    await mkdir(outputDirectory);
  } catch (error) {
    if (error?.code !== "EEXIST") {
      throw error;
    }
    const entries = await readdir(outputDirectory);
    if (entries.length !== 0) {
      throw new Error(
        `capture output directory is not empty: ${outputDirectory}`,
        { cause: error },
      );
    }
  }
  return outputDirectory;
}

async function availableDisplayNumber() {
  for (let displayNumber = 90; displayNumber < 200; displayNumber += 1) {
    const socket = `/tmp/.X11-unix/X${displayNumber}`;
    const lock = `/tmp/.X${displayNumber}-lock`;
    if (!(await exists(socket)) && !(await exists(lock))) {
      return displayNumber;
    }
  }
  throw new Error(
    "no free X11 display number is available for native captures",
  );
}

async function exists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function parseArguments(arguments_) {
  const parsed = {
    list: false,
    output: undefined,
    only: undefined,
    viewport: undefined,
    settleMilliseconds: 900,
  };

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    switch (argument) {
      case "--list":
        parsed.list = true;
        break;
      case "--output":
        parsed.output = requiredValue(arguments_, ++index, argument);
        break;
      case "--only":
        parsed.only = requiredValue(arguments_, ++index, argument);
        break;
      case "--viewport":
        parsed.viewport = requiredValue(arguments_, ++index, argument);
        break;
      case "--settle-ms": {
        const value = requiredValue(arguments_, ++index, argument);
        parsed.settleMilliseconds = Number.parseInt(value, 10);
        if (
          !Number.isSafeInteger(parsed.settleMilliseconds) ||
          parsed.settleMilliseconds < 0
        ) {
          throw new Error("--settle-ms must be a non-negative integer");
        }
        break;
      }
      default:
        throw new Error(`unknown capture option: ${argument}`);
    }
  }

  if (parsed.list && arguments_.length !== 1) {
    throw new Error("--list cannot be combined with capture options");
  }
  return parsed;
}

function requiredValue(arguments_, index, option) {
  const value = arguments_[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}
