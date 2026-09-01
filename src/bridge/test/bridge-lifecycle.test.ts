import assert from "node:assert/strict";
import test from "node:test";

import {
  installBridgeLifecycle,
  shutdownBridge,
} from "../src/bridge-lifecycle.js";

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  test(`${signal} closes the bridge and publisher before exiting successfully`, async () => {
    const events: string[] = [];
    const handlers = new Map<NodeJS.Signals, () => void | Promise<void>>();
    installBridgeLifecycle({
      bridge: {
        currentSnapshot: () => {
          throw new Error("unused");
        },
        stop: async () => {
          events.push("bridge stopped");
        },
      },
      publisher: {
        publish: () => undefined,
        close: async () => {
          events.push("publisher closed");
        },
      },
      once: (observedSignal, handler) => {
        handlers.set(observedSignal, handler);
      },
      reportError: (message) => {
        events.push(`stderr: ${message}`);
      },
      exit: (code) => events.push(`exit: ${code}`),
    });

    await handlers.get(signal)?.();

    assert.deepEqual(events, ["bridge stopped", "publisher closed", "exit: 0"]);
  });
}

test("bridge shutdown attempts every cleanup and preserves every failure", async () => {
  const events: string[] = [];

  await assert.rejects(
    shutdownBridge(
      {
        currentSnapshot: () => {
          throw new Error("unused");
        },
        stop: async () => {
          events.push("bridge stopped");
          throw new Error("Roon services remained open");
        },
      },
      {
        publish: () => undefined,
        close: async () => {
          events.push("publisher closed");
          throw new Error("snapshot socket remained open");
        },
      },
    ),
    (error) => {
      assert.ok(error instanceof AggregateError);
      assert.deepEqual(
        error.errors.map((failure: unknown) =>
          failure instanceof Error ? failure.message : String(failure),
        ),
        ["Roon services remained open", "snapshot socket remained open"],
      );
      return true;
    },
  );
  assert.deepEqual(events, ["bridge stopped", "publisher closed"]);
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  test(`${signal} reports every bridge cleanup failure and exits nonzero`, async () => {
    const diagnostics: string[] = [];
    const lifecycleEvents: string[] = [];
    const handlers = new Map<NodeJS.Signals, () => void | Promise<void>>();
    let exitCode: number | undefined;
    installBridgeLifecycle({
      bridge: {
        currentSnapshot: () => {
          throw new Error("unused");
        },
        stop: async () => {
          throw new AggregateError(
            [
              new Error("Roon discovery remained active"),
              new Error("Roon services remained open"),
              new Error("artwork files remained present"),
            ],
            "Could not stop RoonScape Bridge",
          );
        },
      },
      publisher: {
        publish: () => undefined,
        close: async () => {
          throw new Error("snapshot socket remained open");
        },
      },
      once: (observedSignal, handler) => {
        handlers.set(observedSignal, handler);
      },
      reportError: async (message) => {
        await Promise.resolve();
        diagnostics.push(message);
        lifecycleEvents.push("diagnostic written");
      },
      exit: (code) => {
        exitCode = code;
        lifecycleEvents.push("exited");
      },
    });

    await handlers.get(signal)?.();

    assert.equal(exitCode, 1);
    assert.deepEqual(diagnostics, [
      "Could not stop RoonScape Bridge: Roon discovery remained active",
      "Could not stop RoonScape Bridge: Roon services remained open",
      "Could not stop RoonScape Bridge: artwork files remained present",
      "Could not stop RoonScape Bridge: snapshot socket remained open",
    ]);
    assert.deepEqual(lifecycleEvents, [
      "diagnostic written",
      "diagnostic written",
      "diagnostic written",
      "diagnostic written",
      "exited",
    ]);
  });
}
