import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { tmpdir } from "node:os";
import { createInterface } from "node:readline";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcess,
} from "./process-harness.mjs";

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
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(controlSocketPath, resolve);
  });
  const { display, xvfb } = await startXvfbDisplay({
    width: 1280,
    height: 720,
    cwd: repositoryRoot,
  });
  let renderer;

  try {
    const connected = once(server, "connection");
    renderer = startMonitoredProcess(
      rendererPath,
      ["--config", path.join(taskDirectory, "display.json")],
      {
        cwd: repositoryRoot,
        environment: {
          ...process.env,
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
    const [control] = await connected;
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
      `${JSON.stringify(await selection("playing.json", "playing", 74))}\n`,
    );
    assert.deepEqual(await nextAcknowledgement(acknowledgements, renderer), {
      type: "painted",
      scenario: "playing",
      revision: 74,
    });

    const closed = once(renderer, "close");
    control.destroy();
    const [exitCode, signal] = await closed;
    assert.equal(signal, null);
    assert.equal(exitCode, 1);
    assert.match(
      renderer.capturedStandardError,
      /capture control channel disconnected/,
    );
  } finally {
    await stopProcess(renderer);
    await stopProcess(xvfb);
    await new Promise((resolve) => server.close(resolve));
    await rm(taskDirectory, { recursive: true });
  }
});

async function selection(fixtureName, scenario, revision) {
  const snapshot = JSON.parse(
    await readFile(
      path.join(repositoryRoot, "src/shared/fixtures", fixtureName),
      "utf8",
    ),
  );
  snapshot.revision = revision;
  return { type: "select", scenario, revision, snapshot };
}

async function nextAcknowledgement(acknowledgements, renderer) {
  const next = await acknowledgements.next();
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
