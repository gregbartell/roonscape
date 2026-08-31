import { once } from "node:events";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcesses,
  waitFor,
} from "./process-harness.mjs";
import { validatePresentationCaptureSnapshot } from "./presentation-snapshot.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const captureDisplayConfiguration = {
  trackedOutputId: "visual-acceptance-capture",
  inactivity: {
    gracePeriodSeconds: 3600,
    dimmedOpacity: 0.35,
    repositionCadenceSeconds: 60,
  },
};

export async function runControlledRendererSession(
  captures,
  {
    environment = process.env,
    publishCapture,
    onCaptureStarted = () => {},
    onCapturePublished = () => {},
  },
) {
  const [firstCapture] = captures;
  const { width, height } = firstCapture;
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
      environment,
      description: "the controlled native capture display",
    });
    xvfb = displaySession.xvfb;
    const connected = once(controlServer, "connection");
    const rendererEnvironment = buildRendererEnvironment(
      environment,
      displaySession.display,
      controlSocketPath,
      firstCapture,
    );
    renderer = startNativeRenderer(
      displayConfigurationPath,
      rendererEnvironment,
    );
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
      const snapshot = await prepareSnapshot(
        capture,
        revision,
        runtimeDirectory,
      );
      onCaptureStarted(capture);
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
      assertExpectedAcknowledgement(acknowledgement, capture, revision);
      windowId ??= await waitForRoonScapeWindow(
        renderer,
        rendererEnvironment,
        width,
        height,
      );
      await publishCapture({
        finalCapturePath: capture.finalCapturePath,
        width,
        height,
        produce: (temporaryCapturePath) =>
          captureNativeWindow(
            windowId,
            temporaryCapturePath,
            rendererEnvironment,
          ),
      });
      onCapturePublished(capture.finalCapturePath);
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

function buildRendererEnvironment(
  environment,
  display,
  controlSocketPath,
  { viewport, typography, diagnostics },
) {
  const rendererEnvironment = {
    ...environment,
    DISPLAY: display,
    GDK_BACKEND: "x11",
    NO_AT_BRIDGE: "1",
    ROONSCAPE_CAPTURE_CONTROL: controlSocketPath,
    ROONSCAPE_CAPTURE_VIEWPORT: viewport,
    ROONSCAPE_DIAGNOSTICS: diagnostics ? "1" : "0",
    ROONSCAPE_STATIC_FIXTURE: "1",
  };
  if (typography === "automatic") {
    delete rendererEnvironment.ROONSCAPE_CAPTURE_TYPOGRAPHY;
  } else {
    rendererEnvironment.ROONSCAPE_CAPTURE_TYPOGRAPHY = typography;
  }
  delete rendererEnvironment.ROONSCAPE_DISPLAY_CONFIG;
  delete rendererEnvironment.ROONSCAPE_FIXTURE;
  delete rendererEnvironment.ROONSCAPE_FIXTURE_AUTO_CLOSE_MS;
  delete rendererEnvironment.ROONSCAPE_FIXTURE_CONTROL;
  delete rendererEnvironment.ROONSCAPE_SOCKET;
  return rendererEnvironment;
}

async function prepareSnapshot(capture, revision, runtimeDirectory) {
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
  return snapshot;
}

function assertExpectedAcknowledgement(acknowledgement, capture, revision) {
  if (
    acknowledgement.type !== "painted" ||
    acknowledgement.scenario !== capture.scenario ||
    acknowledgement.revision !== revision
  ) {
    throw new Error(
      `renderer acknowledged an unexpected Fixture Scenario revision: ${JSON.stringify(acknowledgement)}`,
    );
  }
}

function startNativeRenderer(displayConfigurationPath, environment) {
  return startMonitoredProcess(
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
    { cwd: repositoryRoot, environment },
  );
}

async function captureNativeWindow(windowId, capturePath, environment) {
  await runMonitoredProcess(
    "scrot",
    ["--window", windowId, "--overwrite", capturePath],
    {
      cwd: repositoryRoot,
      environment,
      timeoutMilliseconds: 5_000,
    },
  );
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
      const output = await runMonitoredProcess(
        "xwininfo",
        ["-name", "RoonScape", "-int"],
        {
          cwd: repositoryRoot,
          environment,
          timeoutMilliseconds: 5_000,
        },
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

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
