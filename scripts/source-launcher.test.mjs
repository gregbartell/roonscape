import assert from "node:assert/strict";
import {
  chmod,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  startMonitoredProcess,
  stopProcess,
  waitFor,
  waitForProcessExit,
} from "./process-harness.mjs";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));

for (const captureMode of ["disabled", "enabled", "failing"]) {
  test(`source launch resolves bridge, renderer, and capture: ${captureMode}`, async () => {
    const taskDirectory = await mkdtemp(
      path.join(tmpdir(), "roonscape-source-launcher-test."),
    );
    const sourceRoot = path.join(taskDirectory, "source");

    let child;
    try {
      const bridgeRoot = path.join(sourceRoot, "src/bridge");
      const launcher = path.join(sourceRoot, "src/launcher/roonscape");
      const renderer = path.join(
        sourceRoot,
        "target/release/roonscape-renderer",
      );
      const runtimeRoot = path.join(taskDirectory, "runtime");
      const configurationFile = path.join(taskDirectory, "display.json");
      const readyMarker = path.join(taskDirectory, "bridge-ready");
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
        writeFile(
          configurationFile,
          '{"trackedOutputId":"output-test","lyricsEnabled":true,"trackedOutputName":"Test Output"}\n',
        ),
        writeFile(
          renderer,
          `#!/bin/sh
for attempt in $(seq 1 500); do
  if [ -S "$ROONSCAPE_SOCKET" ] && [ -f "$ROONSCAPE_SOURCE_LAUNCH_READY" ]; then
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

      const captureDirectory = path.join(taskDirectory, "capture");
      if (captureMode === "failing")
        await writeFile(captureDirectory, "not a directory");
      const launchArguments = [
        "--config",
        configurationFile,
        "--roon-server",
        "unavailable.invalid",
      ];
      if (captureMode !== "disabled")
        launchArguments.push(
          "--capture-bridge",
          captureDirectory,
          "--capture-budget-mib",
          "1",
        );
      child = startMonitoredProcess(launcher, launchArguments, {
        cwd: sourceRoot,
        environment: {
          ...process.env,
          ROONSCAPE_AUTHORIZATION_FILE: path.join(
            taskDirectory,
            "authorization.json",
          ),
          ROONSCAPE_STATIC_FIXTURE: "1",
          ROONSCAPE_SOURCE_LAUNCH_RECORD: launchRecord,
          ROONSCAPE_SOURCE_LAUNCH_READY: readyMarker,
          XDG_RUNTIME_DIR: runtimeRoot,
        },
      });

      await waitFor(
        () =>
          assert.match(
            child.capturedStandardOutput,
            /RoonScape Bridge listening at/,
          ),
        child,
        "Bridge readiness",
      );
      await writeFile(readyMarker, "ready");
      const [exitCode, signal] = await waitForProcessExit(child, {
        timeoutMilliseconds: 15000,
      });
      assert.equal(signal, null);
      assert.equal(exitCode, 0, child.capturedStandardError);
      if (captureMode === "enabled") {
        const files = (await readdir(captureDirectory))
          .filter((name) => name.endsWith(".jsonl"))
          .sort();
        const records = (
          await Promise.all(
            files.map((name) =>
              readFile(path.join(captureDirectory, name), "utf8"),
            ),
          )
        )
          .join("")
          .trim()
          .split("\n")
          .map((line) => JSON.parse(line));
        assert.equal(
          records.find((record) => record.type === "session").data.budgetBytes,
          1048576,
        );
        assert.equal(
          records.find((record) => record.type === "snapshot").data.snapshot
            .availability,
          "pairingRequired",
        );
        assert.ok(
          records.some(
            (record) =>
              record.type === "discovery" &&
              record.data.roonServerHost === "unavailable.invalid",
          ),
        );
      } else if (captureMode === "failing") {
        assert.match(
          child.capturedStandardError,
          /RoonScape capture: destination\/write failure/,
        );
        assert.equal(
          await readFile(captureDirectory, "utf8"),
          "not a directory",
        );
      }
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
      await stopProcess(child);
      await rm(taskDirectory, { force: true, recursive: true });
    }
  });
}
