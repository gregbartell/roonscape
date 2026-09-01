import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  archiveName,
  assembleReleasePackage,
  releaseRootName,
  validateReleasePackage,
  verifyFileChecksum,
} from "./release-package.mjs";

const expectedVersion = "1.0.0";

test("controlled inputs produce a complete relocatable release", (context) => {
  const fixture = createPackagingFixture(context);
  const artifacts = assembleReleasePackage({
    inputs: fixture.inputs,
    outputDirectory: fixture.outputDirectory,
    scratchDirectory: fixture.scratchDirectory,
    sourceDateEpoch: "1700000000",
    version: expectedVersion,
  });

  assert.deepEqual(readdirSync(fixture.outputDirectory).sort(), [
    archiveName,
    `${archiveName}.sha256`,
  ]);
  const archive = readFileSync(artifacts.archiveFile);
  assert.equal(
    readFileSync(artifacts.checksumFile, "utf8"),
    `${createHash("sha256").update(archive).digest("hex")}  ${archiveName}\n`,
  );

  const extractionRoot = path.join(fixture.root, "extracted");
  mkdirSync(extractionRoot);
  const extraction = run("tar", ["-xzf", artifacts.archiveFile], {
    cwd: extractionRoot,
  });
  assert.equal(extraction.status, 0, commandOutput(extraction));
  const relocatedRoot = path.join(fixture.root, "relocated");
  renameSync(path.join(extractionRoot, releaseRootName), relocatedRoot);

  assert.deepEqual(listFiles(relocatedRoot), [
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
    "src/shared/fixtures/artwork/playing.svg",
    "src/shared/fixtures/fixture-scenario-catalog.json",
    "src/shared/fixtures/playing.json",
    "src/shared/schema/display-configuration.schema.json",
    "src/shared/schema/presentation-snapshot.schema.json",
    "target/release/roonscape-renderer",
  ]);
  assert.throws(
    () => statSync(path.join(relocatedRoot, "src/bridge/dist/src/stale.js")),
    { code: "ENOENT" },
  );

  for (const relativePath of [
    "roonscape",
    "runtime/node/bin/node",
    "target/release/roonscape-renderer",
  ]) {
    assert.equal(
      statSync(path.join(relocatedRoot, relativePath)).mode & 0o777,
      0o755,
    );
  }
  assert.equal(
    statSync(path.join(relocatedRoot, "src/bridge/dist/src/index.js")).mode &
      0o777,
    0o644,
  );

  const invocationFile = path.join(fixture.root, "node-invocation.txt");
  const launcher = run(path.join(relocatedRoot, "roonscape"), ["--help"], {
    cwd: fixture.root,
    env: { ...process.env, ROONSCAPE_TEST_INVOCATION: invocationFile },
  });
  assert.equal(launcher.status, 0, commandOutput(launcher));
  assert.equal(
    readFileSync(invocationFile, "utf8"),
    `${path.join(relocatedRoot, "src/bridge/dist/src/roonscape.js")}\n--help\n`,
  );
});

test("the assembled release passes full validation after relocation", (context) => {
  const fixture = createPackagingFixture(context);
  const artifacts = assembleFixture(fixture);
  const validated = validateReleasePackage({
    artifacts,
    commands: { readelf: fixture.readelf },
    environment: process.env,
    expectedVersion,
    scratchDirectory: path.join(fixture.root, "validation"),
  });

  assert.equal(path.basename(validated.relocatedRoot), "relocated-roonscape");
});

test("assembly rejects missing or malformed prepared inputs", (context) => {
  const missingFixture = createPackagingFixture(context);
  unlinkSync(path.join(missingFixture.inputs.bridgeBuild, "roonscape.js"));

  assert.throws(
    () => assembleFixture(missingFixture),
    /prepared RoonScape Bridge entry point is unavailable/,
  );

  const malformedFixture = createPackagingFixture(context);
  rmSync(malformedFixture.inputs.rootNodeModules, {
    force: true,
    recursive: true,
  });
  write(malformedFixture.inputs.rootNodeModules, "not a dependency tree\n");
  assert.throws(
    () => assembleFixture(malformedFixture),
    /prepared rootNodeModules is unavailable/,
  );
});

test("a Node archive checksum mismatch is rejected", (context) => {
  const fixture = createPackagingFixture(context);
  const archive = path.join(fixture.root, "node.tar.xz");
  write(archive, "controlled archive");

  assert.throws(
    () => verifyFileChecksum(archive, "0".repeat(64), "Node runtime"),
    /Node runtime checksum mismatch/,
  );
});

test("validation rejects non-glibc and unsupported executables", (context) => {
  const fixture = createPackagingFixture(context);
  const artifacts = assembleFixture(fixture);
  const nonGlibcReadelf = executable(fixture.root, "readelf-none", "exit 0\n");
  assert.throws(
    () =>
      validateReleasePackage({
        artifacts,
        commands: { readelf: nonGlibcReadelf },
        expectedVersion,
        scratchDirectory: path.join(fixture.root, "validation-none"),
      }),
    /is not linked against glibc/,
  );

  const unsupportedReadelf = executable(
    fixture.root,
    "readelf-new",
    "echo GLIBC_2.36\n",
  );
  assert.throws(
    () =>
      validateReleasePackage({
        artifacts,
        commands: { readelf: unsupportedReadelf },
        expectedVersion,
        scratchDirectory: path.join(fixture.root, "validation-new"),
      }),
    /requires GLIBC_2\.36/,
  );
});

test("archive and checksum tool failures fail without successful artifacts", (context) => {
  const archiveFixture = createPackagingFixture(context);
  const failure = executable(archiveFixture.root, "failure", "exit 23\n");
  assert.throws(
    () =>
      assembleReleasePackage({
        commands: { tar: failure },
        inputs: archiveFixture.inputs,
        outputDirectory: archiveFixture.outputDirectory,
        scratchDirectory: archiveFixture.scratchDirectory,
        sourceDateEpoch: "1700000000",
        version: expectedVersion,
      }),
    /exited with status 23/,
  );
  assert.throws(() => statSync(archiveFixture.outputDirectory), {
    code: "ENOENT",
  });

  const checksumFixture = createPackagingFixture(context);
  const artifacts = assembleFixture(checksumFixture);
  assert.throws(
    () =>
      validateReleasePackage({
        artifacts,
        commands: { readelf: checksumFixture.readelf, sha256sum: failure },
        expectedVersion,
        scratchDirectory: path.join(checksumFixture.root, "validation"),
      }),
    /exited with status 23/,
  );
});

function createPackagingFixture(context) {
  const root = mkdtempSync(path.join(tmpdir(), "roonscape-package-test."));
  context.after(() => rmSync(root, { force: true, recursive: true }));
  const inputRoot = path.join(root, "inputs");
  const outputDirectory = path.join(root, "release");
  const scratchDirectory = path.join(root, "scratch");
  const paths = {
    bridgeBuild: path.join(inputRoot, "bridge-build"),
    bridgeSource: path.join(inputRoot, "bridge-source"),
    bridgeNodeModules: path.join(inputRoot, "bridge-node-modules"),
    fonts: path.join(inputRoot, "fonts"),
    launcher: path.join(inputRoot, "roonscape"),
    nodeDistribution: path.join(inputRoot, "node"),
    renderer: path.join(inputRoot, "roonscape-renderer"),
    rootNodeModules: path.join(inputRoot, "root-node-modules"),
    shared: path.join(inputRoot, "shared"),
  };

  write(paths.launcher, readFileSync("src/launcher/roonscape"));
  write(paths.renderer, "controlled renderer\n");
  write(path.join(paths.bridgeSource, "index.ts"), "export {};\n");
  write(path.join(paths.bridgeSource, "roonscape.ts"), "export {};\n");
  write(path.join(paths.bridgeBuild, "index.js"), "export {};\n");
  write(path.join(paths.bridgeBuild, "roonscape.js"), "export {};\n");
  write(path.join(paths.bridgeBuild, "stale.js"), "throw new Error();\n");
  write(path.join(paths.rootNodeModules, "ajv-formats/package.json"), "{}\n");
  write(path.join(paths.rootNodeModules, "node-roon-api/package.json"), "{}\n");
  write(path.join(paths.bridgeNodeModules, "ajv/package.json"), "{}\n");
  write(path.join(paths.shared, "fixtures/artwork/playing.svg"), "<svg/>\n");
  write(
    path.join(paths.shared, "fixtures/fixture-scenario-catalog.json"),
    "{}\n",
  );
  write(path.join(paths.shared, "fixtures/playing.json"), "{}\n");
  write(
    path.join(paths.shared, "schema/display-configuration.schema.json"),
    "{}\n",
  );
  write(
    path.join(paths.shared, "schema/presentation-snapshot.schema.json"),
    "{}\n",
  );
  for (const font of [
    "IBM-Plex-Sans-OFL.txt",
    "IBMPlexSans-Variable.ttf",
    "Libre-Baskerville-OFL.txt",
    "LibreBaskerville-Variable.ttf",
  ]) {
    write(path.join(paths.fonts, font), "font\n");
  }
  write(path.join(paths.nodeDistribution, "LICENSE"), "Node license\n");
  write(
    path.join(paths.nodeDistribution, "bin/node"),
    `#!/bin/sh
if [ -n "\${ROONSCAPE_TEST_INVOCATION:-}" ]; then
  printf '%s\\n' "$@" > "$ROONSCAPE_TEST_INVOCATION"
fi
case "\${2:-}" in
  --help) echo 'Usage: roonscape [options]' ;;
  --version) echo 'RoonScape ${expectedVersion}' ;;
esac
`,
  );
  for (const executable of [
    paths.launcher,
    paths.renderer,
    path.join(paths.nodeDistribution, "bin/node"),
  ]) {
    chmodSync(executable, 0o755);
  }

  const readelf = executable(root, "readelf", "echo GLIBC_2.35\n");
  return {
    inputs: paths,
    outputDirectory,
    readelf,
    root,
    scratchDirectory,
  };
}

function assembleFixture(fixture) {
  return assembleReleasePackage({
    inputs: fixture.inputs,
    outputDirectory: fixture.outputDirectory,
    scratchDirectory: fixture.scratchDirectory,
    sourceDateEpoch: "1700000000",
    version: expectedVersion,
  });
}

function executable(root, name, body) {
  const file = path.join(root, "commands", name);
  write(file, `#!/bin/sh\n${body}`);
  chmodSync(file, 0o755);
  return file;
}

function write(file, contents) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, contents);
}

function run(executable, arguments_, options) {
  return spawnSync(executable, arguments_, { encoding: "utf8", ...options });
}

function commandOutput(result) {
  return [result.stdout, result.stderr].filter(Boolean).join("\n");
}

function listFiles(root) {
  const files = [];
  visit(root, "");
  return files.sort();

  function visit(directory, prefix) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const relativePath = path.join(prefix, entry.name);
      if (entry.isDirectory()) {
        visit(path.join(directory, entry.name), relativePath);
      } else {
        files.push(relativePath);
      }
    }
  }
}
