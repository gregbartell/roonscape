import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmod,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));

test("source launch resolves the relocated bridge and renderer", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-source-launcher-test."),
  );
  const sourceRoot = path.join(taskDirectory, "source");

  try {
    const bridgeRoot = path.join(sourceRoot, "src/bridge");
    const launcher = path.join(sourceRoot, "src/launcher/roonscape");
    const renderer = path.join(sourceRoot, "target/release/roonscape-renderer");
    const runtimeRoot = path.join(taskDirectory, "runtime");
    const configurationFile = path.join(taskDirectory, "display.json");
    const launchRecord = path.join(taskDirectory, "renderer-launch");

    await Promise.all([
      mkdir(path.join(bridgeRoot, "src"), { recursive: true }),
      mkdir(path.dirname(launcher), { recursive: true }),
      mkdir(path.dirname(renderer), { recursive: true }),
      mkdir(runtimeRoot, { mode: 0o700, recursive: true }),
      cp(
        path.join(repositoryRoot, "src/bridge/dist/src"),
        path.join(bridgeRoot, "dist/src"),
        { recursive: true },
      ),
      cp(
        path.join(repositoryRoot, "src/shared/schema"),
        path.join(sourceRoot, "src/shared/schema"),
        { recursive: true },
      ),
      cp(path.join(repositoryRoot, "src/launcher/roonscape"), launcher),
    ]);
    await Promise.all([
      symlink(
        path.join(repositoryRoot, "node_modules"),
        path.join(sourceRoot, "node_modules"),
        "dir",
      ),
      symlink(
        path.join(repositoryRoot, "src/bridge/node_modules"),
        path.join(bridgeRoot, "node_modules"),
        "dir",
      ),
      writeFile(
        path.join(sourceRoot, "package.json"),
        '{"name":"roonscape","version":"0.1.0-test","private":true,"type":"module"}\n',
      ),
      writeFile(path.join(sourceRoot, "package-lock.json"), "{}\n"),
      writeFile(configurationFile, '{"trackedOutputId":"output-test"}\n'),
      writeFile(
        renderer,
        `#!/bin/sh
for attempt in $(seq 1 500); do
  if [ -S "$ROONSCAPE_SOCKET" ]; then
    printf '%s\\n%s\\n%s\\n%s\\n' "$1" "$2" "$ROONSCAPE_SOCKET" "\${ROONSCAPE_STATIC_FIXTURE-unset}" > "$ROONSCAPE_SOURCE_LAUNCH_RECORD"
    exit 0
  fi
  sleep 0.01
done
exit 1
`,
      ),
    ]);
    await Promise.all([chmod(launcher, 0o755), chmod(renderer, 0o755)]);

    const result = spawnSync(launcher, ["--config", configurationFile], {
      cwd: sourceRoot,
      encoding: "utf8",
      env: {
        ...process.env,
        ROONSCAPE_AUTHORIZATION_FILE: path.join(
          taskDirectory,
          "authorization.json",
        ),
        ROONSCAPE_STATIC_FIXTURE: "1",
        ROONSCAPE_SOURCE_LAUNCH_RECORD: launchRecord,
        XDG_RUNTIME_DIR: runtimeRoot,
      },
      timeout: 15_000,
    });

    assert.equal(result.status, 0, processOutput(result));
    assert.deepEqual(
      (await readFile(launchRecord, "utf8")).trim().split("\n"),
      [
        "--config",
        configurationFile,
        path.join(runtimeRoot, "roonscape/roonscape.sock"),
        "unset",
      ],
    );
  } finally {
    await rm(taskDirectory, { force: true, recursive: true });
  }
});

function processOutput(result) {
  return [result.error?.message, result.stdout, result.stderr]
    .filter(Boolean)
    .join("\n");
}
