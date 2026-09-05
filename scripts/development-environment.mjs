import {
  constants,
  accessSync,
  lstatSync,
  readFileSync,
  statSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  findExecutable,
  nativeTestFailures,
} from "./native-test-environment.mjs";

const root = path.resolve(import.meta.dirname, "..");
const [mode, ...args] = process.argv.slice(2);

try {
  if (
    !["diagnose", "prepare"].includes(mode) ||
    (args.length !== 0 && (args.length !== 2 || args[0] !== "--evidence"))
  ) {
    throw new Error(
      "Usage: node scripts/development-environment.mjs diagnose|prepare [--evidence DIRECTORY]",
    );
  }
  const nodeVersion = read(".node-version").trim();
  const npmVersion = JSON.parse(read("package.json")).packageManager.replace(
    /^npm@/,
    "",
  );
  const rustVersion = read("rust-toolchain.toml").match(
    /^channel\s*=\s*"([^"]+)"/m,
  )?.[1];
  if (!rustVersion)
    throw new Error("Missing Rust toolchain pin in rust-toolchain.toml");
  // Disable rustup's automatic installation, including during version probes.
  const environment = {
    ...process.env,
    RUSTUP_TOOLCHAIN: rustVersion,
    RUSTUP_AUTO_INSTALL: "0",
    CARGO_TARGET_DIR: path.join(root, "target"),
    CARGO_BUILD_BUILD_DIR: path.join(root, "target"),
  };
  const required = [];
  if (process.versions.node !== nodeVersion)
    required.push(
      `Node ${nodeVersion} required; found ${process.versions.node}. Select .node-version with your Node manager.`,
    );
  for (const [name, version] of [
    ["npm", npmVersion],
    ["rustc", rustVersion],
    ["cargo", rustVersion],
  ]) {
    const result = probe(name, ["--version"]);
    if (!result.ok || !result.output.split(/\s+/).includes(version))
      required.push(
        `${name} ${version} required; unavailable or incompatible. Select the repository-pinned toolchain (docs/development.md).`,
      );
  }
  for (const name of ["rustfmt", "cargo-clippy", "cc", "git", "python3"]) {
    if (!probe(name, ["--version"]).ok)
      required.push(
        `${name} is unavailable. Provision the development host; see docs/development.md.`,
      );
  }
  for (const name of ["xvfb-run", "xauth"]) {
    if (!findExecutable(name, environment))
      required.push(
        `${name} is unavailable. Provision the development host; see docs/development.md.`,
      );
  }
  required.push(
    ...nativeTestFailures(environment).map(
      (failure) =>
        `${failure}. Provision native packages; see docs/development.md.`,
    ),
  );
  if (!probe("pkg-config", ["--exists", "fontconfig"]).ok)
    required.push(
      "Fontconfig development files are unavailable. Provision native packages; see docs/development.md.",
    );
  const runtime = tmpdir();
  checkDirectory(runtime, required, "Runtime directory", false);
  if (
    Buffer.byteLength(
      path.join(
        runtime,
        "roonscape-controlled-capture.XXXXXX/capture-control.sock",
      ),
    ) >= 108
  )
    required.push(
      "Runtime directory path is too long for capture sockets. Set TMPDIR to a shorter writable directory.",
    );
  if (process.env.XDG_RUNTIME_DIR)
    checkDirectory(
      process.env.XDG_RUNTIME_DIR,
      required,
      "XDG_RUNTIME_DIR",
      false,
    );
  checkDirectory(root, required, "Worktree", false);
  checkDirectory(path.join(root, "target"), required, "Build output", true);
  for (const output of [
    "node_modules",
    "target",
    "src/bridge/node_modules",
    "src/bridge/dist",
  ]) {
    let directory = root;
    for (const component of output.split("/")) {
      directory = path.join(directory, component);
      try {
        if (lstatSync(directory).isSymbolicLink()) {
          required.push(
            `${output} uses a symlink. Restore a worktree-local directory before preparation.`,
          );
          break;
        }
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
      }
    }
  }
  const packaged = [];
  for (const name of [
    "LibreBaskerville-Variable.ttf",
    "LibreBaskerville-Italic-Variable.ttf",
    "IBMPlexSans-Variable.ttf",
    "IBMPlexSans-Italic-Variable.ttf",
  ]) {
    const font = path.join(root, "src/renderer/assets/fonts", name);
    if (!probe("fc-query", ["--format=%{family}", font]).ok)
      packaged.push(
        `Packaged font ${name} is unreadable or invalid (or fc-query is missing). Restore tracked font assets and install Fontconfig.`,
      );
  }
  const evidence = path.resolve(args[1] ?? root);
  const capture = [...packaged];
  checkDirectory(evidence, capture, "Evidence directory", true);
  const typography = [];
  const hostFonts = probe("python3", [
    path.join(root, "scripts/inspect-host-fonts.py"),
  ]);
  const fontInventory = hostFonts.ok
    ? JSON.parse(hostFonts.output)
    : { families: [], hasMoonGlyph: false };
  const availableFamilies = new Set(
    fontInventory.families.map((name) => name.toLowerCase()),
  );
  if (!hostFonts.ok)
    typography.push(
      "Read-only host font inspection failed. Install Python 3 and Fontconfig and grant access to configured font directories; see docs/development.md.",
    );
  for (const family of ["Sitka Display", "Palatino Linotype", "Segoe UI"]) {
    if (!availableFamilies.has(family.toLowerCase()))
      typography.push(
        `Host font ${family} unavailable. Install a licensed host copy; see docs/development.md.`,
      );
  }
  if (!fontInventory.hasMoonGlyph)
    typography.push(
      "Host glyph fallback for 月 is unavailable. Install Noto CJK fonts; see docs/development.md.",
    );
  report("Automated checks", [...required, ...packaged]);
  report("Packaged-fallback Presentation Captures", [...required, ...capture]);
  report("Complete typography/profile", [
    ...required,
    ...capture,
    ...typography,
  ]);
  process.stdout.write(
    `Runtime: ${runtime}\nEvidence: ${evidence}\nRead-only probes cannot guarantee free space, socket binding, or agent execution permission; verification exercises those operations.\n`,
  );
  process.exitCode = required.length + capture.length > 0 ? 1 : 0;
  if (mode === "prepare") {
    if (process.exitCode !== 0)
      throw new Error(
        "Resolve the prerequisite failures above, then retry preparation.",
      );
    for (const [name, arguments_] of [
      [
        "npm",
        [
          "ci",
          "--ignore-scripts",
          "--no-audit",
          "--no-fund",
          "--include=dev",
          "--global=false",
          "--prefix",
          root,
        ],
      ],
      ["cargo", ["fetch", "--locked"]],
    ]) {
      process.stdout.write(`Preparing dependencies with ${name}...\n`);
      const result = spawnSync(findExecutable(name, environment), arguments_, {
        cwd: root,
        env: environment,
        stdio: "inherit",
      });
      if (result.error || result.status !== 0)
        throw new Error(
          `${name} dependency installation failed${result.status === null ? "" : ` (exit ${result.status})`}. Check network/cache permissions and the preceding output; source lockfiles must remain unchanged.`,
        );
    }
    process.stdout.write(
      "Development dependencies prepared. Run npm run verify for verification; add -- --design when the change requires the design suite.\n",
    );
  }

  function probe(name, arguments_) {
    const executable = findExecutable(name, environment);
    if (!executable) return { ok: false, output: "" };
    const result = spawnSync(executable, arguments_, {
      cwd: root,
      env: environment,
      encoding: "utf8",
      timeout: 15_000,
    });
    return {
      ok: !result.error && result.status === 0,
      output: result.stdout ?? "",
    };
  }
} catch (error) {
  process.stderr.write(
    `Development ${mode ?? "command"} failed: ${error.message}\n`,
  );
  process.exitCode = 1;
}

function read(file) {
  return readFileSync(path.join(root, file), "utf8");
}

function report(label, failures) {
  process.stdout.write(
    `${label}: ${failures.length ? "unavailable" : "ready"}\n`,
  );
  for (const failure of failures) process.stdout.write(`- ${failure}\n`);
}

function checkDirectory(directory, failures, label, allowMissing) {
  try {
    let existing = directory;
    while (true) {
      try {
        if (!statSync(existing).isDirectory())
          throw new Error("not a directory");
        accessSync(existing, constants.W_OK | constants.X_OK);
        break;
      } catch (error) {
        if (
          !allowMissing ||
          error.code !== "ENOENT" ||
          path.dirname(existing) === existing
        )
          throw error;
        existing = path.dirname(existing);
      }
    }
  } catch {
    failures.push(
      `${label} is unusable: ${directory}. Choose an accessible writable directory and grant filesystem access explicitly.`,
    );
  }
}
