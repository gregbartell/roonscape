import assert from "node:assert/strict";
import test from "node:test";

import { createFixtureModeSession } from "../src/fixture-mode-session.js";

test("Fixture Mode session close attempts control-server and publisher cleanup", async () => {
  const events: string[] = [];
  const session = createFixtureModeSession({
    disconnectControlClients: () => events.push("clients disconnected"),
    closeControlServer: async () => {
      events.push("control server closed");
      throw new Error("control server cleanup failed");
    },
    closePublisher: async () => {
      events.push("publisher closed");
      throw new Error("publisher cleanup failed");
    },
  });

  await assert.rejects(session.close(), (error) => {
    assert.ok(error instanceof AggregateError);
    assert.deepEqual(
      error.errors.map((failure: unknown) =>
        failure instanceof Error ? failure.message : String(failure),
      ),
      ["control server cleanup failed", "publisher cleanup failed"],
    );
    return true;
  });
  assert.deepEqual(events, [
    "clients disconnected",
    "control server closed",
    "publisher closed",
  ]);
});
