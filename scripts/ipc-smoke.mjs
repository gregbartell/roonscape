import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const scratchRoot = "/tmp/codex/roonscape";
await mkdir(scratchRoot, { recursive: true });
const runtimeDirectory = await mkdtemp(path.join(scratchRoot, "ipc-smoke."));
const socketPath = path.join(runtimeDirectory, "roonscape.sock");
const environment = { ...process.env, ROONSCAPE_SOCKET: socketPath };
const children = new Set();

try {
  const rendererFirst = startProbe("renderer-first");
  await rendererFirst.output.next(
    (event) =>
      event.event === "connection" &&
      event.connection === "disconnected" &&
      event.presentation === "disconnected",
    "renderer-first disconnected presentation",
  );

  const firstBridge = startBridge(
    "src/shared/fixtures/playing.json",
    "bridge-initial",
  );
  await firstBridge.output.next(
    (line) => line.includes("Fixture publisher listening"),
    "initial bridge listener",
  );
  await rendererFirst.output.next(
    (event) => event.event === "connection" && event.connection === "connected",
    "renderer-first connection",
  );
  await rendererFirst.output.next(
    (event) =>
      event.event === "snapshot" &&
      event.availability === "available" &&
      event.revision === 7,
    "initial current-state replay",
  );

  await stop(firstBridge.child, "SIGKILL");
  await rendererFirst.output.next(
    (event) =>
      event.event === "connection" &&
      event.connection === "disconnected" &&
      event.presentation === "disconnected",
    "stale Now Playing clearing",
  );

  const restartedBridge = startBridge(
    "src/shared/fixtures/paused.json",
    "bridge-restarted",
  );
  await restartedBridge.output.next(
    (line) => line.includes("Fixture publisher listening"),
    "restarted bridge listener",
  );
  await rendererFirst.output.next(
    (event) => event.event === "connection" && event.connection === "connected",
    "bridge restart reconnection",
  );
  await rendererFirst.output.next(
    (event) =>
      event.event === "snapshot" &&
      event.availability === "available" &&
      event.revision === 8,
    "bridge restart current-state replay",
  );

  await stop(rendererFirst.child, "SIGTERM");
  const bridgeFirst = startProbe("bridge-first");
  await bridgeFirst.output.next(
    (event) =>
      event.event === "connection" && event.connection === "disconnected",
    "bridge-first renderer startup",
  );
  await bridgeFirst.output.next(
    (event) => event.event === "connection" && event.connection === "connected",
    "renderer restart connection",
  );
  await bridgeFirst.output.next(
    (event) =>
      event.event === "snapshot" &&
      event.availability === "available" &&
      event.revision === 8,
    "renderer restart current-state replay",
  );

  await stop(bridgeFirst.child, "SIGTERM");
  await stop(restartedBridge.child, "SIGTERM");
  process.stdout.write(
    "IPC smoke check passed: both startup orders, both process restarts, reconnect, replay, and stale-content clearing.\n",
  );
} finally {
  await Promise.all([...children].map((child) => stop(child, "SIGKILL")));
  await rm(runtimeDirectory, { recursive: true });
}

function startProbe(label) {
  const child = startChild(
    label,
    path.join(repositoryRoot, "target/debug/examples/ipc-probe"),
    [],
    environment,
  );
  return { child, output: jsonOutput(child, label) };
}

function startBridge(fixture, label) {
  const child = startChild(
    label,
    process.execPath,
    [path.join(repositoryRoot, "src/bridge/dist/src/fixture.js")],
    { ...environment, ROONSCAPE_FIXTURE: fixture },
  );
  return { child, output: lineOutput(child, label) };
}

function startChild(label, command, arguments_, env) {
  const child = spawn(command, arguments_, {
    cwd: repositoryRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  children.add(child);
  child.once("close", () => children.delete(child));
  child.stderr.on("data", (chunk) => {
    process.stderr.write(`[${label}] ${chunk}`);
  });
  return child;
}

function lineOutput(child, label) {
  return monitoredOutput(child, label, (line) => line);
}

function jsonOutput(child, label) {
  return monitoredOutput(child, label, (line) => JSON.parse(line));
}

function monitoredOutput(child, label, parse) {
  const values = [];
  const waiters = [];
  const lines = createInterface({ input: child.stdout });
  lines.on("line", (line) => {
    let value;
    try {
      value = parse(line);
    } catch (error) {
      rejectWaiters(error);
      return;
    }
    values.push(value);
    settleWaiters();
  });
  child.once("error", rejectWaiters);
  child.once("close", (code, signal) => {
    if (waiters.length > 0) {
      rejectWaiters(
        new Error(`${label} exited before expected output (${code ?? signal})`),
      );
    }
  });

  return {
    next(predicate, description) {
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          const index = waiters.indexOf(waiter);
          if (index !== -1) {
            waiters.splice(index, 1);
          }
          reject(new Error(`Timed out waiting for ${description}`));
        }, 5_000);
        const waiter = { predicate, resolve, reject, timer };
        waiters.push(waiter);
        settleWaiters();
      });
    },
  };

  function settleWaiters() {
    while (waiters.length > 0) {
      const waiter = waiters[0];
      const index = values.findIndex(waiter.predicate);
      if (index === -1) {
        return;
      }
      const [value] = values.splice(index, 1);
      waiters.shift();
      clearTimeout(waiter.timer);
      waiter.resolve(value);
    }
  }

  function rejectWaiters(error) {
    for (const waiter of waiters.splice(0)) {
      clearTimeout(waiter.timer);
      waiter.reject(error);
    }
  }
}

async function stop(child, signal) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const closed = once(child, "close");
  child.kill(signal);
  await closed;
}
