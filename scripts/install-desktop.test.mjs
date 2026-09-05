import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { installDesktop } from "./install-desktop.mjs";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const applicationId = "io.roonscape.Renderer";

function fixture(context) {
  const scratchRoot = "/var/tmp/codex/roonscape";
  mkdirSync(scratchRoot, { recursive: true });
  const scratch = mkdtempSync(path.join(scratchRoot, "task."));
  context.after(() => rmSync(scratch, { recursive: true, force: true }));
  const root = path.join(scratch, "checkout ' \" $ ` % = \\ space");
  const dataHome = path.join(scratch, "user ' \" $ ` % = \\ data");
  const bridge = path.join(root, "src/bridge/dist/src/roonscape.js");
  mkdirSync(path.dirname(bridge), { recursive: true });
  writeFileSync(
    bridge,
    "process.stdout.write(JSON.stringify(process.argv.slice(2)));\n",
  );
  mkdirSync(path.join(root, "target/release"), { recursive: true });
  writeFileSync(
    path.join(root, "target/release/roonscape-renderer"),
    "#!/bin/sh\nexit 0\n",
    { mode: 0o755 },
  );
  cpSync(
    path.join(repositoryRoot, "src/desktop"),
    path.join(root, "src/desktop"),
    { recursive: true },
  );
  return { root, environment: { XDG_DATA_HOME: dataHome }, scratch };
}

test("installs a usable launcher without relying on the desktop PATH", (context) => {
  const options = fixture(context);
  const [entry, icon, launcher] = installDesktop(options);
  const desktop = readFileSync(entry, "utf8");
  assert.match(desktop, /^Terminal=false$/m);
  assert.match(desktop, /^Icon=io\.roonscape\.Renderer$/m);
  assert.match(desktop, /^StartupWMClass=io\.roonscape\.Renderer$/m);
  assert.doesNotMatch(desktop, /@EXEC@|@ROOT@/);
  assert.equal(
    readFileSync(icon, "utf8"),
    readFileSync(
      path.join(
        options.root,
        "src/desktop/icons/hicolor/scalable/apps",
        `${applicationId}.svg`,
      ),
      "utf8",
    ),
  );
  const launched = spawnSync(
    "/bin/sh",
    [launcher, "literal ' $ ` % \\ argument"],
    {
      encoding: "utf8",
      env: { PATH: "/nonexistent" },
    },
  );
  assert.equal(launched.status, 0, launched.stderr);
  assert.deepEqual(JSON.parse(launched.stdout), [
    "literal ' $ ` % \\ argument",
  ]);

  const validation = spawnSync("desktop-file-validate", [entry], {
    encoding: "utf8",
  });
  assert.equal(validation.status, 0, validation.stderr);
  const menuLaunch = spawnSync("gio", ["launch", entry], {
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(menuLaunch.status, 0, menuLaunch.stderr);
  assert.deepEqual(JSON.parse(menuLaunch.stdout), []);
  assert.deepEqual(installDesktop(options), [entry, icon, launcher]);
});

test("uninstall removes only its files and protects another checkout", (context) => {
  const options = fixture(context);
  const files = installDesktop(options);
  const unrelated = path.join(
    options.environment.XDG_DATA_HOME,
    "applications/other.desktop",
  );
  writeFileSync(unrelated, "keep me");
  assert.throws(
    () =>
      installDesktop({
        ...options,
        root: path.join(options.scratch, "other"),
        uninstall: true,
      }),
    /another installation/,
  );
  assert.ok(files.every(existsSync));
  assert.deepEqual(installDesktop({ ...options, uninstall: true }), files);
  assert.ok(files.every((filename) => !existsSync(filename)));
  assert.equal(readFileSync(unrelated, "utf8"), "keep me");
  assert.deepEqual(installDesktop({ ...options, uninstall: true }), []);
});

test("missing builds fail before installing anything", (context) => {
  const options = fixture(context);
  rmSync(path.join(options.root, "target/release/roonscape-renderer"));
  assert.throws(() => installDesktop(options), /Build the Renderer first/);
  assert.equal(existsSync(options.environment.XDG_DATA_HOME), false);
});

test("respects HOME fallback and rejects relative XDG paths", (context) => {
  const options = fixture(context);
  assert.throws(
    () =>
      installDesktop({
        ...options,
        environment: { XDG_DATA_HOME: "relative" },
      }),
    /absolute/,
  );
  const files = installDesktop({
    ...options,
    environment: { HOME: path.join(options.scratch, "home") },
  });
  assert.ok(
    files.every((filename) =>
      filename.startsWith(path.join(options.scratch, "home/.local/share/")),
    ),
  );
});

test("does not overwrite unrelated entries or follow file symlinks", (context) => {
  const options = fixture(context);
  const entry = path.join(
    options.environment.XDG_DATA_HOME,
    "applications",
    `${applicationId}.desktop`,
  );
  mkdirSync(path.dirname(entry), { recursive: true });
  writeFileSync(entry, "[Desktop Entry]\nName=User customization\n");
  assert.throws(() => installDesktop(options), /not managed/);
  assert.match(readFileSync(entry, "utf8"), /User customization/);
  rmSync(entry);
  const untouched = path.join(options.scratch, "untouched");
  writeFileSync(untouched, "keep me");
  symlinkSync(untouched, entry);
  assert.throws(() => installDesktop(options), /non-file/);
  assert.equal(readFileSync(untouched, "utf8"), "keep me");
});
