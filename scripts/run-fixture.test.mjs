import assert from "node:assert/strict";
import { once } from "node:events";
import {
  access,
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const scratchRoot = "/tmp/codex/roonscape";

test("a clean renderer exit stops the fixture publisher and removes runtime state", async () => {
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const binDirectory = path.join(taskDirectory, "bin");
  const socketPathRecord = path.join(taskDirectory, "socket-path");
  await mkdir(binDirectory);

  const cargoStub = path.join(binDirectory, "cargo");
  await writeFile(
    cargoStub,
    '#!/bin/sh\nprintf "%s\\n" "$ROONSCAPE_SOCKET" > "$ROONSCAPE_FIXTURE_TEST_SOCKET_PATH"\n',
  );
  await chmod(cargoStub, 0o755);

  const launcher = spawn(process.execPath, ["scripts/run-fixture.mjs"], {
    cwd: repositoryRoot,
    detached: true,
    env: {
      ...process.env,
      PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
      ROONSCAPE_FIXTURE_TEST_SOCKET_PATH: socketPathRecord,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  assert.notEqual(launcher.pid, undefined);

  let standardOutput = "";
  let standardError = "";
  launcher.stdout.setEncoding("utf8");
  launcher.stderr.setEncoding("utf8");
  launcher.stdout.on("data", (chunk) => {
    standardOutput += chunk;
  });
  launcher.stderr.on("data", (chunk) => {
    standardError += chunk;
  });

  try {
    const [exitCode, signal] = await once(launcher, "exit");
    assert.equal(
      exitCode,
      0,
      `fixture launcher exited via ${signal ?? "no signal"}\n${standardOutput}${standardError}`,
    );

    const socketPath = (await readFile(socketPathRecord, "utf8")).trim();
    const runtimeDirectory = path.dirname(socketPath);
    await assert.rejects(access(runtimeDirectory), { code: "ENOENT" });
    assert.throws(() => process.kill(-launcher.pid, 0), { code: "ESRCH" });
  } finally {
    stopProcessGroup(launcher.pid);
    await rm(taskDirectory, { recursive: true });
  }
});

function stopProcessGroup(processId) {
  try {
    process.kill(-processId, "SIGKILL");
  } catch (error) {
    if (error.code !== "ESRCH") {
      throw error;
    }
  }
}
