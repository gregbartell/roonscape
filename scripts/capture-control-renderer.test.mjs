import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { tmpdir } from "node:os";
import { createInterface } from "node:readline";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  startMonitoredProcess,
  waitFor,
  waitForProcessExit,
  stopProcess,
} from "./process-harness.mjs";

import { createNativeSession } from "./native-session.mjs";

const executeFile = promisify(execFile);
const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const rendererPath = path.join(
  repositoryRoot,
  "target/debug/roonscape-renderer",
);

test("acknowledges initial and repeated Fixture Scenario revisions only after their exact frame paints", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-capture-control-test."),
  );
  const controlSocketPath = path.join(taskDirectory, "capture-control.sock");
  const validArtworkPath = path.join(taskDirectory, "valid-artwork.unknown");
  const invalidArtworkPath = path.join(taskDirectory, "invalid-artwork.png");
  await writeFile(
    validArtworkPath,
    await readFile(
      path.join(repositoryRoot, "src/shared/fixtures/artwork/playing.svg"),
    ),
  );
  await writeFile(invalidArtworkPath, "not an image");
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(controlSocketPath, resolve);
  });
  const nativeSession = await createNativeSession({ width: 1280, height: 720 });
  const display = nativeSession.environment.DISPLAY;
  let renderer;

  try {
    const connected = once(server, "connection");
    renderer = startMonitoredProcess(
      rendererPath,
      ["--config", path.join(taskDirectory, "display.json")],
      {
        cwd: repositoryRoot,
        environment: {
          ...nativeSession.environment,
          DISPLAY: display,
          GDK_BACKEND: "x11",
          NO_AT_BRIDGE: "1",
          ROONSCAPE_CAPTURE_CONTROL: controlSocketPath,
          ROONSCAPE_CAPTURE_VIEWPORT: "1280x720",
          ROONSCAPE_STATIC_FIXTURE: "1",
        },
      },
    );
    await renderer.spawned;
    const [control] = await waitFor(
      () => connected,
      renderer,
      "capture control connection",
    );
    const acknowledgements = createInterface({ input: control })[
      Symbol.asyncIterator
    ]();

    control.write(
      `${JSON.stringify(await selection("output-unavailable.json", "output-unavailable", 71))}\n`,
    );
    assert.deepEqual(await nextAcknowledgement(acknowledgements, renderer), {
      type: "painted",
      scenario: "output-unavailable",
      revision: 71,
    });
    assert.deepEqual(await windowDimensions(display), {
      width: 1280,
      height: 720,
    });

    control.write(
      [
        JSON.stringify(await selection("stopped.json", "idle", 72)),
        JSON.stringify(await selection("playing.json", "playing", 73)),
        "",
      ].join("\n"),
    );
    assert.deepEqual(await nextAcknowledgement(acknowledgements, renderer), {
      type: "painted",
      scenario: "playing",
      revision: 73,
    });

    control.write(
      `${JSON.stringify(await selection("playing.json", "playing", 74, validArtworkPath))}\n`,
    );
    assert.deepEqual(await nextAcknowledgement(acknowledgements, renderer), {
      type: "painted",
      scenario: "playing",
      revision: 74,
    });

    control.write(
      `${JSON.stringify(await selection("playing.json", "playing", 75, invalidArtworkPath))}\n`,
    );
    const closed = waitForProcessExit(renderer, {
      timeoutMilliseconds: 5000,
    }).then(([exitCode, signal]) => ({
      type: "closed",
      exitCode,
      signal,
    }));
    const acknowledged = acknowledgements.next().then((next) =>
      next.done
        ? closed
        : {
            type: "acknowledged",
            acknowledgement: JSON.parse(next.value),
          },
    );
    const outcome = await Promise.race([closed, acknowledged]);
    assert.deepEqual(outcome, {
      type: "closed",
      exitCode: 1,
      signal: null,
    });
    const { exitCode, signal } = outcome;
    assert.equal(signal, null);
    assert.equal(exitCode, 1);
    assert.match(
      renderer.capturedStandardError,
      new RegExp(
        `could not decode or derive a palette from artwork at ${invalidArtworkPath}`,
      ),
    );
  } finally {
    await stopProcess(renderer);
    await nativeSession.close();
    await new Promise((resolve) => server.close(resolve));
    await rm(taskDirectory, { recursive: true });
  }
});

async function selection(fixtureName, scenario, revision, artworkPath) {
  const snapshot = JSON.parse(
    await readFile(
      path.join(repositoryRoot, "src/shared/fixtures", fixtureName),
      "utf8",
    ),
  );
  snapshot.revision = revision;
  if (artworkPath !== undefined) {
    snapshot.artwork = { revision: 1, path: artworkPath };
  }
  return { type: "select", scenario, revision, snapshot };
}

async function nextAcknowledgement(acknowledgements, renderer) {
  const next = await waitFor(
    () => acknowledgements.next(),
    renderer,
    "painted revision acknowledgement",
  );
  assert.equal(next.done, false, renderer.capturedStandardError);
  return JSON.parse(next.value);
}

async function windowDimensions(display) {
  const { stdout } = await executeFile("xwininfo", ["-name", "RoonScape"], {
    env: { ...process.env, DISPLAY: display },
    timeout: 1_000,
  });
  return {
    width: Number.parseInt(
      stdout.match(/^ {2}Width:\s+(\d+)$/m)?.[1] ?? "",
      10,
    ),
    height: Number.parseInt(
      stdout.match(/^ {2}Height:\s+(\d+)$/m)?.[1] ?? "",
      10,
    ),
  };
}
