import {
  access,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { createHash } from "node:crypto";
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
  stopProcesses,
  waitFor,
} from "./process-harness.mjs";
import {
  loadPresentationCaptureSnapshot,
  validatePresentationCaptureSnapshot,
} from "./presentation-snapshot.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const maximumCaptureDimension = 32_767;
const captureFontPaths = [
  "src/renderer/assets/fonts/LibreBaskerville-Variable.ttf",
  "src/renderer/assets/fonts/LibreBaskerville-Italic-Variable.ttf",
  "src/renderer/assets/fonts/IBMPlexSans-Variable.ttf",
  "src/renderer/assets/fonts/IBMPlexSans-Italic-Variable.ttf",
];
const customArtworkScenarios = new Set([
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
const ordinaryScenarioOmissions = new Set([
  "light-artwork",
  "non-square-artwork",
]);
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
} else if (options.profile !== undefined) {
  await captureVisualAcceptanceProfile(options);
} else if (options.all) {
  await captureOrdinaryScenarioSet(options);
} else if (options.scenario !== undefined) {
  await captureFocusedScenario(options);
}

async function captureFocusedScenario(options) {
  const captures = await preflightFocusedCapture(options);
  for (const capture of captures) {
    await captureControlledSession([capture], []);
  }
}

async function captureOrdinaryScenarioSet(options) {
  const captures = await preflightOrdinaryScenarioSet(options);
  const sessions = Map.groupBy(
    captures,
    (capture) => capture.viewport,
  ).values();
  await captureProgressiveSet(captures, sessions, "All-scenario capture");
}

async function captureVisualAcceptanceProfile(options) {
  const captures = await preflightVisualAcceptanceProfile(options);
  await captureProgressiveSet(
    captures,
    groupCompatibleCaptures(captures),
    "Visual-acceptance profile",
  );
}

async function captureProgressiveSet(captures, sessions, incompleteSetName) {
  const completedPaths = [];

  try {
    for (const sessionCaptures of sessions) {
      await captureControlledSession(sessionCaptures, completedPaths);
    }
  } catch (error) {
    const completed =
      completedPaths.length === 0
        ? "none"
        : completedPaths.map((capturePath) => `- ${capturePath}`).join("\n");
    throw new Error(
      `${incompleteSetName} is incomplete (${completedPaths.length}/${captures.length} captures completed).\nCompleted captures:\n${completed}\nFailure: ${errorMessage(error)}`,
      { cause: error },
    );
  }
}

function groupCompatibleCaptures(captures) {
  const sessionGroups = [];
  for (const capture of captures) {
    const compatibleGroup = sessionGroups.find(([firstCapture]) =>
      capturesShareSession(firstCapture, capture),
    );
    if (compatibleGroup === undefined) {
      sessionGroups.push([capture]);
    } else {
      compatibleGroup.push(capture);
    }
  }
  return sessionGroups;
}

function capturesShareSession(first, second) {
  return (
    first.viewport === second.viewport &&
    first.typography === second.typography &&
    first.diagnostics === second.diagnostics
  );
}

async function preflightVisualAcceptanceProfile({ output, overwrite }) {
  const outputDirectory = path.resolve(process.cwd(), output);
  const plan = buildPresentationCapturePlan();
  const failures = [];
  const snapshots = new Map();
  const runtimeInputPaths = new Set(captureFontPaths);

  for (const { fixture } of plan) {
    runtimeInputPaths.add(fixture);
    if (snapshots.has(fixture)) {
      continue;
    }
    try {
      const snapshot = await loadPresentationCaptureSnapshot(fixture);
      snapshots.set(fixture, snapshot);
      if (typeof snapshot.artwork?.path === "string") {
        runtimeInputPaths.add(snapshot.artwork.path);
      }
    } catch (error) {
      failures.push(
        `required Fixture Scenario snapshot is invalid: ${fixture}: ${errorMessage(error)}`,
      );
    }
  }

  const captures = plan.map((capture) => ({
    ...capture,
    finalCapturePath: path.join(outputDirectory, capture.fileName),
    snapshot: snapshots.get(capture.fixture),
  }));
  await addUnreadableInputFailures(runtimeInputPaths, failures);
  const availableExecutables = await addMissingExecutableFailures(failures);
  await addDestinationFailures(captures, outputDirectory, overwrite, failures);
  await addRendererBuildFailure(availableExecutables, failures);
  assertCapturePreflightSucceeded(failures);
  return captures;
}

async function captureControlledSession(captures, completedPaths) {
  const [firstCapture] = captures;
  const { width, height, viewport, typography, diagnostics } = firstCapture;
  const runtimeDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-controlled-capture."),
  );
  const controlSocketPath = path.join(runtimeDirectory, "capture-control.sock");
  const displayConfigurationPath = path.join(runtimeDirectory, "display.json");
  const controlServer = createServer();
  let control;
  let renderer;
  let xvfb;
  let captureFailure;

  try {
    await writeFile(
      displayConfigurationPath,
      `${JSON.stringify(captureDisplayConfiguration, null, 2)}\n`,
    );
    await new Promise((resolve, reject) => {
      controlServer.once("error", reject);
      controlServer.listen(controlSocketPath, resolve);
    });
    const displaySession = await startXvfbDisplay({
      width,
      height,
      cwd: repositoryRoot,
      description: "the controlled native capture display",
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
      ROONSCAPE_DIAGNOSTICS: diagnostics ? "1" : "0",
      ROONSCAPE_STATIC_FIXTURE: "1",
    };
    if (typography === "automatic") {
      delete environment.ROONSCAPE_CAPTURE_TYPOGRAPHY;
    } else {
      environment.ROONSCAPE_CAPTURE_TYPOGRAPHY = typography;
    }
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
    let windowId;

    for (const [index, capture] of captures.entries()) {
      const revision = index + 1;
      const snapshot = structuredClone(capture.snapshot);
      if (capture.customArtwork !== undefined) {
        const artworkPath = path.join(
          runtimeDirectory,
          `custom-artwork-${revision}`,
        );
        await writeFile(artworkPath, capture.customArtwork.contents, {
          flag: "wx",
          mode: 0o600,
        });
        snapshot.artwork = { revision: 1, path: artworkPath };
      }
      snapshot.revision = revision;
      await validatePresentationCaptureSnapshot(snapshot);
      process.stderr.write(
        `Capturing Fixture Scenario ${capture.scenario} at ${viewport}\n`,
      );
      control.write(
        `${JSON.stringify({
          type: "select",
          scenario: capture.scenario,
          revision,
          snapshot,
        })}\n`,
      );
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
      windowId ??= await waitForRoonScapeWindow(
        renderer,
        environment,
        width,
        height,
      );
      const publicationDirectory = await mkdtemp(
        path.join(
          path.dirname(capture.finalCapturePath),
          ".roonscape-capture.",
        ),
      );
      try {
        const temporaryCapturePath = path.join(
          publicationDirectory,
          "capture.png",
        );
        await captureNativeWindow(
          windowId,
          temporaryCapturePath,
          environment,
          width,
          height,
        );
        await rename(temporaryCapturePath, capture.finalCapturePath);
        completedPaths.push(capture.finalCapturePath);
        process.stdout.write(`${capture.finalCapturePath}\n`);
      } finally {
        await rm(publicationDirectory, { force: true, recursive: true });
      }
    }
  } catch (error) {
    captureFailure = error;
    throw error;
  } finally {
    control?.destroy();
    await stopProcesses([renderer, xvfb], { failure: captureFailure });
    if (controlServer.listening) {
      await new Promise((resolve) => controlServer.close(resolve));
    }
    await rm(runtimeDirectory, { force: true, recursive: true });
  }
}

async function preflightFocusedCapture({
  artwork,
  output,
  overwrite,
  resolutions,
  scenario,
}) {
  const selected = selectFocusedPresentationCapture(
    buildPresentationCapturePlan(),
    scenario,
  );
  if (artwork !== undefined && !customArtworkScenarios.has(selected.scenario)) {
    throw new Error(
      `--artwork is incompatible with Fixture Scenario ${selected.scenario}`,
    );
  }
  const customArtwork =
    artwork === undefined ? undefined : await validateCustomArtwork(artwork);
  const requestedResolutions = normalizeCaptureResolutions(resolutions);

  const outputDirectory = path.resolve(process.cwd(), output ?? ".");
  const captures = requestedResolutions.map((resolution) => {
    const { viewport } = resolution;
    const artworkIdentity =
      customArtwork === undefined
        ? ""
        : `--${customArtwork.sanitizedBasename}--${customArtwork.contentHash}`;
    const fileName = `${viewport}--${selected.scenario}${artworkIdentity}.png`;
    return {
      ...selected,
      ...resolution,
      fileName,
      finalCapturePath: path.join(outputDirectory, fileName),
    };
  });
  const failures = [];

  const availableExecutables = await addMissingExecutableFailures(failures);
  const runtimeInputPaths = [...captureFontPaths, selected.fixture];
  let selectedSnapshot;
  try {
    selectedSnapshot = await loadPresentationCaptureSnapshot(selected.fixture);
    if (
      customArtwork === undefined &&
      typeof selectedSnapshot.artwork?.path === "string"
    ) {
      runtimeInputPaths.push(selectedSnapshot.artwork.path);
    }
  } catch (error) {
    failures.push(
      `required Fixture Scenario snapshot is invalid: ${selected.fixture}: ${errorMessage(error)}`,
    );
  }
  await addUnreadableInputFailures(runtimeInputPaths, failures);
  await addDestinationFailures(captures, outputDirectory, overwrite, failures);
  await addRendererBuildFailure(availableExecutables, failures);
  assertCapturePreflightSucceeded(failures);
  return captures.map((capture) => ({
    ...capture,
    snapshot: selectedSnapshot,
    customArtwork,
  }));
}

async function preflightOrdinaryScenarioSet({
  artwork,
  output,
  overwrite,
  resolutions,
}) {
  const selectedScenarios = buildPresentationCapturePlan().filter(
    (capture) =>
      capture.variant === "matrix" &&
      capture.viewport === "3840x2160" &&
      !ordinaryScenarioOmissions.has(capture.scenario),
  );
  const customArtwork =
    artwork === undefined ? undefined : await validateCustomArtwork(artwork);
  const requestedResolutions = normalizeCaptureResolutions(resolutions);

  const outputDirectory = path.resolve(process.cwd(), output ?? ".");
  const failures = [];
  const snapshots = new Map();
  const runtimeInputPaths = new Set(captureFontPaths);
  for (const selected of selectedScenarios) {
    runtimeInputPaths.add(selected.fixture);
    try {
      const snapshot = await loadPresentationCaptureSnapshot(selected.fixture);
      snapshots.set(selected.fixture, snapshot);
      if (
        (!customArtworkScenarios.has(selected.scenario) ||
          customArtwork === undefined) &&
        typeof snapshot.artwork?.path === "string"
      ) {
        runtimeInputPaths.add(snapshot.artwork.path);
      }
    } catch (error) {
      failures.push(
        `required Fixture Scenario snapshot is invalid: ${selected.fixture}: ${errorMessage(error)}`,
      );
    }
  }
  const captures = requestedResolutions.flatMap((resolution) =>
    selectedScenarios.map((selected) => {
      const substitutesArtwork =
        customArtwork !== undefined &&
        customArtworkScenarios.has(selected.scenario);
      const artworkIdentity = substitutesArtwork
        ? `--${customArtwork.sanitizedBasename}--${customArtwork.contentHash}`
        : "";
      const fileName = `${resolution.viewport}--${selected.scenario}${artworkIdentity}.png`;
      return {
        ...selected,
        ...resolution,
        fileName,
        finalCapturePath: path.join(outputDirectory, fileName),
        snapshot: snapshots.get(selected.fixture),
        customArtwork: substitutesArtwork ? customArtwork : undefined,
      };
    }),
  );

  const availableExecutables = await addMissingExecutableFailures(failures);
  await addUnreadableInputFailures(runtimeInputPaths, failures);
  await addDestinationFailures(captures, outputDirectory, overwrite, failures);
  await addRendererBuildFailure(availableExecutables, failures);
  assertCapturePreflightSucceeded(failures);
  return captures;
}

function normalizeCaptureResolutions(resolutions) {
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
  return requestedResolutions;
}

async function validateCustomArtwork(requestedPath) {
  const artworkPath = path.resolve(process.cwd(), requestedPath);
  let artworkStats;
  try {
    artworkStats = await stat(artworkPath);
  } catch (error) {
    if (error?.code === "ENOENT") {
      throw new Error(`custom artwork does not exist: ${artworkPath}`, {
        cause: error,
      });
    }
    throw new Error(
      `could not inspect custom artwork ${artworkPath}: ${errorMessage(error)}`,
      { cause: error },
    );
  }
  if (!artworkStats.isFile()) {
    throw new Error(`custom artwork is not a file: ${artworkPath}`);
  }
  if (artworkStats.size === 0) {
    throw new Error(`custom artwork is empty: ${artworkPath}`);
  }
  try {
    await access(artworkPath, fileConstants.R_OK);
  } catch {
    throw new Error(`custom artwork is unreadable: ${artworkPath}`);
  }
  let contents;
  try {
    contents = await readFile(artworkPath);
  } catch {
    throw new Error(`custom artwork is unreadable: ${artworkPath}`);
  }
  const sanitizedBasename =
    path
      .basename(artworkPath)
      .toLowerCase()
      .replaceAll(/[^a-z0-9]+/g, "-")
      .replaceAll(/^-|-$/g, "")
      .slice(0, 80)
      .replace(/-$/, "") || "artwork";
  return {
    contents,
    sanitizedBasename,
    contentHash: createHash("sha256")
      .update(contents)
      .digest("hex")
      .slice(0, 12),
  };
}

async function addUnreadableInputFailures(inputPaths, failures) {
  for (const inputPath of inputPaths) {
    try {
      await access(path.join(repositoryRoot, inputPath), fileConstants.R_OK);
    } catch {
      failures.push(`required capture input is unreadable: ${inputPath}`);
    }
  }
}

async function addMissingExecutableFailures(failures) {
  const availableExecutables = new Set();
  for (const executableName of ["Xvfb", "xwininfo", "scrot", "cargo"]) {
    if (await executableOnPath(executableName)) {
      availableExecutables.add(executableName);
    } else {
      failures.push(`required executable is unavailable: ${executableName}`);
    }
  }
  return availableExecutables;
}

async function addDestinationFailures(
  captures,
  outputDirectory,
  overwrite,
  failures,
) {
  try {
    await mkdir(outputDirectory, { recursive: true });
    const outputStats = await stat(outputDirectory);
    if (!outputStats.isDirectory()) {
      failures.push(`capture output is not a directory: ${outputDirectory}`);
      return;
    }
    await access(
      outputDirectory,
      fileConstants.R_OK | fileConstants.W_OK | fileConstants.X_OK,
    );
  } catch (error) {
    failures.push(
      `capture output is unavailable: ${outputDirectory}: ${errorMessage(error)}`,
    );
    return;
  }

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
    failures.push(`destination files already exist:\n${collisions.join("\n")}`);
  }
}

async function addRendererBuildFailure(availableExecutables, failures) {
  if (!availableExecutables.has("cargo") || failures.length > 0) {
    return;
  }
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

function assertCapturePreflightSucceeded(failures) {
  if (failures.length > 0) {
    throw new Error(
      `Presentation Capture preflight failed:\n${failures.map((failure) => `- ${failure}`).join("\n")}`,
    );
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

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function parseArguments(arguments_) {
  const parsed = {
    all: false,
    artwork: undefined,
    listScenarios: false,
    output: undefined,
    overwrite: false,
    profile: undefined,
    resolutions: [],
    scenario: undefined,
  };

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    switch (argument) {
      case "--all":
        rejectDuplicateOption(parsed.all, argument);
        parsed.all = true;
        break;
      case "--artwork":
        rejectDuplicateOption(parsed.artwork !== undefined, argument);
        parsed.artwork = requiredValue(arguments_, ++index, argument);
        break;
      case "--list":
        throw new Error("--list was removed; use --list-scenarios");
      case "--list-scenarios":
        rejectDuplicateOption(parsed.listScenarios, argument);
        parsed.listScenarios = true;
        break;
      case "--output":
        rejectDuplicateOption(parsed.output !== undefined, argument);
        parsed.output = requiredValue(arguments_, ++index, argument);
        break;
      case "--profile":
        rejectDuplicateOption(parsed.profile !== undefined, argument);
        parsed.profile = requiredValue(arguments_, ++index, argument);
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
        throw new Error("--only was removed; use --scenario");
      case "--viewport":
        throw new Error("--viewport was removed; use --resolution");
      case "--settle-ms":
        throw new Error(
          "--settle-ms was removed; Presentation Captures now wait for painted-frame readiness",
        );
      default:
        throw new Error(`unknown capture option: ${argument}`);
    }
  }

  if (parsed.listScenarios && arguments_.length !== 1) {
    throw new Error("--list-scenarios cannot be combined with capture options");
  }
  if (parsed.profile !== undefined && parsed.profile !== "visual-acceptance") {
    throw new Error(`unknown capture profile: ${parsed.profile}`);
  }
  if (parsed.profile === "visual-acceptance") {
    const incompatibleOption = [
      [parsed.scenario !== undefined, "--scenario"],
      [parsed.all, "--all"],
      [parsed.artwork !== undefined, "--artwork"],
      [parsed.resolutions.length > 0, "--resolution"],
    ].find(([present]) => present)?.[1];
    if (incompatibleOption !== undefined) {
      throw new Error(
        `--profile visual-acceptance cannot be combined with ${incompatibleOption}`,
      );
    }
    if (parsed.output === undefined) {
      throw new Error("--profile visual-acceptance requires --output");
    }
  }
  if (parsed.all && parsed.scenario !== undefined) {
    throw new Error("--all and --scenario cannot be combined");
  }
  if (
    parsed.scenario === undefined &&
    !parsed.all &&
    parsed.profile === undefined &&
    !parsed.listScenarios
  ) {
    throw new Error(
      "a Presentation Capture selector is required: use --scenario, --all, or --profile visual-acceptance",
    );
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
