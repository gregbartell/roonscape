import { spawn } from "node:child_process";
import { access } from "node:fs/promises";

const diagnosticTailBytes = 64 * 1024;
const truncationMarker = "[... earlier output truncated ...]\n";

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
  const captureStandardOutput = captureTail((output) => {
    child.capturedStandardOutput = output;
  });
  const captureStandardError = captureTail((output) => {
    child.capturedStandardError = output;
  });
  child.spawned = new Promise((resolve, reject) => {
    child.once("spawn", resolve);
    child.once("error", reject);
  });
  child.on("error", (error) => {
    child.capturedError = error;
  });
  child.stdout.on("data", captureStandardOutput);
  child.stderr.on("data", captureStandardError);
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
    await failAfterStopping(
      new Error(`timed out waiting for ${description}`),
      child,
      { description },
    );
  }

  const [exitCode, signal] = outcome.value;
  if (exitCode !== 0) {
    throw processFailure(description, child, exitCode, signal);
  }
  return child.capturedStandardOutput;
}

export async function stopProcess(
  child,
  { description, graceMilliseconds = 2_000, killMilliseconds = 2_000 } = {},
) {
  if (
    child === undefined ||
    processHasExited(child) ||
    child.capturedError !== undefined
  ) {
    return;
  }

  const escalationStarted = Date.now();
  if (await stopWithSignal(child, "SIGTERM", graceMilliseconds)) {
    return;
  }
  if (processHasExited(child)) {
    return;
  }

  if (
    (await stopWithSignal(child, "SIGKILL", killMilliseconds)) ||
    processHasExited(child)
  ) {
    return;
  }

  const elapsedMilliseconds = Date.now() - escalationStarted;
  const command = child.spawnfile ?? "monitored process";
  const processDescription =
    description === undefined || description === command
      ? command
      : `${description} (${command})`;
  throw new Error(
    `failed to clean up ${processDescription}: no exit after SIGTERM and SIGKILL (${elapsedMilliseconds} ms elapsed)`,
  );
}

export async function stopProcesses(
  children,
  { failure, ...stopOptions } = {},
) {
  const results = await Promise.allSettled(
    children.map((child) => stopProcess(child, stopOptions)),
  );
  const cleanupFailures = results.flatMap((result) =>
    result.status === "rejected" ? [result.reason] : [],
  );
  if (cleanupFailures.length === 0) {
    return;
  }

  throw new AggregateError(
    failure === undefined ? cleanupFailures : [failure, ...cleanupFailures],
    failure === undefined
      ? "failed to clean up monitored processes"
      : errorMessage(failure),
    { cause: cleanupFailures[0] },
  );
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
    await failAfterStopping(error, xvfb, { description });
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
    const exited = () => {
      clearTimeout(timeout);
      resolve(true);
    };
    const timeout = setTimeout(() => {
      child.off("exit", exited);
      resolve(false);
    }, milliseconds);
    child.once("exit", exited);
    if (processHasExited(child)) {
      child.off("exit", exited);
      clearTimeout(timeout);
      resolve(true);
      return;
    }
    child.kill(signal);
  });
}

function processHasExited(child) {
  return child.exitCode !== null || child.signalCode !== null;
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

async function failAfterStopping(failure, child, stopOptions) {
  await stopProcesses([child], { failure, ...stopOptions });
  throw failure;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function captureTail(update) {
  let tail = Buffer.alloc(0);
  let truncated = false;

  return (chunk) => {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    if (tail.length + bytes.length > diagnosticTailBytes) {
      truncated = true;
      tail =
        bytes.length >= diagnosticTailBytes
          ? bytes.subarray(bytes.length - diagnosticTailBytes)
          : Buffer.concat(
              [
                tail.subarray(tail.length + bytes.length - diagnosticTailBytes),
                bytes,
              ],
              diagnosticTailBytes,
            );
    } else {
      tail = Buffer.concat([tail, bytes], tail.length + bytes.length);
    }
    update(`${truncated ? truncationMarker : ""}${tail.toString("utf8")}`);
  };
}
