import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  closeSync,
  copyFileSync,
  cpSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

export const releaseRootName = "roonscape";
export const archiveName = "roonscape-linux-x64.tar.gz";
const executablePaths = [
  "roonscape",
  "runtime/node/bin/node",
  "target/release/roonscape-renderer",
];

export function assembleReleasePackage({
  commands = {},
  inputs,
  outputDirectory,
  scratchDirectory,
  sourceDateEpoch,
  version,
}) {
  assertPreparedInputs(inputs);
  requiredString(outputDirectory, "output directory");
  requiredString(scratchDirectory, "scratch directory");
  requiredString(sourceDateEpoch, "source date epoch");
  requiredString(version, "package version");

  rmSync(scratchDirectory, { force: true, recursive: true });
  mkdirSync(scratchDirectory, { recursive: true });
  const stageRoot = path.join(scratchDirectory, "stage", releaseRootName);

  copyFile(inputs.launcher, path.join(stageRoot, "roonscape"));
  copyBridgeJavaScript(
    inputs.bridgeSource,
    inputs.bridgeBuild,
    path.join(stageRoot, "src/bridge/dist/src"),
  );
  copyTree(
    inputs.rootNodeModules,
    path.join(stageRoot, "node_modules"),
    (candidate) => {
      const relativePath = path.relative(inputs.rootNodeModules, candidate);
      return (
        relativePath !== ".bin" &&
        !relativePath.startsWith(`.bin${path.sep}`) &&
        relativePath !== path.join("@roonscape", "bridge") &&
        !relativePath.startsWith(
          `${path.join("@roonscape", "bridge")}${path.sep}`,
        )
      );
    },
  );
  copyTree(
    inputs.bridgeNodeModules,
    path.join(stageRoot, "src/bridge/node_modules"),
  );
  copyTree(inputs.shared, path.join(stageRoot, "src/shared"));
  copyTree(inputs.fonts, path.join(stageRoot, "src/renderer/assets/fonts"));
  copyFile(
    inputs.renderer,
    path.join(stageRoot, "target/release/roonscape-renderer"),
  );
  copyFile(
    path.join(inputs.nodeDistribution, "bin/node"),
    path.join(stageRoot, "runtime/node/bin/node"),
  );
  copyFile(
    path.join(inputs.nodeDistribution, "LICENSE"),
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

  normalizePermissions(stageRoot);
  for (const relativePath of executablePaths) {
    chmodSync(path.join(stageRoot, relativePath), 0o755);
  }

  const tarFile = path.join(scratchDirectory, `${releaseRootName}.tar`);
  run(commands.tar ?? "tar", [
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
  const preparedArchive = path.join(scratchDirectory, archiveName);
  runGzip(commands.gzip ?? "gzip", tarFile, preparedArchive);
  const checksum = sha256(preparedArchive);
  const preparedChecksum = `${preparedArchive}.sha256`;
  writeFileSync(preparedChecksum, `${checksum}  ${archiveName}\n`);

  rmSync(outputDirectory, { force: true, recursive: true });
  mkdirSync(outputDirectory, { recursive: true });
  const archiveFile = path.join(outputDirectory, archiveName);
  const checksumFile = `${archiveFile}.sha256`;
  const incompleteArchive = `${archiveFile}.incomplete`;
  const incompleteChecksum = `${checksumFile}.incomplete`;
  copyFileSync(preparedArchive, incompleteArchive);
  copyFileSync(preparedChecksum, incompleteChecksum);
  renameSync(incompleteChecksum, checksumFile);
  renameSync(incompleteArchive, archiveFile);
  return { archiveFile, checksumFile };
}

export function validateReleasePackage({
  artifacts,
  commands = {},
  environment = process.env,
  expectedVersion,
  scratchDirectory,
}) {
  const archiveFile = artifacts?.archiveFile;
  const checksumFile = artifacts?.checksumFile;
  assertFile(archiveFile, "release archive");
  assertFile(checksumFile, "release checksum");
  if (path.basename(archiveFile) !== archiveName) {
    throw new Error(
      `Unexpected release archive name: ${path.basename(archiveFile)}`,
    );
  }
  if (path.basename(checksumFile) !== `${archiveName}.sha256`) {
    throw new Error(
      `Unexpected release checksum name: ${path.basename(checksumFile)}`,
    );
  }
  requiredString(expectedVersion, "expected package version");
  requiredString(scratchDirectory, "validation scratch directory");

  const checksumContents = readFileSync(checksumFile, "utf8");
  if (
    !/^[a-f0-9]{64} {2}roonscape-linux-x64\.tar\.gz\n$/.test(checksumContents)
  ) {
    throw new Error(
      "Release checksum has an unexpected format or archive name",
    );
  }
  run(
    commands.sha256sum ?? "sha256sum",
    ["--check", path.basename(checksumFile)],
    {
      cwd: path.dirname(checksumFile),
      env: environment,
    },
  );
  rmSync(scratchDirectory, { force: true, recursive: true });
  mkdirSync(scratchDirectory, { recursive: true });
  const listing = output(commands.tar ?? "tar", ["-tzf", archiveFile], {
    env: environment,
  });
  const archiveEntries = listing.trim().split("\n");
  if (
    archiveEntries.length === 0 ||
    archiveEntries.some(
      (entry) =>
        entry !== releaseRootName && !entry.startsWith(`${releaseRootName}/`),
    )
  ) {
    throw new Error(`Release archive must contain only ${releaseRootName}`);
  }
  run(commands.tar ?? "tar", ["-xzf", archiveFile, "-C", scratchDirectory], {
    env: environment,
  });
  const releaseRoot = path.join(scratchDirectory, releaseRootName);
  const relocatedRoot = path.join(scratchDirectory, "relocated-roonscape");
  renameSync(releaseRoot, relocatedRoot);

  const requiredPaths = [
    "node_modules/ajv-formats/package.json",
    "node_modules/node-roon-api/package.json",
    "package.json",
    "roonscape",
    "runtime/node/LICENSE",
    "runtime/node/bin/node",
    "src/bridge/dist/src/index.js",
    "src/bridge/dist/src/roonscape.js",
    "src/bridge/node_modules/ajv/package.json",
    "src/renderer/assets/fonts/IBM-Plex-Sans-OFL.txt",
    "src/renderer/assets/fonts/IBMPlexSans-Variable.ttf",
    "src/renderer/assets/fonts/Libre-Baskerville-OFL.txt",
    "src/renderer/assets/fonts/LibreBaskerville-Variable.ttf",
    "src/shared/fixtures/artwork/playing.jpg",
    "src/shared/fixtures/fixture-scenario-catalog.json",
    "src/shared/fixtures/playing.json",
    "src/shared/schema/display-configuration.schema.json",
    "src/shared/schema/presentation-snapshot.schema.json",
    "target/release/roonscape-renderer",
  ];
  for (const relativePath of requiredPaths) {
    assertFile(
      path.join(relocatedRoot, relativePath),
      `packaged ${relativePath}`,
    );
  }
  const metadata = JSON.parse(
    readFileSync(path.join(relocatedRoot, "package.json"), "utf8"),
  );
  if (metadata.version !== expectedVersion) {
    throw new Error(
      `Packaged version mismatch: expected ${expectedVersion}, received ${metadata.version}`,
    );
  }
  assertNormalizedPermissions(relocatedRoot, new Set(executablePaths));
  assertGlibcBaseline(
    path.join(relocatedRoot, "target/release/roonscape-renderer"),
    commands.readelf ?? "readelf",
    environment,
  );
  assertGlibcBaseline(
    path.join(relocatedRoot, "runtime/node/bin/node"),
    commands.readelf ?? "readelf",
    environment,
  );

  const launcher = path.join(relocatedRoot, "roonscape");
  const help = output(launcher, ["--help"], {
    cwd: scratchDirectory,
    env: environment,
  });
  if (!help.startsWith("Usage: roonscape ")) {
    throw new Error("Packaged launcher did not return RoonScape help");
  }
  const version = output(launcher, ["--version"], {
    cwd: scratchDirectory,
    env: environment,
  });
  if (version !== `RoonScape ${expectedVersion}\n`) {
    throw new Error(
      `Packaged launcher returned an unexpected version: ${version.trim()}`,
    );
  }
  return { relocatedRoot };
}

function assertNormalizedPermissions(root, executablePaths) {
  visit(root, "");

  function visit(directory, relativeDirectory) {
    assertMode(directory, relativeDirectory || releaseRootName, 0o755);
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const relativePath = path.join(relativeDirectory, entry.name);
      const entryPath = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) continue;
      if (entry.isDirectory()) visit(entryPath, relativePath);
      else {
        assertMode(
          entryPath,
          relativePath,
          executablePaths.has(relativePath) ? 0o755 : 0o644,
        );
      }
    }
  }
}

function assertMode(candidate, label, expected) {
  const mode = statSync(candidate).mode & 0o777;
  if (mode !== expected) {
    throw new Error(
      `${label} has mode ${mode.toString(8)}; expected ${expected.toString(8)}`,
    );
  }
}

export function verifyFileChecksum(file, expected, label) {
  const actual = sha256(file);
  if (actual !== expected) {
    throw new Error(
      `${label} checksum mismatch: expected ${expected}, received ${actual}`,
    );
  }
}

function assertPreparedInputs(inputs) {
  if (inputs === null || typeof inputs !== "object") {
    throw new Error("RoonScape prepared package inputs are unavailable");
  }
  for (const name of ["launcher", "renderer"]) {
    assertFile(inputs[name], `prepared ${name}`);
  }
  for (const name of [
    "bridgeBuild",
    "bridgeSource",
    "bridgeNodeModules",
    "fonts",
    "nodeDistribution",
    "rootNodeModules",
    "shared",
  ]) {
    assertDirectory(inputs[name], `prepared ${name}`);
  }
  assertFile(
    path.join(inputs.bridgeBuild, "roonscape.js"),
    "prepared RoonScape Bridge entry point",
  );
  assertFile(
    path.join(inputs.nodeDistribution, "bin/node"),
    "prepared Node executable",
  );
  assertFile(
    path.join(inputs.nodeDistribution, "LICENSE"),
    "prepared Node license",
  );
}

function assertFile(candidate, label) {
  try {
    if (typeof candidate === "string" && statSync(candidate).isFile()) return;
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  throw new Error(`RoonScape ${label} is unavailable`);
}

function assertDirectory(candidate, label) {
  try {
    if (typeof candidate === "string" && statSync(candidate).isDirectory())
      return;
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  throw new Error(`RoonScape ${label} is unavailable`);
}

function copyBridgeJavaScript(sourceRoot, buildRoot, destinationRoot) {
  visit(sourceRoot, "");

  function visit(directory, relativeDirectory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const relativePath = path.join(relativeDirectory, entry.name);
      if (entry.isDirectory()) {
        visit(path.join(directory, entry.name), relativePath);
      } else if (entry.name.endsWith(".ts") && !entry.name.endsWith(".d.ts")) {
        const outputPath = relativePath.replace(/\.ts$/, ".js");
        copyFile(
          path.join(buildRoot, outputPath),
          path.join(destinationRoot, outputPath),
        );
      }
    }
  }
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
    if (details.isSymbolicLink()) continue;
    if (details.isDirectory()) normalizePermissions(entryPath);
    else chmodSync(entryPath, 0o644);
  }
}

function assertGlibcBaseline(executable, readelf, environment) {
  const versions = [
    ...output(readelf, ["--version-info", executable], {
      env: environment,
    }).matchAll(/GLIBC_(\d+)\.(\d+)/g),
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

function runGzip(gzip, tarFile, archiveFile) {
  const archiveDescriptor = openSync(archiveFile, "w");
  try {
    const result = spawnSync(
      gzip,
      ["--no-name", "--best", "--stdout", tarFile],
      { stdio: ["ignore", archiveDescriptor, "inherit"] },
    );
    assertCommandSucceeded(gzip, result);
  } finally {
    closeSync(archiveDescriptor);
  }
}

function run(executable, arguments_, options = {}) {
  const result = spawnSync(executable, arguments_, {
    stdio: "inherit",
    ...options,
  });
  assertCommandSucceeded(executable, result);
}

function output(executable, arguments_, options = {}) {
  const result = spawnSync(executable, arguments_, {
    encoding: "utf8",
    ...options,
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

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function requiredString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`RoonScape ${label} is unavailable`);
  }
  return value;
}
