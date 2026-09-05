import { spawn } from "node:child_process";

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
    detached: true,
  });
  child.ownsProcessGroup = true;
  child.completed = new Promise((resolve) => {
    child.once("close", (...outcome) => resolve(outcome));
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
  // Callers may cancel before awaiting startup; retain the error without an
  // unhandled rejection while their finally block terminates owned children.
  void child.spawned.catch(() => {});
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
  { retryMilliseconds = 50, timeoutMilliseconds = 5_000, signal } = {},
) {
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (true) {
    signal?.throwIfAborted();
    const remainingMilliseconds = deadline - Date.now();
    if (remainingMilliseconds <= 0) {
      break;
    }
    assertProcessRunning(child, description);
    const attempt = await settleBefore(
      Promise.resolve().then(action),
      remainingMilliseconds,
      signal,
    );
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
  const details = processDetails(child);
  throw new Error(
    `timed out waiting for ${description}${details ? `\n${details}` : ""}`,
    {
      cause: lastError,
    },
  );
}

export async function runMonitoredProcess(
  command,
  arguments_,
  {
    cwd,
    environment = process.env,
    description = command,
    timeoutMilliseconds = 5_000,
    signal,
  } = {},
) {
  const child = startMonitoredProcess(command, arguments_, {
    cwd,
    environment,
  });
  let outcome;
  try {
    await child.spawned;
    outcome = await settleBefore(child.completed, timeoutMilliseconds, signal);
    if (outcome.kind === "timed-out") {
      throw new Error(`timed out waiting for ${description}`);
    }
  } catch (error) {
    await failAfterStopping(error, child, { description, signal });
  }

  await stopProcess(child);
  const [exitCode, exitSignal] = outcome.value;
  if (exitCode !== 0) {
    throw processFailure(description, child, exitCode, exitSignal);
  }
  return child.capturedStandardOutput;
}

export async function stopProcess(child, options = {}) {
  try {
    await stopOwnedProcess(child, options);
  } finally {
    // A wrapper can exit before its children or leave inherited pipes open.
    // Signal only the process group created for this monitored command.
    if (child?.ownsProcessGroup && child.pid !== undefined)
      signalProcess(child, "SIGKILL");
  }
}

async function stopOwnedProcess(
  child,
  {
    description,
    signal,
    // Leave time for the owning CLI to finish cleanup before its monitor's
    // ordinary two-second grace period expires.
    graceMilliseconds = signal?.aborted ? 250 : 2_000,
    killMilliseconds = signal?.aborted ? 250 : 2_000,
  } = {},
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

export async function startXvfbDisplay({
  width,
  height,
  depth = 24,
  cwd,
  environment = process.env,
  description = "Xvfb display",
  retryMilliseconds = 25,
  timeoutMilliseconds = 5_000,
  signal,
} = {}) {
  // Xvfb allocates and locks its display atomically, then reports it only
  // after initialization. Never infer ownership from a neighboring socket.
  const xvfb = startMonitoredProcess(
    "Xvfb",
    [
      "-displayfd",
      "1",
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
    const display = await waitFor(
      () => {
        const match = xvfb.capturedStandardOutput.match(/^(\d+)\n/);
        if (match === null)
          throw new Error("Xvfb has not reported its allocated display");
        return `:${match[1]}`;
      },
      xvfb,
      description,
      { retryMilliseconds, timeoutMilliseconds, signal },
    );
    return { display, xvfb };
  } catch (error) {
    await failAfterStopping(error, xvfb, { description, signal });
  }
}

function processDetails(child) {
  const output = (child.capturedStandardOutput ?? "").trim();
  const error = (child.capturedStandardError ?? "").trim();
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
    signalProcess(child, signal);
  });
}

function signalProcess(child, signal) {
  if (!child.ownsProcessGroup) {
    child.kill(signal);
    return;
  }
  try {
    process.kill(-child.pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

function processHasExited(child) {
  return child.exitCode !== null || child.signalCode !== null;
}

export async function waitForProcessExit(
  child,
  { signal, timeoutMilliseconds } = {},
) {
  const outcome = await settleBefore(
    child.completed,
    timeoutMilliseconds,
    signal,
  );
  if (outcome.kind === "timed-out") {
    throw new Error(`timed out waiting for ${child.spawnfile} to exit`);
  }
  return outcome.value;
}

export function processCancellation() {
  const controller = new AbortController();
  const cancel = (signal) =>
    controller.abort(new Error(`cancelled by ${signal}`));
  const interrupt = () => cancel("SIGINT");
  const terminate = () => cancel("SIGTERM");
  process.on("SIGINT", interrupt);
  process.on("SIGTERM", terminate);
  return {
    signal: controller.signal,
    dispose() {
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", terminate);
    },
  };
}

function settleBefore(promise, milliseconds, signal) {
  return new Promise((resolve, reject) => {
    let timeout;
    const finish = (outcome) => {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", abort);
      resolve(outcome);
    };
    const abort = () => {
      clearTimeout(timeout);
      signal.removeEventListener("abort", abort);
      reject(signal.reason);
    };
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) {
      abort();
    } else if (milliseconds !== undefined) {
      timeout = setTimeout(() => finish({ kind: "timed-out" }), milliseconds);
    }
    promise.then(
      (value) => finish({ kind: "completed", value }),
      (error) => finish({ kind: "failed", error }),
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
