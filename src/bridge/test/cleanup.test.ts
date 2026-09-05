import assert from "node:assert/strict";
import test from "node:test";

import { attemptAllCleanup } from "../src/cleanup.js";

test("cleanup attempts every step and preserves synchronous and asynchronous failures", async () => {
  const events: string[] = [];
  const discoveryFailure = new Error("discovery remained active");
  const socketFailure = new Error("socket remained open");

  await assert.rejects(
    attemptAllCleanup("Could not stop RoonScape", [
      () => {
        events.push("discovery stopped");
        throw discoveryFailure;
      },
      async () => {
        events.push("publisher closed");
        throw socketFailure;
      },
      () => {
        events.push("artwork removed");
      },
    ]),
    (error) => {
      assert.ok(error instanceof AggregateError);
      assert.equal(error.message, "Could not stop RoonScape");
      assert.deepEqual(error.errors, [discoveryFailure, socketFailure]);
      return true;
    },
  );
  assert.deepEqual(events, [
    "discovery stopped",
    "publisher closed",
    "artwork removed",
  ]);
});
