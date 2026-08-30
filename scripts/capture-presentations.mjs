import {
  access,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { constants as fileConstants } from "node:fs";
import { once } from "node:events";
import { createServer } from "node:net";
import path from "node:path";
import { tmpdir } from "node:os";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

import {
  buildPresentationCapturePlan,
  listFixtureScenarios,
  presentationCaptureResolution,
  selectFocusedPresentationCapture,
} from "./presentation-captures.mjs";
import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcess,
  waitFor,
} from "./process-harness.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const nativeRenderer = "native GTK 4/Pango";
const defaultSettleMilliseconds = 1_500;
const maximumCaptureDimension = 32_767;
const captureDisplayConfiguration = {
  trackedOutputId: "visual-acceptance-capture",
  inactivity: {
    gracePeriodSeconds: 3600,
    dimmedOpacity: 0.35,
    repositionCadenceSeconds: 60,
  },
};
const options = parseArguments(process.argv.slice(2));

if (options.listScenarios) {
  process.stdout.write(
    listFixtureScenarios()
      .map(({ scenario, label }) => `${scenario}\t${label}\n`)
      .join(""),
  );
} else if (options.list) {
  process.stdout.write(
    `${JSON.stringify(buildPresentationCapturePlan(), null, 2)}\n`,
  );
} else if (options.scenario !== undefined) {
  await captureFocusedScenario(options);
} else {
  await capturePresentations(options);
}

async function captureFocusedScenario(options) {
  const captures = await preflightFocusedCapture(options);
  for (const capture of captures) {
    await captureFocusedResolution(capture);
  }
}

async function preflightFocusedCapture({
  output,
  overwrite,
  resolutions,
  scenario,
}) {
  const selected = selectFocusedPresentationCapture(
    buildPresentationCapturePlan(),
    scenario,
  );
  const requestedResolutions =
    resolutions.length === 0
      ? [presentationCaptureResolution(3840, 2160)]
      : resolutions;
  const duplicateResolution = requestedResolutions.find(
    ({ viewport }, index) =>
      requestedResolutions.findIndex(
        (candidate) => candidate.viewport === viewport,
      ) !== index,
  );
  if (duplicateResolution !== undefined) {
    throw new Error(`duplicate --resolution: ${duplicateResolution.viewport}`);
  }

  const outputDirectory = path.resolve(process.cwd(), output ?? ".");
  const captures = requestedResolutions.map((resolution) => {
    const { viewport } = resolution;
    const fileName = `${viewport}--${selected.scenario}.png`;
    return {
      ...selected,
      ...resolution,
      fileName,
      finalCapturePath: path.join(outputDirectory, fileName),
    };
  });
  const failures = [];

  const availableExecutables = new Set();
  for (const executableName of ["Xvfb", "xwininfo", "scrot", "cargo"]) {
    if (await executableOnPath(executableName)) {
      availableExecutables.add(executableName);
    } else {
      failures.push(`required executable is unavailable: ${executableName}`);
    }
  }
  const runtimeInputPaths = [
    "src/renderer/assets/fonts/LibreBaskerville-Variable.ttf",
    "src/renderer/assets/fonts/LibreBaskerville-Italic-Variable.ttf",
    "src/renderer/assets/fonts/IBMPlexSans-Variable.ttf",
    "src/renderer/assets/fonts/IBMPlexSans-Italic-Variable.ttf",
    selected.fixture,
  ];
  let selectedSnapshot;
  try {
    selectedSnapshot = JSON.parse(
      await readFile(path.join(repositoryRoot, selected.fixture), "utf8"),
    );
    if (typeof selectedSnapshot.artwork?.path === "string") {
      runtimeInputPaths.push(selectedSnapshot.artwork.path);
    }
  } catch (error) {
    failures.push(
      `required Fixture Scenario snapshot is invalid: ${selected.fixture}: ${errorMessage(error)}`,
    );
  }
  for (const inputPath of runtimeInputPaths) {
    try {
      await access(path.join(repositoryRoot, inputPath), fileConstants.R_OK);
    } catch {
      failures.push(`required capture input is unreadable: ${inputPath}`);
    }
  }

  let outputReady = false;
  try {
    await mkdir(outputDirectory, { recursive: true });
    const outputStats = await stat(outputDirectory);
    if (!outputStats.isDirectory()) {
      failures.push(`capture output is not a directory: ${outputDirectory}`);
    } else {
      await access(
        outputDirectory,
        fileConstants.R_OK | fileConstants.W_OK | fileConstants.X_OK,
      );
      outputReady = true;
    }
  } catch (error) {
    failures.push(
      `capture output is unavailable: ${outputDirectory}: ${errorMessage(error)}`,
    );
  }

  if (outputReady) {
    const collisions = [];
    for (const { finalCapturePath } of captures) {
      try {
        const destinationStats = await lstat(finalCapturePath);
        if (!destinationStats.isFile() && !destinationStats.isSymbolicLink()) {
          failures.push(
            `destination is not a replaceable file: ${finalCapturePath}`,
          );
        } else if (!overwrite) {
          collisions.push(finalCapturePath);
        }
      } catch (error) {
        if (error?.code !== "ENOENT") {
          failures.push(
            `could not inspect destination ${finalCapturePath}: ${errorMessage(error)}`,
          );
        }
      }
    }
    if (collisions.length > 0) {
      failures.push(
        `destination files already exist:\n${collisions.join("\n")}`,
      );
    }
  }

  if (availableExecutables.has("cargo") && failures.length === 0) {
    try {
      await runMonitoredProcess(
        "cargo",
        ["build", "--locked", "--package", "roonscape-renderer"],
        {
          cwd: repositoryRoot,
          environment: process.env,
          description: "the renderer build preflight",
          timeoutMilliseconds: 300_000,
        },
      );
    } catch (error) {
      failures.push(`renderer build preflight failed: ${errorMessage(error)}`);
    }
  }

  if (failures.length > 0) {
    throw new Error(
      `Presentation Capture preflight failed:\n${failures.map((failure) => `- ${failure}`).join("\n")}`,
    );
  }
  return captures.map((capture) => ({
    ...capture,
    snapshot: selectedSnapshot,
  }));
}

async function captureFocusedResolution(capture) {
  const { width, height, viewport, finalCapturePath } = capture;
  const runtimeDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-focused-capture."),
  );
  const controlSocketPath = path.join(runtimeDirectory, "capture-control.sock");
  const displayConfigurationPath = path.join(runtimeDirectory, "display.json");
  const controlServer = createServer();
  let control;
  let publicationDirectory;
  let renderer;
  let xvfb;
  try {
    publicationDirectory = await mkdtemp(
      path.join(path.dirname(finalCapturePath), ".roonscape-capture."),
    );
    const temporaryCapturePath = path.join(publicationDirectory, "capture.png");
    await writeFile(
      displayConfigurationPath,
      `${JSON.stringify(captureDisplayConfiguration, null, 2)}\n`,
    );
    const snapshot = structuredClone(capture.snapshot);
    const revision = 1;
    snapshot.revision = revision;
    const selection = {
      type: "select",
      scenario: capture.scenario,
      revision,
      snapshot,
    };
    await new Promise((resolve, reject) => {
      controlServer.once("error", reject);
      controlServer.listen(controlSocketPath, resolve);
    });
    process.stderr.write(
      `Capturing Fixture Scenario ${capture.scenario} at ${viewport}\n`,
    );
    const displaySession = await startXvfbDisplay({
      width,
      height,
      cwd: repositoryRoot,
      description: "the focused native capture display",
    });
    xvfb = displaySession.xvfb;
    const connected = once(controlServer, "connection");
    const environment = {
      ...process.env,
      DISPLAY: displaySession.display,
      GDK_BACKEND: "x11",
      NO_AT_BRIDGE: "1",
      ROONSCAPE_CAPTURE_CONTROL: controlSocketPath,
      ROONSCAPE_CAPTURE_VIEWPORT: viewport,
      ROONSCAPE_DIAGNOSTICS: "0",
      ROONSCAPE_STATIC_FIXTURE: "1",
    };
    delete environment.ROONSCAPE_CAPTURE_TYPOGRAPHY;
    delete environment.ROONSCAPE_DISPLAY_CONFIG;
    delete environment.ROONSCAPE_FIXTURE;
    delete environment.ROONSCAPE_FIXTURE_AUTO_CLOSE_MS;
    delete environment.ROONSCAPE_FIXTURE_CONTROL;
    delete environment.ROONSCAPE_SOCKET;
    renderer = startNativeRenderer(displayConfigurationPath, environment);
    await renderer.spawned;
    [control] = await waitFor(
      () => connected,
      renderer,
      "the renderer capture control connection",
    );
    const acknowledgements = createInterface({ input: control })[
      Symbol.asyncIterator
    ]();
    control.write(`${JSON.stringify(selection)}\n`);
    const acknowledgement = await waitFor(
      async () => {
        const next = await acknowledgements.next();
        if (next.done) {
          throw new Error("capture control channel closed before readiness");
        }
        return JSON.parse(next.value);
      },
      renderer,
      `painted Fixture Scenario revision ${revision}`,
    );
    if (
      acknowledgement.type !== "painted" ||
      acknowledgement.scenario !== capture.scenario ||
      acknowledgement.revision !== revision
    ) {
      throw new Error(
        `renderer acknowledged an unexpected Fixture Scenario revision: ${JSON.stringify(acknowledgement)}`,
      );
    }
    const windowId = await waitForRoonScapeWindow(
      renderer,
      environment,
      width,
      height,
    );
    await captureNativeWindow(
      windowId,
      temporaryCapturePath,
      environment,
      width,
      height,
    );
    await rename(temporaryCapturePath, finalCapturePath);
    process.stdout.write(`${finalCapturePath}\n`);
  } finally {
    control?.destroy();
    await Promise.all([stopProcess(renderer), stopProcess(xvfb)]);
    if (controlServer.listening) {
      await new Promise((resolve) => controlServer.close(resolve));
    }
    await Promise.all([
      rm(runtimeDirectory, { force: true, recursive: true }),
      publicationDirectory === undefined
        ? Promise.resolve()
        : rm(publicationDirectory, { force: true, recursive: true }),
    ]);
  }
}

async function executableOnPath(executableName) {
  const searchDirectories = (process.env.PATH ?? "").split(path.delimiter);
  for (const directory of searchDirectories) {
    if (directory.length === 0) {
      continue;
    }
    try {
      await access(path.join(directory, executableName), fileConstants.X_OK);
      return true;
    } catch {
      // Continue searching the remaining PATH entries.
    }
  }
  return false;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
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
    path.join(tmpdir(), "roonscape-presentation-capture."),
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
    renderer = startNativeRenderer(displayConfigurationPath, environment);
    await renderer.spawned;
    const windowId = await waitForRoonScapeWindow(
      renderer,
      environment,
      capture.width,
      capture.height,
    );
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
    await delay(settleMilliseconds);
    assertProcessRunning(renderer, "the native renderer");

    const capturePath = path.join(outputDirectory, capture.fileName);
    await captureNativeWindow(
      windowId,
      capturePath,
      environment,
      capture.width,
      capture.height,
    );
  } finally {
    await Promise.all([stopProcess(renderer), stopProcess(publisher)]);
    await rm(runtimeDirectory, { force: true, recursive: true });
  }
}

function startNativeRenderer(displayConfigurationPath, environment) {
  return startLongRunning(
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
}

async function captureNativeWindow(
  windowId,
  capturePath,
  environment,
  width,
  height,
) {
  await run(
    "scrot",
    ["--window", windowId, "--overwrite", capturePath],
    environment,
  );
  await verifyPngDimensions(capturePath, width, height);
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
  if (requestedOutput === undefined) {
    return mkdtemp(path.join(tmpdir(), "roonscape-presentation-captures."));
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
    listScenarios: false,
    legacyOption: false,
    output: undefined,
    only: undefined,
    overwrite: false,
    resolutions: [],
    scenario: undefined,
    viewport: undefined,
    settleMilliseconds: defaultSettleMilliseconds,
  };

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    switch (argument) {
      case "--list":
        rejectDuplicateOption(parsed.list, argument);
        parsed.list = true;
        break;
      case "--list-scenarios":
        rejectDuplicateOption(parsed.listScenarios, argument);
        parsed.listScenarios = true;
        break;
      case "--output":
        rejectDuplicateOption(parsed.output !== undefined, argument);
        parsed.output = requiredValue(arguments_, ++index, argument);
        break;
      case "--scenario":
        rejectDuplicateOption(parsed.scenario !== undefined, argument);
        parsed.scenario = requiredValue(arguments_, ++index, argument);
        break;
      case "--resolution":
        parsed.resolutions.push(
          parseResolution(requiredValue(arguments_, ++index, argument)),
        );
        break;
      case "--overwrite":
        rejectDuplicateOption(parsed.overwrite, argument);
        parsed.overwrite = true;
        break;
      case "--only":
        parsed.legacyOption = true;
        parsed.only = requiredValue(arguments_, ++index, argument);
        break;
      case "--viewport":
        parsed.legacyOption = true;
        parsed.viewport = requiredValue(arguments_, ++index, argument);
        break;
      case "--settle-ms": {
        parsed.legacyOption = true;
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
  if (parsed.listScenarios && arguments_.length !== 1) {
    throw new Error("--list-scenarios cannot be combined with capture options");
  }
  if (parsed.scenario !== undefined && (parsed.list || parsed.legacyOption)) {
    throw new Error(
      "--scenario cannot be combined with legacy capture options",
    );
  }
  if (parsed.scenario === undefined && parsed.resolutions.length > 0) {
    throw new Error("--resolution requires --scenario");
  }
  if (parsed.scenario === undefined && parsed.overwrite) {
    throw new Error("--overwrite requires --scenario");
  }
  return parsed;
}

function parseResolution(value) {
  const match = value.match(/^(\d+)x(\d+)$/);
  if (match === null) {
    throw new Error("--resolution must use WIDTHxHEIGHT");
  }
  const width = Number(match[1]);
  const height = Number(match[2]);
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0
  ) {
    throw new Error("--resolution dimensions must be positive safe integers");
  }
  if (width < 1280 || height < 720) {
    throw new Error("--resolution must be at least 1280x720");
  }
  if (width <= height) {
    throw new Error("--resolution must be landscape");
  }
  if (width > maximumCaptureDimension || height > maximumCaptureDimension) {
    throw new Error(
      `--resolution exceeds the supported maximum of ${maximumCaptureDimension}`,
    );
  }
  return presentationCaptureResolution(width, height);
}

function rejectDuplicateOption(duplicate, option) {
  if (duplicate) {
    throw new Error(`duplicate capture option: ${option}`);
  }
}

function requiredValue(arguments_, index, option) {
  const value = arguments_[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}
