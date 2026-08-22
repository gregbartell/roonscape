import assert from "node:assert/strict";
import { once } from "node:events";
import test from "node:test";

import {
  assertProcessRunning,
  availableXDisplayNumber,
  runMonitoredProcess,
  startMonitoredProcess,
  stopProcess,
  waitFor,
} from "./process-harness.mjs";

test("allocates an X display only when its socket and lock are absent", async () => {
  const occupied = new Set(["/tmp/.X11-unix/X90", "/tmp/.X91-lock"]);

  assert.equal(
    await availableXDisplayNumber({
      first: 90,
      exclusiveLimit: 93,
      pathExists: async (filePath) => occupied.has(filePath),
    }),
    92,
  );
});

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
    'process.stdout.write("out\\n"); process.stderr.write("err\\n"); process.exit(7);',
  ]);
  await child.spawned;
  await once(child, "close");

  assert.throws(
    () => assertProcessRunning(child, "test child"),
    /test child exited with 7\nstandard output:\nout\nstandard error:\nerr/,
  );
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

test("escalates termination without waiting indefinitely", async () => {
  const child = startMonitoredProcess(process.execPath, [
    "--eval",
    'process.on("SIGTERM", () => {}); process.stdout.write("ready\\n"); setInterval(() => {}, 1000);',
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
