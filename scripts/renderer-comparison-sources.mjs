import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import {
  copyFile,
  cp,
  lstat,
  mkdir,
  readFile,
  readdir,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import {
  runMonitoredProcess,
  startMonitoredProcess,
  waitForProcessExit,
  stopProcess,
  processFailure,
} from "./process-harness.mjs";

const execute = promisify(execFile);
export const digest = (value) =>
  createHash("sha256").update(value).digest("hex");
async function git(cwd, args, signal) {
  return (
    await execute("git", ["-C", cwd, ...args], {
      maxBuffer: 32 * 1024 * 1024,
      signal,
    })
  ).stdout;
}

export async function prepareSource(selection, destination, signal) {
  const started = performance.now();
  const isWorktree = selection.startsWith("worktree:");
  if (!isWorktree && !selection.startsWith("ref:"))
    throw new Error("source must be ref:REVISION or worktree:PATH");
  const input = selection.slice(selection.indexOf(":") + 1);
  if (!input) throw new Error("empty source selection");
  const cwd = isWorktree ? path.resolve(input) : process.cwd();
  if (
    isWorktree &&
    path.resolve(
      (await git(cwd, ["rev-parse", "--show-toplevel"], signal)).trim(),
    ) !== cwd
  )
    throw new Error("worktree selection must name its repository root");
  const revision = (
    await git(
      cwd,
      ["rev-parse", "--verify", `${isWorktree ? "HEAD" : input}^{commit}`],
      signal,
    )
  ).trim();
  const state = isWorktree
    ? await git(
        cwd,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        signal,
      )
    : "";
  await mkdir(destination);
  if (isWorktree) {
    const files = (
      await git(
        cwd,
        ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        signal,
      )
    )
      .split("\0")
      .filter(Boolean);
    for (const file of new Set(files)) {
      signal.throwIfAborted();
      let metadata;
      try {
        metadata = await lstat(path.join(cwd, file));
      } catch (error) {
        if (error.code === "ENOENT") continue;
        throw error;
      }
      if (!metadata.isFile())
        throw new Error(
          `source requires regular files (no submodules or symlinks): ${file}`,
        );
      await mkdir(path.dirname(path.join(destination, file)), {
        recursive: true,
      });
      await copyFile(path.join(cwd, file), path.join(destination, file));
    }
    // Detect edits during copying; the copied content, not HEAD, identifies this build.
    if (
      state !==
      (await git(
        cwd,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        signal,
      ))
    )
      throw new Error(
        "source changed during snapshot; retry with a stable worktree",
      );
  } else {
    const archive = `${destination}.tar`;
    await git(
      cwd,
      ["archive", "--format=tar", `--output=${archive}`, revision],
      signal,
    );
    await runMonitoredProcess("tar", ["-xf", archive, "-C", destination], {
      signal,
    });
  }
  const manifest = [];
  async function visit(directory, prefix = "") {
    for (const entry of (
      await readdir(directory, { withFileTypes: true })
    ).sort((a, b) => a.name.localeCompare(b.name))) {
      const relative = path.join(prefix, entry.name);
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) await visit(full, relative);
      else if (entry.isFile())
        manifest.push({
          path: relative,
          mode: (await lstat(full)).mode & 0o777,
          sha256: digest(await readFile(full)),
        });
      else throw new Error(`source requires regular files: ${relative}`);
    }
  }
  await visit(destination);
  if (isWorktree) {
    for (const file of manifest)
      if (digest(await readFile(path.join(cwd, file.path))) !== file.sha256)
        throw new Error(
          "source changed during snapshot; retry with a stable worktree",
        );
  }
  if (
    isWorktree &&
    ((await git(cwd, ["rev-parse", "HEAD"], signal)).trim() !== revision ||
      (await git(
        cwd,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        signal,
      )) !== state)
  )
    throw new Error(
      "source changed during snapshot; retry with a stable worktree",
    );
  return {
    selection,
    revision,
    dirty: state.length > 0,
    contentDigest: digest(JSON.stringify(manifest)),
    manifest,
    preparationMs: performance.now() - started,
  };
}

export async function buildSource(directory, evidence, signal) {
  const started = performance.now();
  const environment = {
    ...process.env,
    CARGO_TARGET_DIR: path.join(path.dirname(directory), "target"),
    CARGO_BUILD_BUILD_DIR: path.join(path.dirname(directory), "target"),
    CARGO_PROFILE_RELEASE_OPT_LEVEL: "3",
    CARGO_PROFILE_RELEASE_DEBUG: "0",
    RUSTUP_AUTO_INSTALL: "0",
  };
  for (const name of [
    "CARGO_BUILD_TARGET_DIR",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CFLAGS",
    "CXXFLAGS",
    "CPPFLAGS",
    "LDFLAGS",
    "LD_PRELOAD",
    "LD_AUDIT",
  ])
    delete environment[name];
  // Git archives retain old mtimes. Cargo can otherwise reuse this package's
  // artifacts from the other snapshot, despite different source contents.
  // Keep the expensive third-party dependencies shared, but rebuild our crate.
  await runMonitoredProcess(
    "cargo",
    ["clean", "--release", "--package", "roonscape-renderer"],
    { cwd: directory, environment, signal },
  );
  const command = [
    "rustc",
    "--locked",
    "--release",
    "--package",
    "roonscape-renderer",
    "--bin",
    "roonscape-renderer",
    "--",
    "-C",
    "opt-level=3",
  ];
  const child = startMonitoredProcess("cargo", command, {
    cwd: directory,
    environment,
  });
  try {
    await child.spawned;
    const [code, exitSignal] = await waitForProcessExit(child, {
      signal,
      timeoutMilliseconds: 30 * 60_000,
    });
    if (code !== 0)
      throw processFailure("release build", child, code, exitSignal);
  } finally {
    await stopProcess(child, { signal });
    await writeFile(
      path.join(evidence, "build.log"),
      child.capturedStandardOutput + child.capturedStandardError,
    );
  }
  const binary = path.join(
    path.dirname(directory),
    "target/release/roonscape-renderer",
  );
  const bytes = await readFile(binary);
  await mkdir(path.join(evidence, "target/release"), { recursive: true });
  await copyFile(
    binary,
    path.join(evidence, "target/release/roonscape-renderer"),
  );
  await copyRuntimeResources(directory, evidence);
  return {
    binaryDigest: digest(bytes),
    profile: "release",
    optLevel: 3,
    command: ["cargo", ...command],
    buildMs: performance.now() - started,
    log: "build.log",
  };
}

export async function copyRuntimeResources(source, destination) {
  for (const relative of ["src/renderer/assets/fonts", "src/desktop/icons"]) {
    await mkdir(path.dirname(path.join(destination, relative)), {
      recursive: true,
    });
    await cp(path.join(source, relative), path.join(destination, relative), {
      recursive: true,
    });
  }
}
