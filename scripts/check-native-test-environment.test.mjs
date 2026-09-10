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
import { installQtFixture } from "./qt-environment-fixture.mjs";
import { qtDevelopmentFailures } from "./native-test-environment.mjs";

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

test("the native test preflight accepts Qt without pkg-config metadata", (context) => {
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
    writeFileSync(
      executable,
      command === "pkg-config"
        ? '#!/bin/sh\ncase "$*" in *Qt6*) exit 1;; esac\nexit 0\n'
        : "#!/bin/sh\nexit 0\n",
    );
    chmodSync(executable, 0o755);
  }
  installQtFixture(directory);
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

for (const [failure, change, expected] of [
  [
    "missing qmake6",
    (directory) => rmSync(path.join(directory, "qmake6")),
    "required executable is unavailable: qmake6",
  ],
  [
    "unsupported Qt version",
    (directory) => installQtFixture(directory, "6.1.3"),
    "Qt 6.2 or newer development files are unavailable",
  ],
  [
    "missing Quick headers",
    (directory) => rmSync(path.join(directory, "qt/include/QtQuick/QtQuick")),
    "Qt Quick 6.2 development files are unavailable",
  ],
  [
    "missing OpenGL library",
    (directory) => rmSync(path.join(directory, "qt/lib/libQt6OpenGL.so")),
    "Qt OpenGL 6.2 development files are unavailable",
  ],
  [
    "failed metadata query",
    (directory) =>
      writeFileSync(path.join(directory, "qmake6"), "#!/bin/sh\nexit 1\n"),
    "Qt installation metadata is unavailable: qmake6 -query failed",
  ],
]) {
  test(`Qt discovery reports ${failure}`, (context) => {
    const directory = createCommandDirectory(context, []);
    change(directory);
    assert.deepEqual(qtDevelopmentFailures({ PATH: directory }), [expected]);
  });
}
