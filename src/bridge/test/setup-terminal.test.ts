import assert from "node:assert/strict";
import { PassThrough } from "node:stream";
import test from "node:test";
import type { ReadStream } from "node:tty";

import { readSetupKey } from "../src/setup-terminal.js";

test("Ctrl-C cancels a setup key read and releases stdin", async () => {
  const input = new PassThrough() as PassThrough & {
    isRaw: boolean;
    setRawMode(enabled: boolean): ReadStream;
  };
  input.isRaw = false;
  input.setRawMode = (enabled) => {
    input.isRaw = enabled;
    return input as unknown as ReadStream;
  };

  const key = readSetupKey(
    new AbortController().signal,
    input as unknown as ReadStream,
  );
  input.write("\u0003");

  await assert.rejects(key, { name: "AbortError" });
  assert.equal(input.isPaused(), true);
});
