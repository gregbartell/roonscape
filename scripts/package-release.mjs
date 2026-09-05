import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  assembleReleasePackage,
  validateReleasePackage,
  verifyFileChecksum,
} from "./release-package.mjs";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const packageMetadata = readJson(path.join(repositoryRoot, "package.json"));
const version = requiredString(packageMetadata.version, "package version");
const nodeVersion = readFileSync(
  path.join(repositoryRoot, ".node-version"),
  "utf8",
).trim();
const nodeArchiveName = `node-v${nodeVersion}-linux-x64.tar.xz`;
const nodeArchiveChecksum =
  "14b342e71204f811bde6153be8e04b62aef63c236fef92b55f9c83154b409647";
const scratchDirectory = mkdtempSync(path.join(tmpdir(), "roonscape-package."));

if (process.platform !== "linux" || process.arch !== "x64") {
  throw new Error(
    "RoonScape packaging supports only x86-64 glibc-based Linux hosts",
  );
}

try {
  run("pkg-config", ["--atleast-version=4.6", "gtk4"]);
  rmSync(path.join(repositoryRoot, "src/bridge/dist"), {
    force: true,
    recursive: true,
  });
  run("npm", ["run", "build", "--workspace", "@roonscape/bridge"]);
  run("cargo", [
    "build",
    "--locked",
    "--release",
    "--package",
    "roonscape-renderer",
  ]);

  const dependencyWorkspace = path.join(scratchDirectory, "dependencies");
  mkdirSync(path.join(dependencyWorkspace, "src/bridge"), { recursive: true });
  copyFileSync(
    path.join(repositoryRoot, "package.json"),
    path.join(dependencyWorkspace, "package.json"),
  );
  copyFileSync(
    path.join(repositoryRoot, "package-lock.json"),
    path.join(dependencyWorkspace, "package-lock.json"),
  );
  copyFileSync(
    path.join(repositoryRoot, "src/bridge/package.json"),
    path.join(dependencyWorkspace, "src/bridge/package.json"),
  );
  run(
    "npm",
    ["ci", "--omit=dev", "--ignore-scripts", "--no-audit", "--no-fund"],
    { cwd: dependencyWorkspace },
  );

  const nodeArchive = path.join(scratchDirectory, nodeArchiveName);
  await download(
    `https://nodejs.org/dist/v${nodeVersion}/${nodeArchiveName}`,
    nodeArchive,
  );
  verifyFileChecksum(nodeArchive, nodeArchiveChecksum, "Node runtime");
  run("tar", ["-xJf", nodeArchive, "-C", scratchDirectory]);
  const nodeDistribution = path.join(
    scratchDirectory,
    `node-v${nodeVersion}-linux-x64`,
  );

  const sourceDateEpoch = output("git", ["log", "-1", "--format=%ct"]).trim();
  const artifacts = assembleReleasePackage({
    inputs: {
      bridgeBuild: path.join(repositoryRoot, "src/bridge/dist/src"),
      bridgeNodeModules: path.join(
        dependencyWorkspace,
        "src/bridge/node_modules",
      ),
      bridgeSource: path.join(repositoryRoot, "src/bridge/src"),
      desktop: path.join(repositoryRoot, "src/desktop"),
      desktopInstaller: path.join(
        repositoryRoot,
        "scripts/install-desktop.mjs",
      ),
      fonts: path.join(repositoryRoot, "src/renderer/assets/fonts"),
      launcher: path.join(repositoryRoot, "src/launcher/roonscape"),
      nodeDistribution,
      renderer: path.join(repositoryRoot, "target/release/roonscape-renderer"),
      rootNodeModules: path.join(dependencyWorkspace, "node_modules"),
      shared: path.join(repositoryRoot, "src/shared"),
    },
    outputDirectory: path.join(repositoryRoot, "release"),
    scratchDirectory: path.join(scratchDirectory, "assembly"),
    sourceDateEpoch,
    version,
  });
  validateReleasePackage({
    artifacts,
    expectedVersion: version,
    scratchDirectory: path.join(scratchDirectory, "validation"),
  });

  process.stdout.write(
    `Built ${path.relative(repositoryRoot, artifacts.archiveFile)}\n` +
      `Built ${path.relative(repositoryRoot, artifacts.checksumFile)}\n`,
  );
} finally {
  rmSync(scratchDirectory, { force: true, recursive: true });
}

async function download(url, destination) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Could not download ${url}: HTTP ${response.status}`);
  }
  writeFileSync(destination, Buffer.from(await response.arrayBuffer()));
}

function run(executable, arguments_, options = {}) {
  const result = spawnSync(executable, arguments_, {
    cwd: repositoryRoot,
    stdio: "inherit",
    ...options,
  });
  assertCommandSucceeded(executable, result);
}

function output(executable, arguments_) {
  const result = spawnSync(executable, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
  assertCommandSucceeded(executable, result);
  return result.stdout;
}

function assertCommandSucceeded(executable, result) {
  if (result.error !== undefined && result.status === null) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${executable} exited with status ${result.status}`);
  }
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function requiredString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`RoonScape ${label} is unavailable`);
  }
  return value;
}
