import { spawn } from "node:child_process";
import { access } from "node:fs/promises";

export function startMonitoredProcess(
  command,
  arguments_,
  { cwd, environment = process.env } = {},
) {
  const child = spawn(command, arguments_, {
    cwd,
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.capturedError = undefined;
  child.capturedStandardOutput = "";
  child.capturedStandardError = "";
  child.spawned = new Promise((resolve, reject) => {
    child.once("spawn", resolve);
    child.once("error", reject);
  });
  child.on("error", (error) => {
    child.capturedError = error;
  });
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    child.capturedStandardOutput += chunk;
  });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    child.capturedStandardError += chunk;
  });
  return child;
}

export function assertProcessRunning(child, description) {
  if (child.capturedError !== undefined) {
    throw child.capturedError;
  }
  if (child.exitCode !== null || child.signalCode !== null) {
    throw processFailure(description, child, child.exitCode, child.signalCode);
  }
}

export function processFailure(description, child, exitCode, signal) {
  const outcome = signal ?? exitCode ?? "unknown status";
  const details = processDetails(child);
  return new Error(
    `${description} exited with ${outcome}${details.length === 0 ? "" : `\n${details}`}`,
  );
}

export async function waitFor(
  action,
  child,
  description,
  { retryMilliseconds = 50, timeoutMilliseconds = 5_000 } = {},
) {
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (true) {
    const remainingMilliseconds = deadline - Date.now();
    if (remainingMilliseconds <= 0) {
      break;
    }
    assertProcessRunning(child, description);
    const attempt = await actionBefore(action, remainingMilliseconds);
    if (attempt.kind === "completed") {
      return attempt.value;
    }
    if (attempt.kind === "timed-out") {
      break;
    }
    lastError = attempt.error;

    const retryDelayMilliseconds = Math.min(
      retryMilliseconds,
      Math.max(0, deadline - Date.now()),
    );
    if (retryDelayMilliseconds > 0) {
      await delay(retryDelayMilliseconds);
    }
  }

  assertProcessRunning(child, description);
  throw new Error(`timed out waiting for ${description}`, {
    cause: lastError,
  });
}

export async function runMonitoredProcess(
  command,
  arguments_,
  {
    cwd,
    environment = process.env,
    description = command,
    timeoutMilliseconds = 5_000,
  } = {},
) {
  const child = startMonitoredProcess(command, arguments_, {
    cwd,
    environment,
  });
  const completed = new Promise((resolve) => {
    child.once("close", (...outcome) => resolve(outcome));
  });
  await child.spawned;
  const outcome = await completionBefore(completed, timeoutMilliseconds);
  if (outcome.kind === "timed-out") {
    await stopProcess(child);
    throw new Error(`timed out waiting for ${description}`);
  }

  const [exitCode, signal] = outcome.value;
  if (exitCode !== 0) {
    throw processFailure(description, child, exitCode, signal);
  }
  return child.capturedStandardOutput;
}

export async function stopProcess(
  child,
  { graceMilliseconds = 2_000, killMilliseconds = 2_000 } = {},
) {
  if (
    child === undefined ||
    child.exitCode !== null ||
    child.signalCode !== null ||
    child.capturedError !== undefined
  ) {
    return;
  }

  if (await stopWithSignal(child, "SIGTERM", graceMilliseconds)) {
    return;
  }
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }

  await stopWithSignal(child, "SIGKILL", killMilliseconds);
}

export async function availableXDisplayNumber({
  first = 90,
  exclusiveLimit = 200,
  pathExists = exists,
} = {}) {
  for (
    let displayNumber = first;
    displayNumber < exclusiveLimit;
    displayNumber += 1
  ) {
    const socket = `/tmp/.X11-unix/X${displayNumber}`;
    const lock = `/tmp/.X${displayNumber}-lock`;
    if (!(await pathExists(socket)) && !(await pathExists(lock))) {
      return displayNumber;
    }
  }
  throw new Error("no free X11 display number is available");
}

export async function startXvfbDisplay({
  width,
  height,
  depth = 24,
  cwd,
  environment = process.env,
  description = "Xvfb display",
  retryMilliseconds = 25,
  timeoutMilliseconds = 5_000,
} = {}) {
  const displayNumber = await availableXDisplayNumber();
  const display = `:${displayNumber}`;
  const displaySocket = `/tmp/.X11-unix/X${displayNumber}`;
  const xvfb = startMonitoredProcess(
    "Xvfb",
    [
      display,
      "-screen",
      "0",
      `${width}x${height}x${depth}`,
      "-nolisten",
      "tcp",
    ],
    { cwd, environment },
  );

  try {
    await xvfb.spawned;
    await waitFor(() => access(displaySocket), xvfb, description, {
      retryMilliseconds,
      timeoutMilliseconds,
    });
    return { display, xvfb };
  } catch (error) {
    await stopProcess(xvfb);
    throw error;
  }
}

async function exists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

function processDetails(child) {
  const output = child.capturedStandardOutput.trim();
  const error = child.capturedStandardError.trim();
  return [
    output.length === 0 ? undefined : `standard output:\n${output}`,
    error.length === 0 ? undefined : `standard error:\n${error}`,
  ]
    .filter((detail) => detail !== undefined)
    .join("\n");
}

function stopWithSignal(child, signal, milliseconds) {
  return new Promise((resolve) => {
    const closed = () => {
      clearTimeout(timeout);
      resolve(true);
    };
    const timeout = setTimeout(() => {
      child.off("close", closed);
      resolve(false);
    }, milliseconds);
    child.once("close", closed);
    child.kill(signal);
  });
}

function actionBefore(action, milliseconds) {
  return settleBefore(Promise.resolve().then(action), milliseconds);
}

function completionBefore(completed, milliseconds) {
  return settleBefore(completed, milliseconds);
}

function settleBefore(promise, milliseconds) {
  return new Promise((resolve) => {
    const timeout = setTimeout(
      () => resolve({ kind: "timed-out" }),
      milliseconds,
    );
    promise.then(
      (value) => {
        clearTimeout(timeout);
        resolve({ kind: "completed", value });
      },
      (error) => {
        clearTimeout(timeout);
        resolve({ kind: "failed", error });
      },
    );
  });
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
