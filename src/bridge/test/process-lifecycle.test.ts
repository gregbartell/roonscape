import assert from "node:assert/strict";
import test from "node:test";

import { installProcessLifecycle } from "../src/process-lifecycle.js";

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  test(`${signal} finishes cleanup before exiting successfully`, async () => {
    const events: string[] = [];
    const handlers = new Map<NodeJS.Signals, () => void | Promise<void>>();
    installProcessLifecycle({
      cleanup: async () => {
        await Promise.resolve();
        events.push("cleaned up");
      },
      failureMessage: "Could not stop RoonScape",
      once: (observedSignal, handler) => {
        handlers.set(observedSignal, handler);
      },
      reportError: () => assert.fail("successful shutdown reported an error"),
      exit: (code) => events.push(`exit: ${code}`),
    });

    await handlers.get(signal)?.();

    assert.deepEqual(events, ["cleaned up", "exit: 0"]);
  });

  test(`${signal} reports every nested cleanup failure before exiting nonzero`, async () => {
    const events: string[] = [];
    const handlers = new Map<NodeJS.Signals, () => void | Promise<void>>();
    installProcessLifecycle({
      cleanup: async () => {
        throw new AggregateError(
          [
            new AggregateError(
              [
                new Error("discovery remained active"),
                new Error("services remained open"),
              ],
              "Bridge cleanup failed",
            ),
            new Error("socket remained open"),
          ],
          "Cleanup failed",
        );
      },
      failureMessage: "Could not stop RoonScape",
      once: (observedSignal, handler) => {
        handlers.set(observedSignal, handler);
      },
      reportError: async (message) => {
        await Promise.resolve();
        events.push(message);
      },
      exit: (code) => events.push(`exit: ${code}`),
    });

    await handlers.get(signal)?.();

    assert.deepEqual(events, [
      "Could not stop RoonScape: discovery remained active",
      "Could not stop RoonScape: services remained open",
      "Could not stop RoonScape: socket remained open",
      "exit: 1",
    ]);
  });
}
