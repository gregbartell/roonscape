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

import { buildPresentationCapturePlan } from "./presentation-captures.mjs";
import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcess,
  waitFor,
} from "./process-harness.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const scratchRoot = "/tmp/codex/roonscape";
const nativeRenderer = "native GTK 4/Pango";
const defaultSettleMilliseconds = 1_500;
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
    `${JSON.stringify(buildPresentationCapturePlan(), null, 2)}\n`,
  );
} else {
  await capturePresentations(options);
}

async function capturePresentations({
  output,
  only,
  viewport,
  settleMilliseconds,
}) {
  const plan = buildPresentationCapturePlan().filter(
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
    const { display, xvfb } = await startXvfbDisplay({
      width: firstCapture.width,
      height: firstCapture.height,
      cwd: repositoryRoot,
      description: "the native capture display",
    });

    try {
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
      await stopProcess(xvfb);
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
  process.stdout.write(`Presentation captures written to ${outputDirectory}\n`);
}

async function captureFixture(
  capture,
  display,
  outputDirectory,
  settleMilliseconds,
) {
  const runtimeDirectory = await mkdtemp(
    path.join(scratchRoot, "presentation-capture."),
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
    publisher = startLongRunning(
      process.execPath,
      [path.join(repositoryRoot, "src/bridge/dist/src/fixture.js")],
      environment,
    );
    await publisher.spawned;
    await waitFor(
      () => access(socketPath),
      publisher,
      "the fixture publisher",
      { retryMilliseconds: 25 },
    );
    renderer = startLongRunning(
      "cargo",
      [
        "run",
        "--quiet",
        "--package",
        "roonscape-renderer",
        "--",
        "--config",
        displayConfigurationPath,
      ],
      environment,
    );
    await renderer.spawned;
    const windowId = await waitForRoonScapeWindow(
      renderer,
      environment,
      capture.width,
      capture.height,
    );
    await delay(settleMilliseconds);
    assertProcessRunning(renderer, "the native renderer");

    const capturePath = path.join(outputDirectory, capture.fileName);
    await run(
      "scrot",
      ["--window", windowId, "--overwrite", capturePath],
      environment,
    );
    await verifyPngDimensions(capturePath, capture.width, capture.height);
  } finally {
    await Promise.all([stopProcess(renderer), stopProcess(publisher)]);
    await rm(runtimeDirectory, { force: true, recursive: true });
  }
}

function startLongRunning(command, arguments_, environment = process.env) {
  return startMonitoredProcess(command, arguments_, {
    cwd: repositoryRoot,
    environment,
  });
}

async function run(command, arguments_, environment) {
  return runMonitoredProcess(command, arguments_, {
    cwd: repositoryRoot,
    environment,
    timeoutMilliseconds: 5_000,
  });
}

async function waitForRoonScapeWindow(
  renderer,
  environment,
  expectedWidth,
  expectedHeight,
) {
  let lastWindow;
  for (let attempt = 0; attempt < 300; attempt += 1) {
    assertProcessRunning(renderer, "the native renderer");
    try {
      const output = await run(
        "xwininfo",
        ["-name", "RoonScape", "-int"],
        environment,
      );
      const window = parseWindowInformation(output);
      lastWindow = window;
      if (window.width === expectedWidth && window.height === expectedHeight) {
        return window.id;
      }
    } catch (error) {
      if (error?.code === "ENOENT") {
        throw new Error("xwininfo is required for native capture readiness", {
          cause: error,
        });
      }
    }
    await delay(100);
  }

  if (lastWindow !== undefined) {
    throw new Error(
      `native RoonScape window remained ${lastWindow.width}x${lastWindow.height}; expected ${expectedWidth}x${expectedHeight}`,
    );
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
    return mkdtemp(path.join(scratchRoot, "presentation-captures."));
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

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function parseArguments(arguments_) {
  const parsed = {
    list: false,
    output: undefined,
    only: undefined,
    viewport: undefined,
    settleMilliseconds: defaultSettleMilliseconds,
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
