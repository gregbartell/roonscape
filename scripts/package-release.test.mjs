import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const packageMetadata = JSON.parse(
  readFileSync(path.join(repositoryRoot, "package.json"), "utf8"),
);
const releaseName = `roonscape-${packageMetadata.version}-linux-x64`;
const archiveFile = path.join(
  repositoryRoot,
  "release",
  `${releaseName}.tar.gz`,
);
const checksumFile = `${archiveFile}.sha256`;

test(
  "the Linux release is complete, relocatable, and uses its private Node runtime",
  { timeout: 300_000 },
  (context) => {
    const staleBuildOutput = path.join(
      repositoryRoot,
      "bridge/dist/src/stale-package-output.js",
    );
    mkdirSync(path.dirname(staleBuildOutput), { recursive: true });
    writeFileSync(staleBuildOutput, "throw new Error('stale build output');\n");
    context.after(() => rmSync(staleBuildOutput, { force: true }));

    const packaging = run(process.execPath, ["scripts/package-release.mjs"], {
      cwd: repositoryRoot,
    });
    assert.equal(packaging.status, 0, packagingOutput(packaging));

    const archive = readFileSync(archiveFile);
    const expectedChecksum = createHash("sha256").update(archive).digest("hex");
    assert.equal(
      readFileSync(checksumFile, "utf8"),
      `${expectedChecksum}  ${path.basename(archiveFile)}\n`,
    );

    const scratchRoot = path.join(tmpdir(), "codex", "roonscape");
    mkdirSync(scratchRoot, { recursive: true });
    const extractionRoot = mkdtempSync(path.join(scratchRoot, "release-test."));
    try {
      const extraction = run("tar", ["-xzf", archiveFile], {
        cwd: extractionRoot,
      });
      assert.equal(extraction.status, 0, packagingOutput(extraction));

      const releaseRoot = path.join(extractionRoot, releaseName);
      const relocatedRoot = path.join(extractionRoot, "relocated-roonscape");
      renameSync(releaseRoot, relocatedRoot);
      assert.throws(
        () =>
          statSync(
            path.join(relocatedRoot, "bridge/dist/src/stale-package-output.js"),
          ),
        { code: "ENOENT" },
      );

      for (const relativePath of [
        "bridge/dist/src/index.js",
        "bridge/dist/src/roonscape.js",
        "bridge/node_modules/ajv/package.json",
        "node_modules/ajv-formats/package.json",
        "node_modules/node-roon-api/package.json",
        "package.json",
        "renderer/assets/fonts/IBM-Plex-Sans-OFL.txt",
        "renderer/assets/fonts/IBMPlexSans-Variable.ttf",
        "renderer/assets/fonts/Libre-Baskerville-OFL.txt",
        "renderer/assets/fonts/LibreBaskerville-Variable.ttf",
        "runtime/node/LICENSE",
        "schema/display-configuration.schema.json",
        "schema/presentation-snapshot.schema.json",
      ]) {
        assert.ok(
          statSync(path.join(relocatedRoot, relativePath)).isFile(),
          `${relativePath} should be packaged`,
        );
      }

      for (const relativePath of [
        "roonscape",
        "runtime/node/bin/node",
        "target/release/roonscape-renderer",
      ]) {
        const mode = statSync(path.join(relocatedRoot, relativePath)).mode;
        assert.notEqual(
          mode & 0o111,
          0,
          `${relativePath} should be executable`,
        );
      }

      const commandPath = path.join(extractionRoot, "command-path");
      mkdirSync(commandPath);
      for (const command of ["dirname", "readlink"]) {
        symlinkSync(`/usr/bin/${command}`, path.join(commandPath, command));
      }
      chmodSync(commandPath, 0o755);

      const environment = { ...process.env, PATH: commandPath };
      const command = path.join(relocatedRoot, "roonscape");
      const help = run(command, ["--help"], {
        cwd: extractionRoot,
        env: environment,
      });
      assert.equal(help.status, 0, packagingOutput(help));
      assert.match(help.stdout, /^Usage: roonscape /);

      const version = run(command, ["--version"], {
        cwd: extractionRoot,
        env: environment,
      });
      assert.equal(version.status, 0, packagingOutput(version));
      assert.equal(version.stdout, `RoonScape ${packageMetadata.version}\n`);
    } finally {
      rmSync(extractionRoot, { force: true, recursive: true });
    }
  },
);

test(
  "packaging rejects a renderer that does not target glibc",
  { timeout: 300_000 },
  (context) => {
    const fakeCommandDirectory = mkdtempSync(
      path.join(tmpdir(), "roonscape-package-commands."),
    );
    context.after(() =>
      rmSync(fakeCommandDirectory, { force: true, recursive: true }),
    );
    const fakeReadelf = path.join(fakeCommandDirectory, "readelf");
    writeFileSync(fakeReadelf, "#!/bin/sh\nexit 0\n");
    chmodSync(fakeReadelf, 0o755);

    const packaging = run(process.execPath, ["scripts/package-release.mjs"], {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        PATH: `${fakeCommandDirectory}:${process.env.PATH ?? ""}`,
      },
    });

    assert.notEqual(packaging.status, 0, packagingOutput(packaging));
    assert.match(packaging.stderr, /is not linked against glibc/);
  },
);

function run(executable, arguments_, options) {
  return spawnSync(executable, arguments_, {
    encoding: "utf8",
    ...options,
  });
}

function packagingOutput(result) {
  return [result.stdout, result.stderr].filter(Boolean).join("\n");
}
