import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import {
  chmodSync,
  closeSync,
  copyFileSync,
  cpSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
const releaseRootName = "roonscape";
const archiveName = "roonscape-linux-x64.tar.gz";
const outputDirectory = path.join(repositoryRoot, "release");
const archiveFile = path.join(outputDirectory, archiveName);
const checksumFile = `${archiveFile}.sha256`;
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
    dependencyWorkspace,
  );

  const nodeArchive = path.join(scratchDirectory, nodeArchiveName);
  await download(
    `https://nodejs.org/dist/v${nodeVersion}/${nodeArchiveName}`,
    nodeArchive,
  );
  assertChecksum(nodeArchive, nodeArchiveChecksum, "Node runtime");
  const nodeDistributionRoot = path.join(
    scratchDirectory,
    `node-v${nodeVersion}-linux-x64`,
  );
  run("tar", ["-xJf", nodeArchive, "-C", scratchDirectory]);

  const stageRoot = path.join(scratchDirectory, "stage", releaseRootName);
  copyRuntimeTree(stageRoot, dependencyWorkspace, nodeDistributionRoot);
  normalizePermissions(stageRoot);

  const renderer = path.join(stageRoot, "target/release/roonscape-renderer");
  const privateNode = path.join(stageRoot, "runtime/node/bin/node");
  chmodSync(path.join(stageRoot, "roonscape"), 0o755);
  chmodSync(renderer, 0o755);
  chmodSync(privateNode, 0o755);
  assertGlibcBaseline(renderer);
  assertGlibcBaseline(privateNode);

  rmSync(outputDirectory, { force: true, recursive: true });
  mkdirSync(outputDirectory, { recursive: true });
  const sourceDateEpoch = output("git", ["log", "-1", "--format=%ct"]).trim();
  const tarFile = path.join(scratchDirectory, `${releaseRootName}.tar`);
  run("tar", [
    "--sort=name",
    `--mtime=@${sourceDateEpoch}`,
    "--owner=0",
    "--group=0",
    "--numeric-owner",
    "--format=posix",
    "--pax-option=delete=atime,delete=ctime",
    "-cf",
    tarFile,
    "-C",
    path.dirname(stageRoot),
    releaseRootName,
  ]);
  runGzip(tarFile, archiveFile);

  const archiveChecksum = sha256(archiveFile);
  writeFileSync(
    checksumFile,
    `${archiveChecksum}  ${path.basename(archiveFile)}\n`,
  );
  process.stdout.write(
    `Built ${path.relative(repositoryRoot, archiveFile)}\n` +
      `Built ${path.relative(repositoryRoot, checksumFile)}\n`,
  );
} finally {
  rmSync(scratchDirectory, { force: true, recursive: true });
}

function copyRuntimeTree(stageRoot, dependencyWorkspace, nodeDistributionRoot) {
  copyFile(
    path.join(repositoryRoot, "src/launcher/roonscape"),
    path.join(stageRoot, "roonscape"),
  );
  copyJavaScript(
    path.join(repositoryRoot, "src/bridge/dist/src"),
    path.join(stageRoot, "src/bridge/dist/src"),
  );
  copyTree(
    path.join(dependencyWorkspace, "node_modules"),
    path.join(stageRoot, "node_modules"),
    (source) =>
      source !== path.join(dependencyWorkspace, "node_modules/.bin") &&
      source !==
        path.join(dependencyWorkspace, "node_modules/@roonscape/bridge"),
  );
  copyTree(
    path.join(dependencyWorkspace, "src/bridge/node_modules"),
    path.join(stageRoot, "src/bridge/node_modules"),
  );
  copyTree(
    path.join(repositoryRoot, "src/shared"),
    path.join(stageRoot, "src/shared"),
  );
  copyTree(
    path.join(repositoryRoot, "src/renderer/assets/fonts"),
    path.join(stageRoot, "src/renderer/assets/fonts"),
  );
  copyFile(
    path.join(repositoryRoot, "target/release/roonscape-renderer"),
    path.join(stageRoot, "target/release/roonscape-renderer"),
  );
  copyFile(
    path.join(nodeDistributionRoot, "bin/node"),
    path.join(stageRoot, "runtime/node/bin/node"),
  );
  copyFile(
    path.join(nodeDistributionRoot, "LICENSE"),
    path.join(stageRoot, "runtime/node/LICENSE"),
  );
  writeFileSync(
    path.join(stageRoot, "package.json"),
    `${JSON.stringify(
      { name: "roonscape", version, private: true, type: "module" },
      null,
      2,
    )}\n`,
  );
}

function copyJavaScript(source, destination) {
  copyTree(source, destination, (candidate) => {
    const details = lstatSync(candidate);
    return details.isDirectory() || candidate.endsWith(".js");
  });
}

function copyTree(source, destination, filter = undefined) {
  cpSync(source, destination, {
    recursive: true,
    verbatimSymlinks: true,
    ...(filter === undefined ? {} : { filter }),
  });
}

function copyFile(source, destination) {
  mkdirSync(path.dirname(destination), { recursive: true });
  copyFileSync(source, destination);
}

function normalizePermissions(directory) {
  chmodSync(directory, 0o755);
  for (const entry of readdirSync(directory)) {
    const entryPath = path.join(directory, entry);
    const details = lstatSync(entryPath);
    if (details.isSymbolicLink()) {
      continue;
    }
    if (details.isDirectory()) {
      normalizePermissions(entryPath);
    } else {
      chmodSync(entryPath, 0o644);
    }
  }
}

async function download(url, destination) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Could not download ${url}: HTTP ${response.status}`);
  }
  writeFileSync(destination, Buffer.from(await response.arrayBuffer()));
}

function assertChecksum(file, expected, label) {
  const actual = sha256(file);
  if (actual !== expected) {
    throw new Error(
      `${label} checksum mismatch: expected ${expected}, received ${actual}`,
    );
  }
}

function assertGlibcBaseline(executable) {
  const versions = [
    ...output("readelf", ["--version-info", executable]).matchAll(
      /GLIBC_(\d+)\.(\d+)/g,
    ),
  ].map((match) => [Number(match[1]), Number(match[2])]);
  if (versions.length === 0) {
    throw new Error(
      `${path.basename(executable)} is not linked against glibc; the release target is x86-64 glibc-based Linux`,
    );
  }
  const unsupported = versions.find(
    ([major, minor]) => major > 2 || (major === 2 && minor > 35),
  );
  if (unsupported !== undefined) {
    throw new Error(
      `${path.basename(executable)} requires GLIBC_${unsupported.join(".")}; the release baseline is GLIBC_2.35`,
    );
  }
}

function runGzip(tarFile, archiveFile) {
  const archiveDescriptor = openSync(archiveFile, "w");
  try {
    const result = spawnSync(
      "gzip",
      ["--no-name", "--best", "--stdout", tarFile],
      {
        cwd: repositoryRoot,
        stdio: ["ignore", archiveDescriptor, "inherit"],
      },
    );
    assertCommandSucceeded("gzip", result);
  } finally {
    closeSync(archiveDescriptor);
  }
}

function run(executable, arguments_, currentDirectory = repositoryRoot) {
  const result = spawnSync(executable, arguments_, {
    cwd: currentDirectory,
    stdio: "inherit",
  });
  assertCommandSucceeded(executable, result);
}

function output(executable, arguments_) {
  return execFileSync(executable, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
}

function assertCommandSucceeded(executable, result) {
  if (result.error !== undefined) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${executable} exited with status ${result.status}`);
  }
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
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
