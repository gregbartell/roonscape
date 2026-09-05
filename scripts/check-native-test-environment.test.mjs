import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const environmentCheck = fileURLToPath(
  new URL("check-native-test-environment.mjs", import.meta.url),
);

test("the native test preflight reports a missing capture executable", (context) => {
  const commandDirectory = createCommandDirectory(context, [
    "Xvfb",
    "dbus-daemon",
    "pkg-config",
    "ffmpeg",
    "ffprobe",
    "xwininfo",
    "xprop",
    "gio",
    "desktop-file-validate",
  ]);

  const result = runEnvironmentCheck(commandDirectory);

  assert.equal(result.status, 1, processOutput(result));
  assert.equal(result.stdout, "");
  assert.equal(
    result.stderr,
    "Native test environment is unavailable:\n" +
      "- required executable is unavailable: scrot\n",
  );
});

test("the native test preflight accepts the complete environment", (context) => {
  const commandDirectory = createCommandDirectory(context, [
    "Xvfb",
    "dbus-daemon",
    "pkg-config",
    "ffmpeg",
    "ffprobe",
    "scrot",
    "xwininfo",
    "xprop",
    "gio",
    "desktop-file-validate",
  ]);

  const result = runEnvironmentCheck(commandDirectory);

  assert.equal(result.status, 0, processOutput(result));
  assert.equal(result.stdout, "Native test environment is available\n");
  assert.equal(result.stderr, "");
});

function createCommandDirectory(context, commands) {
  const directory = mkdtempSync(
    path.join(tmpdir(), "roonscape-native-environment-test."),
  );
  context.after(() => rmSync(directory, { force: true, recursive: true }));
  for (const command of commands) {
    const executable = path.join(directory, command);
    mkdirSync(path.dirname(executable), { recursive: true });
    writeFileSync(executable, "#!/bin/sh\nexit 0\n");
    chmodSync(executable, 0o755);
  }
  return directory;
}

function runEnvironmentCheck(commandDirectory) {
  return spawnSync(process.execPath, [environmentCheck], {
    encoding: "utf8",
    env: { PATH: commandDirectory },
  });
}

function processOutput(result) {
  return [result.stdout, result.stderr].filter(Boolean).join("\n");
}
