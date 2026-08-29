import { once } from "node:events";
import { access, mkdir, mkdtemp, rm } from "node:fs/promises";
import { spawn } from "node:child_process";
import path from "node:path";

const arguments_ = process.argv.slice(2);
if (
  arguments_.length > 1 ||
  (arguments_[0] !== undefined && arguments_[0] !== "--release")
) {
  throw new Error(`unknown fixture option: ${arguments_[0]}`);
}
const releaseRenderer = arguments_[0] === "--release";

const scratchRoot = "/tmp/codex/roonscape";
await mkdir(scratchRoot, { recursive: true });
const runtimeDirectory = await mkdtemp(path.join(scratchRoot, "fixture."));
const socketPath = path.join(runtimeDirectory, "roonscape.sock");
const controlSocketPath = path.join(
  runtimeDirectory,
  "fixture-navigation.sock",
);
const environment = { ...process.env, ROONSCAPE_SOCKET: socketPath };
const explicitFixture = environment.ROONSCAPE_FIXTURE !== undefined;
if (explicitFixture) {
  delete environment.ROONSCAPE_FIXTURE_CONTROL;
} else {
  environment.ROONSCAPE_FIXTURE_CONTROL = controlSocketPath;
}

const publisher = spawn(process.execPath, ["src/bridge/dist/src/fixture.js"], {
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
  await waitForSocket(socketPath, "fixture publisher");
  if (!explicitFixture) {
    await waitForSocket(controlSocketPath, "Fixture Mode navigation");
  }
  renderer = spawn(
    "cargo",
    [
      "run",
      "--quiet",
      ...(releaseRenderer ? ["--release"] : []),
      "--package",
      "roonscape-renderer",
    ],
    { env: environment, stdio: "inherit" },
  );
  const [exitCode, signal] = await once(renderer, "exit");
  await shutdown(signal === null ? (exitCode ?? 1) : 1);
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  await shutdown(1);
}

async function waitForSocket(expectedPath, label) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (publisher.exitCode !== null || publisher.signalCode !== null) {
      throw new Error(
        `Fixture publisher exited before opening the socket (${publisher.exitCode ?? publisher.signalCode})`,
      );
    }

    try {
      await access(expectedPath);
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  }

  throw new Error(`Timed out waiting for ${label}`);
}
