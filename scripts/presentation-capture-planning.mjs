import { createHash } from "node:crypto";
import { constants as fileConstants } from "node:fs";
import { access, lstat, mkdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  buildPresentationCapturePlan,
  presentationCaptureResolution,
  selectFocusedPresentationCapture,
} from "./presentation-captures.mjs";
import { runMonitoredProcess } from "./process-harness.mjs";
import { loadPresentationCaptureSnapshot } from "./presentation-snapshot.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
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

export async function planPresentationCaptures(
  request,
  { workingDirectory = process.cwd(), environment = process.env } = {},
) {
  if (request.profile !== undefined) {
    const captures = await preflightVisualAcceptanceProfile(request, {
      workingDirectory,
      environment,
    });
    return {
      captures,
      sessions: groupCompatibleCaptures(captures),
      incompleteSetName: "Visual-acceptance profile",
    };
  }
  if (request.all) {
    const captures = await preflightOrdinaryScenarioSet(request, {
      workingDirectory,
      environment,
    });
    return {
      captures,
      sessions: [...Map.groupBy(captures, ({ viewport }) => viewport).values()],
      incompleteSetName: "All-scenario capture",
    };
  }

  const captures = await preflightFocusedCapture(request, {
    workingDirectory,
    environment,
  });
  return {
    captures,
    sessions: captures.map((capture) => [capture]),
    incompleteSetName: undefined,
  };
}

async function preflightVisualAcceptanceProfile(
  { output, overwrite },
  context,
) {
  const outputDirectory = path.resolve(context.workingDirectory, output);
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
  const availableExecutables = await addMissingExecutableFailures(
    failures,
    context.environment,
  );
  await addDestinationFailures(captures, outputDirectory, overwrite, failures);
  await addRendererBuildFailure(
    availableExecutables,
    failures,
    context.environment,
  );
  assertCapturePreflightSucceeded(failures);
  return captures;
}

async function preflightFocusedCapture(
  { artwork, output, overwrite, resolutions, scenario },
  context,
) {
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
    artwork === undefined
      ? undefined
      : await validateCustomArtwork(artwork, context.workingDirectory);
  const requestedResolutions = normalizeCaptureResolutions(resolutions);
  const outputDirectory = path.resolve(context.workingDirectory, output ?? ".");
  const captures = requestedResolutions.map((resolution) => {
    const artworkIdentity =
      customArtwork === undefined
        ? ""
        : `--${customArtwork.sanitizedBasename}--${customArtwork.contentHash}`;
    const fileName = `${resolution.viewport}--${selected.scenario}${artworkIdentity}.png`;
    return {
      ...selected,
      ...resolution,
      fileName,
      finalCapturePath: path.join(outputDirectory, fileName),
    };
  });
  const failures = [];
  const availableExecutables = await addMissingExecutableFailures(
    failures,
    context.environment,
  );
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
  await addRendererBuildFailure(
    availableExecutables,
    failures,
    context.environment,
  );
  assertCapturePreflightSucceeded(failures);
  return captures.map((capture) => ({
    ...capture,
    snapshot: selectedSnapshot,
    customArtwork,
  }));
}

async function preflightOrdinaryScenarioSet(
  { artwork, output, overwrite, resolutions },
  context,
) {
  const selectedScenarios = buildPresentationCapturePlan().filter(
    (capture) =>
      capture.variant === "matrix" &&
      capture.viewport === "3840x2160" &&
      !ordinaryScenarioOmissions.has(capture.scenario),
  );
  const customArtwork =
    artwork === undefined
      ? undefined
      : await validateCustomArtwork(artwork, context.workingDirectory);
  const requestedResolutions = normalizeCaptureResolutions(resolutions);
  const outputDirectory = path.resolve(context.workingDirectory, output ?? ".");
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

  const availableExecutables = await addMissingExecutableFailures(
    failures,
    context.environment,
  );
  await addUnreadableInputFailures(runtimeInputPaths, failures);
  await addDestinationFailures(captures, outputDirectory, overwrite, failures);
  await addRendererBuildFailure(
    availableExecutables,
    failures,
    context.environment,
  );
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

async function validateCustomArtwork(requestedPath, workingDirectory) {
  const artworkPath = path.resolve(workingDirectory, requestedPath);
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

async function addMissingExecutableFailures(failures, environment) {
  const availableExecutables = new Set();
  for (const executableName of ["Xvfb", "xwininfo", "scrot", "cargo"]) {
    if (await executableOnPath(executableName, environment)) {
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

async function addRendererBuildFailure(
  availableExecutables,
  failures,
  environment,
) {
  if (!availableExecutables.has("cargo") || failures.length > 0) {
    return;
  }
  try {
    await runMonitoredProcess(
      "cargo",
      ["build", "--locked", "--package", "roonscape-renderer"],
      {
        cwd: repositoryRoot,
        environment,
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

async function executableOnPath(executableName, environment) {
  const searchDirectories = (environment.PATH ?? "").split(path.delimiter);
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

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
