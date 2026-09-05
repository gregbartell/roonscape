import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { homedir } from "node:os";
import test from "node:test";
import { findExecutable } from "./native-test-environment.mjs";

const sourceRoot = path.resolve(import.meta.dirname, "..");

test("diagnosis reports complete readiness without changing the worktree or personal files", (context) => {
  const fixture = environment(context);
  const before = contents(fixture.root);
  const result = fixture.run("diagnose");
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /Automated checks: ready/);
  assert.match(result.stdout, /Packaged-fallback Presentation Captures: ready/);
  assert.match(result.stdout, /Complete typography\/profile: ready/);
  assert.deepEqual(contents(fixture.root), before);
});

function environment(context) {
  mkdirSync("/var/tmp/codex/roonscape", { recursive: true });
  const root = mkdtempSync("/var/tmp/codex/roonscape/task.");
  context.after(() => rmSync(root, { recursive: true, force: true }));
  for (const directory of [
    "scripts",
    "bin",
    "personal",
    "runtime",
    "evidence",
  ]) {
    mkdirSync(path.join(root, directory));
  }
  for (const file of [
    "development-environment.mjs",
    "native-test-environment.mjs",
    "inspect-host-fonts.py",
  ]) {
    const source = path.join(sourceRoot, "scripts", file);
    if (existsSync(source)) cpSync(source, path.join(root, "scripts", file));
  }
  cpSync(
    path.join(sourceRoot, "src/renderer/assets/fonts"),
    path.join(root, "src/renderer/assets/fonts"),
    { recursive: true },
  );
  writeFileSync(path.join(root, ".node-version"), process.versions.node);
  writeFileSync(
    path.join(root, "package.json"),
    JSON.stringify({ packageManager: "npm@11.17.0" }),
  );
  writeFileSync(
    path.join(root, "rust-toolchain.toml"),
    '[toolchain]\nchannel = "1.97.1"\n',
  );
  writeFileSync(
    path.join(root, "package-lock.json"),
    "locked npm dependencies\n",
  );
  writeFileSync(path.join(root, "Cargo.lock"), "locked Rust dependencies\n");
  writeFileSync(
    path.join(root, "personal", "display.json"),
    "private display sentinel\n",
  );
  writeFileSync(
    path.join(root, "personal", "authorization.json"),
    "private authorization sentinel\n",
  );
  const command = (name, body) => {
    const destination = path.join(root, "bin", name);
    writeFileSync(destination, `#!${process.execPath}\n${body}\n`);
    chmodSync(destination, 0o755);
  };
  command(
    "npm",
    'if (process.argv.includes("--version")) console.log("11.17.0"); else { require("fs").mkdirSync("node_modules", {recursive:true}); }',
  );
  command("rustc", 'console.log("rustc 1.97.1");');
  command(
    "cargo",
    'if (process.argv.includes("--version")) console.log("cargo 1.97.1"); else { require("fs").mkdirSync(process.env.CARGO_TARGET_DIR, {recursive:true}); }',
  );
  command("rustfmt", 'console.log("rustfmt 1.9.0");');
  command("cargo-clippy", 'console.log("clippy 0.1.97");');
  for (const name of [
    "cc",
    "git",
    "Xvfb",
    "dbus-daemon",
    "xvfb-run",
    "xauth",
    "xwininfo",
    "scrot",
    "pkg-config",
    "ffmpeg",
    "ffprobe",
  ])
    command(name, "process.exit(0);");
  command(
    "python3",
    'console.log(JSON.stringify({families:["Sitka Display","Palatino Linotype","Segoe UI"],hasMoonGlyph:true}));',
  );
  command("fc-query", 'console.log("valid packaged font");');
  const env = {
    PATH: path.join(root, "bin"),
    HOME: path.join(root, "personal"),
    XDG_CONFIG_HOME: path.join(root, "personal"),
    TMPDIR: path.join(root, "runtime"),
  };
  return {
    root,
    command,
    env,
    run: (mode, args = []) =>
      spawnSync(
        process.execPath,
        [
          path.join(root, "scripts/development-environment.mjs"),
          mode,
          ...(args.length ? args : ["--evidence", path.join(root, "evidence")]),
        ],
        { cwd: root, env, encoding: "utf8" },
      ),
  };
}

test("preparation installs locked dependencies locally and preserves personal configuration", (context) => {
  const fixture = environment(context);
  fixture.env.CARGO_TARGET_DIR = path.join(fixture.root, "another-worktree");
  const personal = contents(path.join(fixture.root, "personal"));
  const npmLock = readFileSync(
    path.join(fixture.root, "package-lock.json"),
    "utf8",
  );
  const cargoLock = readFileSync(path.join(fixture.root, "Cargo.lock"), "utf8");
  const result = fixture.run("prepare");
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /Development dependencies prepared/);
  assert.ok(existsSync(path.join(fixture.root, "node_modules")));
  assert.ok(existsSync(path.join(fixture.root, "target")));
  assert.ok(!existsSync(fixture.env.CARGO_TARGET_DIR));
  assert.deepEqual(contents(path.join(fixture.root, "personal")), personal);
  assert.equal(
    readFileSync(path.join(fixture.root, "package-lock.json"), "utf8"),
    npmLock,
  );
  assert.equal(
    readFileSync(path.join(fixture.root, "Cargo.lock"), "utf8"),
    cargoLock,
  );
});

test("missing host fonts limit typography without blocking preparation or fallback captures", (context) => {
  const fixture = environment(context);
  fixture.command(
    "python3",
    "console.log(JSON.stringify({families:[],hasMoonGlyph:true}));",
  );
  const result = fixture.run("prepare");
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /Automated checks: ready/);
  assert.match(result.stdout, /Packaged-fallback Presentation Captures: ready/);
  assert.match(result.stdout, /Complete typography\/profile: unavailable/);
  assert.match(result.stdout, /Host font Sitka Display unavailable/);
  assert.match(result.stdout, /Host font Palatino Linotype unavailable/);
  assert.match(result.stdout, /Host font Segoe UI unavailable/);
});

for (const failure of [
  "node",
  "npm",
  "rustc",
  "cargo",
  "rustfmt",
  "cargo-clippy",
  "python3",
  "scrot",
  "gtk",
  "packaged-font",
]) {
  test(`preparation refuses ${failure} prerequisite failure before installation`, (context) => {
    const fixture = environment(context);
    if (failure === "node")
      writeFileSync(path.join(fixture.root, ".node-version"), "0.0.1");
    else if (failure === "gtk")
      fixture.command("pkg-config", "process.exit(1);");
    else if (failure === "packaged-font")
      fixture.command("fc-query", "process.exit(1);");
    else
      fixture.command(
        failure,
        'console.log("incompatible version"); process.exit(1);',
      );
    if (failure === "scrot") rmSync(path.join(fixture.root, "bin/scrot"));
    const before = contents(fixture.root);
    const result = fixture.run("prepare");
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(result.stdout, /Automated checks: unavailable/);
    assert.deepEqual(contents(fixture.root), before);
  });
}

for (const location of ["runtime", "evidence"]) {
  test(`diagnosis rejects an unusable ${location} location without creating files`, (context) => {
    const fixture = environment(context);
    const unusable = path.join(fixture.root, "file");
    writeFileSync(unusable, "not a directory");
    if (location === "runtime") fixture.env.TMPDIR = unusable;
    const before = contents(fixture.root);
    const result = fixture.run(
      "diagnose",
      location === "evidence"
        ? ["--evidence", path.join(unusable, "child")]
        : [],
    );
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(result.stdout, /directory is unusable/);
    assert.deepEqual(contents(fixture.root), before);
  });
}

test("diagnosis accepts a creatable evidence directory without creating it", (context) => {
  const fixture = environment(context);
  const evidence = path.join(fixture.root, "new/evidence");
  const result = fixture.run("diagnose", ["--evidence", evidence]);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.ok(!existsSync(path.join(fixture.root, "new")));
});

test("preparation reports failed downloads and does not continue installation", (context) => {
  const fixture = environment(context);
  fixture.command(
    "npm",
    'if (process.argv.includes("--version")) console.log("11.17.0"); else process.exit(23);',
  );
  const result = fixture.run("prepare");
  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.match(result.stderr, /npm dependency installation failed \(exit 23\)/);
  assert.ok(!existsSync(path.join(fixture.root, "target")));
});

test("preparation rejects dependency directories redirected outside the worktree", (context) => {
  const fixture = environment(context);
  symlinkSync(
    path.join(fixture.root, "personal"),
    path.join(fixture.root, "node_modules"),
  );
  const result = fixture.run("prepare");
  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.match(result.stdout, /node_modules.*outside|node_modules.*symlink/);
});

test("real host font inspection leaves an uncached Fontconfig environment unchanged", (context) => {
  const fixture = environment(context);
  rmSync(path.join(fixture.root, "bin/python3"));
  symlinkSync("/usr/bin/python3", path.join(fixture.root, "bin/python3"));
  fixture.env.FONTCONFIG_FILE = path.join(fixture.root, "fonts.conf");
  writeFileSync(
    fixture.env.FONTCONFIG_FILE,
    `<fontconfig><dir>${fixture.root}/src/renderer/assets/fonts</dir><cachedir>${fixture.root}/personal/font-cache</cachedir></fontconfig>`,
  );
  const before = contents(fixture.root);
  const result = fixture.run("diagnose");
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /Complete typography\/profile: unavailable/);
  assert.doesNotMatch(result.stdout, /host font inspection failed/);
  assert.match(result.stdout, /Host font Sitka Display unavailable/);
  assert.match(result.stdout, /Host glyph fallback for 月 is unavailable/);
  assert.deepEqual(contents(fixture.root), before);
});

test("real package managers preserve lockfiles and never run application lifecycle scripts", (context) => {
  const fixture = environment(context);
  for (const name of ["cargo", "rustc"]) {
    rmSync(path.join(fixture.root, "bin", name));
    symlinkSync(findExecutable(name), path.join(fixture.root, "bin", name));
  }
  symlinkSync(process.execPath, path.join(fixture.root, "bin/node"));
  const npmCli =
    process.env.npm_execpath ??
    spawnSync(
      findExecutable("npm"),
      ["exec", "--", "node", "-p", "process.env.npm_execpath"],
      { encoding: "utf8" },
    ).stdout.trim();
  fixture.command("npm", `require(${JSON.stringify(npmCli)});`);
  fixture.env.npm_config_cache = path.join(fixture.root, "npm-cache");
  fixture.env.CARGO_HOME = path.join(fixture.root, "cargo-cache");
  fixture.env.RUSTUP_HOME =
    process.env.RUSTUP_HOME ?? path.join(homedir(), ".rustup");
  const npmVersion = spawnSync(findExecutable("npm"), ["--version"], {
    encoding: "utf8",
  }).stdout.trim();
  const metadata = {
    name: "preparation-fixture",
    version: "1.0.0",
    packageManager: `npm@${npmVersion}`,
    scripts: { prepare: "exit 91", install: "exit 92" },
  };
  writeFileSync(
    path.join(fixture.root, "package.json"),
    JSON.stringify(metadata),
  );
  writeFileSync(
    path.join(fixture.root, "package-lock.json"),
    JSON.stringify({
      name: metadata.name,
      version: metadata.version,
      lockfileVersion: 3,
      packages: {
        "": {
          name: metadata.name,
          version: metadata.version,
          hasInstallScript: true,
        },
      },
    }),
  );
  writeFileSync(
    path.join(fixture.root, "Cargo.toml"),
    '[package]\nname = "preparation-fixture"\nversion = "1.0.0"\nedition = "2021"\n',
  );
  writeFileSync(path.join(fixture.root, "src/lib.rs"), "");
  writeFileSync(
    path.join(fixture.root, "Cargo.lock"),
    'version = 4\n\n[[package]]\nname = "preparation-fixture"\nversion = "1.0.0"\n',
  );
  const personal = contents(path.join(fixture.root, "personal"));
  const lockfiles = ["package-lock.json", "Cargo.lock"].map((name) =>
    readFileSync(path.join(fixture.root, name), "utf8"),
  );
  const result = fixture.run("prepare");
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /Development dependencies prepared/);
  assert.deepEqual(
    ["package-lock.json", "Cargo.lock"].map((name) =>
      readFileSync(path.join(fixture.root, name), "utf8"),
    ),
    lockfiles,
  );
  assert.deepEqual(contents(path.join(fixture.root, "personal")), personal);
});

function contents(directory) {
  return Object.fromEntries(
    readdirSync(directory, { withFileTypes: true }).map((entry) => [
      entry.name,
      entry.isDirectory()
        ? contents(path.join(directory, entry.name))
        : readFileSync(path.join(directory, entry.name), "base64"),
    ]),
  );
}
