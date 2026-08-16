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
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath, processId } =
      await launchFixture(taskDirectory);
    const runtimeDirectory = path.dirname(socketPath);

    assert.equal(path.dirname(controlSocketPath), runtimeDirectory);
    assert.equal(path.basename(controlSocketPath), "fixture-navigation.sock");
    await assert.rejects(access(runtimeDirectory), { code: "ENOENT" });
    assert.throws(() => process.kill(-processId, 0), { code: "ESRCH" });
  });
});

test("an explicit single-fixture session does not activate navigation", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath } = await launchFixture(
      taskDirectory,
      {
        ROONSCAPE_FIXTURE: "fixtures/paused.json",
        ROONSCAPE_FIXTURE_CONTROL: path.join(taskDirectory, "inherited.sock"),
      },
    );

    assert.equal(controlSocketPath, "unset");
    await assert.rejects(access(path.dirname(socketPath)), { code: "ENOENT" });
  });
});

async function withTaskDirectory(run) {
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  try {
    await run(taskDirectory);
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
}

async function launchFixture(taskDirectory, environmentOverrides = {}) {
  const binDirectory = path.join(taskDirectory, "bin");
  const environmentRecord = path.join(taskDirectory, "renderer-environment");
  await mkdir(binDirectory);

  const cargoStub = path.join(binDirectory, "cargo");
  await writeFile(
    cargoStub,
    '#!/bin/sh\nprintf "%s\\n%s\\n" "$ROONSCAPE_SOCKET" "${ROONSCAPE_FIXTURE_CONTROL-unset}" > "$ROONSCAPE_FIXTURE_TEST_ENVIRONMENT"\n',
  );
  await chmod(cargoStub, 0o755);

  const launcher = spawn(process.execPath, ["scripts/run-fixture.mjs"], {
    cwd: repositoryRoot,
    detached: true,
    env: {
      ...process.env,
      ...environmentOverrides,
      PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
      ROONSCAPE_FIXTURE_TEST_ENVIRONMENT: environmentRecord,
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

    const [socketPath, controlSocketPath] = (
      await readFile(environmentRecord, "utf8")
    )
      .trimEnd()
      .split("\n");
    assert.ok(socketPath);
    assert.ok(controlSocketPath);
    return { socketPath, controlSocketPath, processId: launcher.pid };
  } finally {
    stopProcessGroup(launcher.pid);
  }
}

function stopProcessGroup(processId) {
  try {
    process.kill(-processId, "SIGKILL");
  } catch (error) {
    if (error.code !== "ESRCH") {
      throw error;
    }
  }
}
