import { access, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { readFixtureScenarioCatalog } from "../src/bridge/dist/src/fixture-scenario-catalog.js";
import { createNativeSession, waitForNativeWindow } from "./native-session.mjs";
import {
  processCancellation,
  processFailure,
  startMonitoredProcess,
  stopProcesses,
  waitFor,
  waitForProcessExit,
} from "./process-harness.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const options = new Map();
const arguments_ = process.argv.slice(2);
for (let index = 0; index < arguments_.length; index += 1) {
  const option = arguments_[index];
  if (
    ![
      "--release",
      "--static",
      "--headless",
      "--scenario",
      "--resolution",
    ].includes(option)
  ) {
    throw new Error(`unknown fixture option: ${option}`);
  }
  if (options.has(option))
    throw new Error(`duplicate fixture option: ${option}`);
  let value = true;
  if (["--scenario", "--resolution"].includes(option)) {
    value = arguments_[++index];
    if (value === undefined || value.startsWith("--"))
      throw new Error(`${option} requires a value`);
  }
  options.set(option, value);
}
const headless = options.has("--headless");
if (options.has("--resolution") && !headless)
  throw new Error("--resolution requires --headless");
const viewport = options.get("--resolution") ?? "1600x900";
const dimensions = viewport.match(/^(\d+)x(\d+)$/);
const width = Number(dimensions?.[1]);
const height = Number(dimensions?.[2]);
if (
  !dimensions ||
  width < 1280 ||
  height < 720 ||
  width <= height ||
  width > 32767 ||
  height > 32767
) {
  throw new Error(
    "--resolution requires landscape WIDTHxHEIGHT, at least 1280x720 and at most 32767 per dimension",
  );
}
let fixture;
if (options.has("--scenario")) {
  fixture = readFixtureScenarioCatalog().find(
    ({ scenario }) => scenario === options.get("--scenario"),
  )?.fixture;
  if (fixture === undefined)
    throw new Error(`unknown Fixture Scenario: ${options.get("--scenario")}`);
}
const cancellation = processCancellation();
const { signal } = cancellation;
let session;
let runtimeDirectory;
const children = [];
let failure;
try {
  if (headless) session = await createNativeSession({ width, height, signal });
  runtimeDirectory =
    session?.runtimeDirectory ??
    (await mkdtemp(path.join(tmpdir(), "roonscape-fixture.")));
  const socketPath = path.join(runtimeDirectory, "roonscape.sock");
  const controlSocketPath = path.join(
    runtimeDirectory,
    "fixture-navigation.sock",
  );
  const environment = {
    ...(session?.environment ?? process.env),
    ROONSCAPE_SOCKET: socketPath,
  };
  if (fixture !== undefined) environment.ROONSCAPE_FIXTURE = fixture;
  if (options.has("--static")) environment.ROONSCAPE_STATIC_FIXTURE = "1";
  else delete environment.ROONSCAPE_STATIC_FIXTURE;
  if (environment.ROONSCAPE_FIXTURE !== undefined)
    delete environment.ROONSCAPE_FIXTURE_CONTROL;
  else environment.ROONSCAPE_FIXTURE_CONTROL = controlSocketPath;
  if (headless) environment.ROONSCAPE_CAPTURE_VIEWPORT = viewport;

  const start = (command, arguments_) => {
    signal.throwIfAborted();
    const child = session
      ? session.startProcess(command, arguments_, environment)
      : startMonitoredProcess(command, arguments_, {
          cwd: repositoryRoot,
          environment,
        });
    children.push(child);
    child.stdout.pipe(process.stdout);
    child.stderr.pipe(process.stderr);
    return child;
  };
  const publisher = start(process.execPath, [
    path.join(repositoryRoot, "src/bridge/dist/src/fixture.js"),
  ]);
  await publisher.spawned;
  await waitFor(
    () => access(socketPath),
    publisher,
    "Fixture publisher socket",
    { signal },
  );
  if (environment.ROONSCAPE_FIXTURE_CONTROL !== undefined) {
    await waitFor(
      () => access(controlSocketPath),
      publisher,
      "Fixture Mode navigation",
      { signal },
    );
  }
  const renderer = start(
    path.join(
      repositoryRoot,
      "target",
      options.has("--release") ? "release" : "debug",
      "roonscape-renderer",
    ),
    session ? ["--config", session.configurationPath] : [],
  );
  await renderer.spawned;
  if (headless) {
    await waitForNativeWindow(renderer, environment, width, height, { signal });
    process.stdout.write(
      `Headless Fixture Mode ready at ${viewport}; DISPLAY=${environment.DISPLAY}; runtime=${runtimeDirectory}\n`,
    );
  }
  // Session duration is intentionally interactive; only startup and cleanup
  // have deadlines. Publisher failure also terminates the Renderer.
  const outcome = await Promise.race([
    waitForProcessExit(renderer, { signal }),
    waitForProcessExit(publisher, { signal }).then(([code, exitSignal]) => {
      throw processFailure("Fixture publisher", publisher, code, exitSignal);
    }),
  ]);
  process.exitCode = outcome[1] === null ? (outcome[0] ?? 1) : 1;
} catch (error) {
  failure = error;
  console.error(error);
  process.exitCode = signal.aborted ? 130 : 1;
} finally {
  try {
    if (session) await session.close(failure);
    else {
      await stopProcesses(children, { failure, signal });
      if (runtimeDirectory)
        await rm(runtimeDirectory, { recursive: true, force: true });
    }
  } finally {
    cancellation.dispose();
  }
}
