import assert from "node:assert/strict";
import { PassThrough } from "node:stream";
import test from "node:test";
import type { ReadStream } from "node:tty";

import { readSetupKey, writeSetupLines } from "../src/setup-terminal.js";

test("setup lines restore their origin before replacing wrapped content", () => {
  const output = new PassThrough();
  let written = "";
  output.setEncoding("utf8");
  output.on("data", (chunk: string) => {
    written += chunk;
  });

  const longLabel = "A Tracked Output label that may wrap in a narrow terminal";
  writeSetupLines(["Choose:", `> ${longLabel}`, "  Second"], false, output);
  writeSetupLines(["Choose:", `  ${longLabel}`, "> Second"], true, output);

  assert.equal(
    written,
    `\u001b7Choose:\n> ${longLabel}\n  Second\n\u001b8\u001b[0JChoose:\n  ${longLabel}\n> Second\n`,
  );
});

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
