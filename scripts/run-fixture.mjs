import { once } from "node:events";
import { access, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { spawn } from "node:child_process";
import path from "node:path";

const arguments_ = process.argv.slice(2);
const supportedOptions = new Set(["--release", "--static"]);
const unknownOption = arguments_.find(
  (option) => !supportedOptions.has(option),
);
if (unknownOption !== undefined) {
  throw new Error(`unknown fixture option: ${unknownOption}`);
}
if (new Set(arguments_).size !== arguments_.length) {
  throw new Error(
    `duplicate fixture option: ${arguments_.find((option, index) => arguments_.indexOf(option) !== index)}`,
  );
}
const releaseRenderer = arguments_.includes("--release");
const staticFixture = arguments_.includes("--static");

const runtimeDirectory = await mkdtemp(
  path.join(tmpdir(), "roonscape-fixture."),
);
const socketPath = path.join(runtimeDirectory, "roonscape.sock");
const controlSocketPath = path.join(
  runtimeDirectory,
  "fixture-navigation.sock",
);
const environment = { ...process.env, ROONSCAPE_SOCKET: socketPath };
if (staticFixture) {
  environment.ROONSCAPE_STATIC_FIXTURE = "1";
} else {
  delete environment.ROONSCAPE_STATIC_FIXTURE;
}
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
