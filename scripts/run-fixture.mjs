import { once } from "node:events";
import { access, mkdir, mkdtemp, rm } from "node:fs/promises";
import { spawn } from "node:child_process";
import path from "node:path";

const scratchRoot = "/tmp/codex/roonscape";
await mkdir(scratchRoot, { recursive: true });
const runtimeDirectory = await mkdtemp(path.join(scratchRoot, "fixture."));
const socketPath = path.join(runtimeDirectory, "roonscape.sock");
const environment = { ...process.env, ROONSCAPE_SOCKET: socketPath };

const publisher = spawn(process.execPath, ["bridge/dist/src/fixture.js"], {
  env: environment,
  stdio: "inherit",
});

let renderer;
let shuttingDown = false;

async function stop(child) {
  if (
    child === undefined ||
    child.exitCode !== null ||
    child.signalCode !== null
  ) {
    return;
  }

  const closed = once(child, "close");
  child.kill("SIGTERM");
  await closed;
}

async function shutdown(exitCode) {
  if (shuttingDown) {
    return;
  }

  shuttingDown = true;
  await stop(renderer);
  await stop(publisher);
  await rm(runtimeDirectory, { force: true, recursive: true });
  process.exitCode = exitCode;
}

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.once(signal, () => {
    void shutdown(0);
  });
}

try {
  await waitForSocket();
  renderer = spawn(
    "cargo",
    ["run", "--quiet", "--package", "roonscape-renderer"],
    { env: environment, stdio: "inherit" },
  );
  const [exitCode, signal] = await once(renderer, "exit");
  await shutdown(signal === null ? (exitCode ?? 1) : 1);
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  await shutdown(1);
}

async function waitForSocket() {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (publisher.exitCode !== null || publisher.signalCode !== null) {
      throw new Error(
        `Fixture publisher exited before opening the socket (${publisher.exitCode ?? publisher.signalCode})`,
      );
    }

    try {
      await access(socketPath);
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  }

  throw new Error("Timed out waiting for the fixture publisher");
}
