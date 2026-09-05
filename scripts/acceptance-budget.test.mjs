import assert from "node:assert/strict";
import test from "node:test";
import { createAcceptanceBudget } from "./acceptance-budget.mjs";

test("acceptance phases share the remaining budget without extending it", (context) => {
  let now = 0;
  context.mock.method(performance, "now", () => now);
  const budget = createAcceptanceBudget({ timeoutMilliseconds: 1000 });
  try {
    assert.equal(budget.waitOptions(100).timeoutMilliseconds, 100);
    now = 750;
    assert.equal(budget.waitOptions(500).timeoutMilliseconds, 250);
    assert.equal(budget.waitOptions().timeoutMilliseconds, 250);
    now = 1000;
    assert.throws(() => budget.waitOptions(), /work budget exhausted/);
    assert.equal(budget.signal.aborted, true);
  } finally {
    budget.dispose();
  }
});

test("budget expiry aborts an outstanding wait and disposal leaves cleanup time", (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const expired = createAcceptanceBudget({ timeoutMilliseconds: 1000 });
  const disposed = createAcceptanceBudget({ timeoutMilliseconds: 1000 });
  const waiting = expired.waitOptions().signal;
  disposed.dispose();
  context.mock.timers.tick(1000);
  assert.equal(waiting.aborted, true);
  assert.match(waiting.reason.message, /work budget exhausted/);
  assert.equal(disposed.signal.aborted, false);
  expired.dispose();
});

test("user cancellation interrupts the shared budget with its original reason", () => {
  const cancellation = new AbortController();
  const budget = createAcceptanceBudget({ signal: cancellation.signal });
  try {
    const reason = new Error("requested cancellation");
    cancellation.abort(reason);
    assert.equal(budget.signal.reason, reason);
    assert.throws(
      () => budget.waitOptions(),
      (error) => error === reason,
    );
  } finally {
    budget.dispose();
  }
});
