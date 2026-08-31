import assert from "node:assert/strict";
import test from "node:test";

import {
  observeLinuxProcessIdentity,
  parseLinuxProcessStartTimeTicks,
} from "../src/process-identity.js";

test("process stat parsing accepts names containing spaces and parentheses", () => {
  const stat =
    "1234 (Roon (Bridge) Worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20";

  assert.equal(parseLinuxProcessStartTimeTicks(stat), "987654");
});

test("Linux process identity observes the current process", () => {
  const observation = observeLinuxProcessIdentity(process.pid);

  assert.equal(observation.status, "observed");
  if (observation.status === "observed") {
    assert.match(observation.processStartTimeTicks, /^\d+$/);
  }
});

test("Linux process identity distinguishes a missing process", () => {
  assert.deepEqual(observeLinuxProcessIdentity(Number.MAX_SAFE_INTEGER), {
    status: "absent",
  });
});

test("malformed Linux process stat is unverifiable", () => {
  assert.throws(
    () => parseLinuxProcessStartTimeTicks("1234 malformed"),
    /missing its process name boundary/,
  );
});
