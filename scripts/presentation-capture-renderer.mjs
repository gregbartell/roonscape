import { once } from "node:events";
import { createServer } from "node:net";
import path from "node:path";
import { createInterface } from "node:readline";
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { runMonitoredProcess, waitFor } from "./process-harness.mjs";
import { createNativeSession, waitForNativeWindow } from "./native-session.mjs";
import { validatePresentationCaptureSnapshot } from "./presentation-snapshot.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const defaultRendererExecutable = path.join(
  repositoryRoot,
  "target/debug/roonscape-renderer",
);
const presentationCaptureEncodingTimeoutMilliseconds = 30_000;

export function createControlledRendererSessionAdapter({
  environment = process.env,
  publishCapture,
  rendererExecutable = defaultRendererExecutable,
  signal,
}) {
  return {
    execute: (captures, observer) =>
      runControlledRendererSession(captures, {
        environment,
        signal,
        publishCapture,
        rendererExecutable,
        onCaptureStarted: observer.captureStarted,
        onCapturePublished: observer.capturePublished,
      }),
  };
}

export async function runControlledRendererSession(
  captures,
  {
    environment = process.env,
    publishCapture,
    onCaptureStarted = () => {},
    onCapturePublished = () => {},
    rendererExecutable = defaultRendererExecutable,
    signal,
  },
) {
  const [firstCapture] = captures;
  const { width, height } = firstCapture;
  const session = await createNativeSession({
    width,
    height,
    environment,
    signal,
  });
  const { runtimeDirectory, configurationPath: displayConfigurationPath } =
    session;
  const controlSocketPath = path.join(runtimeDirectory, "capture-control.sock");
  const controlServer = createServer();
  let control;
  let renderer;
  let captureFailure;
  controlServer.on("connection", (socket) => {
    if (control === undefined) control = socket;
    else socket.destroy();
  });

  try {
    await new Promise((resolve, reject) => {
      controlServer.once("error", reject);
      controlServer.listen(controlSocketPath, resolve);
    });
    const connected = once(controlServer, "connection");
    const rendererEnvironment = buildRendererEnvironment(
      session.environment,
      controlSocketPath,
      firstCapture,
    );
    renderer = session.startProcess(
      rendererExecutable,
      ["--config", displayConfigurationPath],
      rendererEnvironment,
    );
    await renderer.spawned;
    [control] = await waitFor(
      () => connected,
      renderer,
      "the renderer capture control connection",
      { signal },
    );
    const acknowledgements = createInterface({ input: control })[
      Symbol.asyncIterator
    ]();

    for (const [index, capture] of captures.entries()) {
      signal?.throwIfAborted();
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
        { signal },
      );
      assertExpectedAcknowledgement(acknowledgement, capture, revision);
      if (index === 0)
        await waitForNativeWindow(
          renderer,
          rendererEnvironment,
          width,
          height,
          { signal },
        );
      await publishCapture({
        finalCapturePath: capture.finalCapturePath,
        width,
        height,
        produce: (temporaryCapturePath) =>
          captureNativeViewport(
            width,
            height,
            temporaryCapturePath,
            rendererEnvironment,
            signal,
          ),
      });
      onCapturePublished(capture.finalCapturePath);
    }
  } catch (error) {
    captureFailure = error;
    throw error;
  } finally {
    try {
      await session.close(captureFailure);
    } finally {
      control?.destroy();
      if (controlServer.listening) {
        await new Promise((resolve) => controlServer.close(resolve));
      }
    }
  }
}

function buildRendererEnvironment(
  environment,
  controlSocketPath,
  { viewport, typography, diagnostics },
) {
  const rendererEnvironment = {
    ...environment,
    ROONSCAPE_CAPTURE_CONTROL: controlSocketPath,
    ROONSCAPE_CAPTURE_VIEWPORT: viewport,
    ROONSCAPE_DIAGNOSTICS: diagnostics ? "1" : "0",
    ROONSCAPE_STATIC_FIXTURE: "1",
  };
  if (typography !== "automatic") {
    rendererEnvironment.ROONSCAPE_CAPTURE_TYPOGRAPHY = typography;
  }
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

async function captureNativeViewport(
  width,
  height,
  capturePath,
  environment,
  signal,
) {
  await runMonitoredProcess(
    "scrot",
    ["--autoselect", `0,0,${width},${height}`, "--overwrite", capturePath],
    {
      cwd: repositoryRoot,
      environment,
      timeoutMilliseconds: presentationCaptureEncodingTimeoutMilliseconds,
      signal,
    },
  );
}
