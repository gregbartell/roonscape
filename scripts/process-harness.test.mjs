import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import test from "node:test";
import { readFile } from "node:fs/promises";

import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  stopProcess,
  stopProcesses,
  waitFor,
} from "./process-harness.mjs";

test("reports process spawn errors", async () => {
  const child = startMonitoredProcess(
    "roonscape-command-that-does-not-exist",
    [],
  );

  await assert.rejects(child.spawned, { code: "ENOENT" });
  assert.throws(() => assertProcessRunning(child, "missing helper"), {
    code: "ENOENT",
  });
});

test("reports both standard output and standard error on early exit", async () => {
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    'const fs = require("node:fs"); fs.writeSync(1, "out\\n"); fs.writeSync(2, "err\\n"); process.exit(7);',
  ]);
  const closed = once(child, "close");
  await child.spawned;
  await closed;

  assert.throws(
    () => assertProcessRunning(child, "test child"),
    /test child exited with 7\nstandard output:\nout\nstandard error:\nerr/,
  );
});

test("retains only the most recent 64 KiB from each diagnostic stream", async () => {
  const retainedBytes = 64 * 1024;
  const discardedOutput = "discarded output\n";
  const discardedError = "discarded error\n";
  const outputTail = "o".repeat(retainedBytes);
  const errorTail = "e".repeat(retainedBytes);
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    `const fs = require("node:fs"); fs.writeSync(1, ${JSON.stringify(discardedOutput)} + "o".repeat(${retainedBytes})); fs.writeSync(2, ${JSON.stringify(discardedError)} + "e".repeat(${retainedBytes}));`,
  ]);
  const closed = once(child, "close");
  await child.spawned;
  await closed;

  const truncationMarker = "[... earlier output truncated ...]\n";
  assert.equal(
    Buffer.byteLength(child.capturedStandardOutput),
    Buffer.byteLength(truncationMarker) + retainedBytes,
  );
  assert.equal(
    Buffer.byteLength(child.capturedStandardError),
    Buffer.byteLength(truncationMarker) + retainedBytes,
  );
  assert.equal(
    child.capturedStandardOutput.slice(0, truncationMarker.length),
    truncationMarker,
  );
  assert.equal(
    child.capturedStandardError.slice(0, truncationMarker.length),
    truncationMarker,
  );
  assert.equal(
    child.capturedStandardOutput.slice(-outputTail.length),
    outputTail,
  );
  assert.equal(child.capturedStandardError.slice(-errorTail.length), errorTail);
});

test("bounds a readiness probe that never settles", async () => {
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    "setInterval(() => {}, 1000);",
  ]);
  await child.spawned;

  try {
    await assert.rejects(
      waitFor(() => new Promise(() => {}), child, "stalled readiness", {
        timeoutMilliseconds: 25,
      }),
      /timed out waiting for stalled readiness/,
    );
  } finally {
    await stopProcess(child);
  }
});

test("bounds a monitored subprocess that never exits", async () => {
  await assert.rejects(
    runMonitoredProcess(
      process.execPath,
      ["--eval", "setInterval(() => {}, 1000);"],
      {
        description: "stalled subprocess",
        timeoutMilliseconds: 25,
      },
    ),
    /timed out waiting for stalled subprocess/,
  );
});

test("preserves an existing failure together with cleanup failures", async () => {
  const failure = new Error("timed out waiting for stubborn subprocess");
  const { child } = stubbornChild();

  await assert.rejects(
    stopProcesses([child], {
      failure,
      description: "stubborn subprocess",
      graceMilliseconds: 5,
      killMilliseconds: 5,
    }),
    (error) => {
      assert(error instanceof AggregateError);
      assert.equal(error.errors[0], failure);
      assert.match(
        error.errors[1].message,
        /failed to clean up stubborn subprocess \(stubborn-command\): no exit after SIGTERM and SIGKILL/,
      );
      return true;
    },
  );
});

test("escalates termination without waiting indefinitely", async () => {
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    'process.on("SIGTERM", () => {}); require("node:fs").writeSync(1, "ready\\n"); setInterval(() => {}, 1000);',
  ]);
  await child.spawned;
  await waitFor(
    async () => {
      assert.match(child.capturedStandardOutput, /ready/);
    },
    child,
    "test child readiness",
  );

  await stopProcess(child, {
    graceMilliseconds: 25,
    killMilliseconds: 500,
  });

  assert.equal(child.signalCode, "SIGKILL");
});

test("rejects cleanup when a command does not exit after SIGKILL", async () => {
  const { child, signals } = stubbornChild();

  await assert.rejects(
    stopProcess(child, {
      graceMilliseconds: 5,
      killMilliseconds: 5,
    }),
    /failed to clean up stubborn-command: no exit after SIGTERM and SIGKILL \(\d+ ms elapsed\)/,
  );
  assert.deepEqual(signals, ["SIGTERM", "SIGKILL"]);
});

function stubbornChild() {
  const signals = [];
  const child = Object.assign(new EventEmitter(), {
    capturedError: undefined,
    exitCode: null,
    signalCode: null,
    spawnfile: "stubborn-command",
    kill(signal) {
      signals.push(signal);
      queueMicrotask(() => this.emit("close", null, signal));
      return true;
    },
  });
  return { child, signals };
}

test("cancelling a stalled readiness probe rejects promptly", async () => {
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    "setInterval(() => {}, 1000)",
  ]);
  await child.spawned;
  const controller = new AbortController();
  const waiting = waitFor(
    () => new Promise(() => {}),
    child,
    "cancelled readiness",
    { signal: controller.signal },
  );
  controller.abort(new Error("requested cancellation"));
  try {
    await assert.rejects(waiting, /requested cancellation/);
  } finally {
    await stopProcess(child);
  }
});

test("stopping an exited command also terminates its surviving descendants", async () => {
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    `
    const {spawn} = require("node:child_process");
    const descendant = spawn(process.execPath, ["--eval", "process.on('SIGTERM', () => {}); setInterval(() => {}, 1000)"], {stdio: "ignore"});
    console.log(descendant.pid);
    descendant.unref();
  `,
  ]);
  await child.spawned;
  await child.completed;
  const descendantPid = Number(child.capturedStandardOutput.trim());
  try {
    await stopProcess(child);
    // A killed descendant may briefly remain a zombie until the host reaps it.
    await waitFor(
      async () => {
        try {
          const stat = await readFile(`/proc/${descendantPid}/stat`, "utf8");
          assert.match(stat, /\) Z /);
        } catch (error) {
          if (error.code !== "ENOENT") throw error;
        }
      },
      { exitCode: null, signalCode: null },
      "descendant termination",
      { timeoutMilliseconds: 500 },
    );
  } finally {
    try {
      process.kill(descendantPid, "SIGKILL");
    } catch (error) {
      assert.equal(error.code, "ESRCH");
    }
  }
});
