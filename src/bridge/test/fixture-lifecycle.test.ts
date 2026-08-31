import assert from "node:assert/strict";
import test from "node:test";

import { installFixtureModeLifecycle } from "../src/fixture-lifecycle.js";

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  test(`${signal} closes Fixture Mode and exits successfully`, async () => {
    const handlers = new Map<NodeJS.Signals, () => void | Promise<void>>();
    let closed = false;
    let exitCode: number | undefined;
    installFixtureModeLifecycle({
      fixtureSession: {
        close: async () => {
          closed = true;
        },
      },
      once: (observedSignal, handler) => {
        handlers.set(observedSignal, handler);
      },
      reportError: () => assert.fail("successful shutdown reported an error"),
      exit: (code) => {
        exitCode = code;
      },
    });

    await handlers.get(signal)?.();

    assert.equal(closed, true);
    assert.equal(exitCode, 0);
  });

  test(`${signal} reports every Fixture Mode failure and exits nonzero`, async () => {
    const handlers = new Map<NodeJS.Signals, () => void | Promise<void>>();
    const diagnostics: string[] = [];
    const lifecycleEvents: string[] = [];
    let exitCode: number | undefined;
    installFixtureModeLifecycle({
      fixtureSession: {
        close: async () => {
          throw new AggregateError(
            [
              new Error("control server cleanup failed"),
              new Error("publisher cleanup failed"),
            ],
            "Could not stop Fixture Mode",
          );
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
      "Could not stop Fixture Mode: control server cleanup failed",
      "Could not stop Fixture Mode: publisher cleanup failed",
    ]);
    assert.deepEqual(lifecycleEvents, [
      "diagnostic written",
      "diagnostic written",
      "exited",
    ]);
  });
}
