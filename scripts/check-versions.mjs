import { readFileSync } from "node:fs";
import path from "node:path";

const repositoryRoot = process.cwd();

try {
  const packageMetadata = readJson("package.json");
  const packageLock = readJson("package-lock.json");
  const bridgeMetadata = readJson("src/bridge/package.json");
  const expectedVersion = requiredString(
    packageMetadata.version,
    "package.json version",
  );
  const versions = [
    ["package-lock.json version", packageLock.version],
    ["package-lock.json root package", packageLock.packages?.[""]?.version],
    [
      "package-lock.json bridge workspace",
      packageLock.packages?.["src/bridge"]?.version,
    ],
    ["src/bridge/package.json", bridgeMetadata.version],
    [
      "src/renderer/Cargo.toml",
      tomlPackageVersion(read("src/renderer/Cargo.toml")),
    ],
    [
      "Cargo.lock roonscape-renderer package",
      capture(
        read("Cargo.lock"),
        /\[\[package\]\]\s*\nname = "roonscape-renderer"\s*\nversion = "([^"]+)"/,
        "Cargo.lock roonscape-renderer package",
      ),
    ],
    [
      "Roon extension identity",
      capture(
        read("src/bridge/src/roon-extension.ts"),
        /display_version:\s*"([^"]+)"/,
        "Roon extension identity",
      ),
    ],
    [
      "release-package expectation",
      capture(
        read("scripts/package-release.test.mjs"),
        /const expectedVersion = "([^"]+)"/,
        "release-package expectation",
      ),
    ],
  ].map(([label, version]) => [
    label,
    requiredString(version, `${label} version`),
  ]);
  const mismatches = versions.filter(
    ([, version]) => version !== expectedVersion,
  );

  if (mismatches.length > 0) {
    process.stderr.write(
      `RoonScape version mismatch; expected ${expectedVersion} from package.json:\n${mismatches
        .map(([label, version]) => `- ${label}: ${version}`)
        .join("\n")}\n`,
    );
    process.exitCode = 1;
  } else {
    process.stdout.write(`RoonScape versions match ${expectedVersion}\n`);
  }
} catch (error) {
  process.stderr.write(`RoonScape version check failed: ${error.message}\n`);
  process.exitCode = 1;
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function read(relativePath) {
  return readFileSync(path.join(repositoryRoot, relativePath), "utf8");
}

function tomlPackageVersion(contents) {
  let inPackageSection = false;
  for (const line of contents.split("\n")) {
    if (line.trim() === "[package]") {
      inPackageSection = true;
      continue;
    }
    if (inPackageSection && line.trimStart().startsWith("[")) {
      break;
    }
    if (inPackageSection) {
      const match = line.match(/^version\s*=\s*"([^"]+)"/);
      if (match !== null) {
        return match[1];
      }
    }
  }
  throw new Error("src/renderer/Cargo.toml package version is unavailable");
}

function capture(contents, pattern, label) {
  const match = contents.match(pattern);
  if (match === null) {
    throw new Error(`${label} version is unavailable`);
  }
  return match[1];
}

function requiredString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label} is unavailable`);
  }
  return value;
}
