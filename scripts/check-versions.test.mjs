import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const versionCheck = path.join(repositoryRoot, "scripts/check-versions.mjs");

test("the repository version check accepts matching release versions", () => {
  withRepository("1.2.3", {}, (fixtureRoot) => {
    const result = runVersionCheck(fixtureRoot);

    assert.equal(result.status, 0, processOutput(result));
    assert.equal(result.stdout, "RoonScape versions match 1.2.3\n");
    assert.equal(result.stderr, "");
  });
});

test("the repository version check reports every release version mismatch", () => {
  withRepository(
    "1.2.3",
    {
      "src/bridge/package.json": json({
        name: "@roonscape/bridge",
        version: "1.2.4",
      }),
      "src/renderer/Cargo.toml":
        '[package]\nname = "roonscape-renderer"\nversion = "2.0.0"\n',
    },
    (fixtureRoot) => {
      const result = runVersionCheck(fixtureRoot);

      assert.equal(result.status, 1, processOutput(result));
      assert.equal(result.stdout, "");
      assert.equal(
        result.stderr,
        "RoonScape version mismatch; expected 1.2.3 from package.json:\n" +
          "- src/bridge/package.json: 1.2.4\n" +
          "- src/renderer/Cargo.toml: 2.0.0\n",
      );
    },
  );
});

function withRepository(version, overrides, run) {
  const fixtureRoot = mkdtempSync(
    path.join(tmpdir(), "roonscape-version-check."),
  );
  const files = {
    "package.json": json({ name: "roonscape", version }),
    "package-lock.json": json({
      name: "roonscape",
      version,
      packages: {
        "": { name: "roonscape", version },
        "src/bridge": { name: "@roonscape/bridge", version },
      },
    }),
    "src/bridge/package.json": json({
      name: "@roonscape/bridge",
      version,
    }),
    "src/renderer/Cargo.toml": `[package]\nname = "roonscape-renderer"\nversion = "${version}"\n`,
    "Cargo.lock": `[[package]]\nname = "roonscape-renderer"\nversion = "${version}"\n`,
    "src/bridge/src/roon-extension.ts": `export const identity = { display_version: "${version}" };\n`,
    "scripts/package-release.test.mjs": `const expectedVersion = "${version}";\n`,
    ...overrides,
  };

  try {
    for (const [relativePath, contents] of Object.entries(files)) {
      const file = path.join(fixtureRoot, relativePath);
      mkdirSync(path.dirname(file), { recursive: true });
      writeFileSync(file, contents);
    }
    run(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { force: true, recursive: true });
  }
}

function runVersionCheck(fixtureRoot) {
  return spawnSync(process.execPath, [versionCheck], {
    cwd: fixtureRoot,
    encoding: "utf8",
  });
}

function json(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function processOutput(result) {
  return [result.stdout, result.stderr].filter(Boolean).join("\n");
}
