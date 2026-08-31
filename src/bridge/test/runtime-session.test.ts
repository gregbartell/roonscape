import assert from "node:assert/strict";
import { access, chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import { openRuntimeSession } from "../src/runtime-session.js";
import { withTaskDirectory } from "./support.js";

test("a matching live Runtime Owner blocks a second invocation", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    await mkdir(runtimeRoot);
    await chmod(runtimeRoot, 0o700);
    const options = {
      environment: { XDG_RUNTIME_DIR: runtimeRoot },
      processId: 1234,
      userId: process.getuid?.() ?? 1_000,
      observeProcessIdentity: () => ({
        status: "observed" as const,
        processStartTimeTicks: "987654",
      }),
    };
    const firstSession = openRuntimeSession(options);

    try {
      const persistedOwner = JSON.parse(
        await readFile(
          path.join(runtimeRoot, "roonscape/owner/session.json"),
          "utf8",
        ),
      ) as Record<string, unknown>;
      assert.deepEqual(Object.keys(persistedOwner), [
        "processId",
        "processStartTimeTicks",
        "token",
      ]);
      assert.equal(persistedOwner.processId, 1234);
      assert.equal(persistedOwner.processStartTimeTicks, "987654");
      assert.equal(typeof persistedOwner.token, "string");
      assert.throws(() => openRuntimeSession(options), /already running/);
    } finally {
      await firstSession.cleanup();
    }
  });
});

test("a missing Runtime Owner process permits stale recovery", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const artifact = path.join(runtimeDirectory, "roonscape.sock");
    await writeOwner(path.join(runtimeDirectory, "owner"), {
      processId: 4321,
      processStartTimeTicks: "owner-start",
      token: "stale-owner",
    });
    await writeFile(artifact, "stale artifact");

    const session = openRuntimeSession({
      environment,
      processId: 1234,
      userId,
      observeProcessIdentity: (processId) =>
        processId === 1234
          ? { status: "observed", processStartTimeTicks: "current-start" }
          : { status: "absent" },
    });

    try {
      await assert.rejects(access(artifact), { code: "ENOENT" });
    } finally {
      await session.cleanup();
    }
  });
});

test("a reused Runtime Owner PID permits stale recovery", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const artifact = path.join(runtimeDirectory, "roonscape.sock");
    await writeOwner(path.join(runtimeDirectory, "owner"), {
      processId: 1234,
      processStartTimeTicks: "previous-start",
      token: "stale-owner",
    });
    await writeFile(artifact, "stale artifact");

    const session = openRuntimeSession({
      environment,
      processId: 1234,
      userId,
      observeProcessIdentity: () => ({
        status: "observed",
        processStartTimeTicks: "current-start",
      }),
    });

    try {
      await assert.rejects(access(artifact), { code: "ENOENT" });
    } finally {
      await session.cleanup();
    }
  });
});

test("an unreadable Runtime Owner identity preserves runtime state", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const ownershipDirectory = path.join(runtimeDirectory, "owner");
    const artifact = path.join(runtimeDirectory, "roonscape.sock");
    await writeOwner(ownershipDirectory, {
      processId: 4321,
      processStartTimeTicks: "owner-start",
      token: "unverifiable-owner",
    });
    await writeFile(artifact, "preserve me");

    assert.throws(
      () =>
        openRuntimeSession({
          environment,
          processId: 1234,
          userId,
          observeProcessIdentity: (processId) => {
            if (processId === 1234) {
              return {
                status: "observed",
                processStartTimeTicks: "current-start",
              };
            }
            throw new Error("procfs read denied");
          },
        }),
      /identity could not be read.*preserve the runtime state/,
    );
    assert.equal(await readFile(artifact, "utf8"), "preserve me");
    assert.equal(await access(ownershipDirectory), undefined);
  });
});

test("a malformed Runtime Owner record is preserved", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const ownershipDirectory = path.join(runtimeDirectory, "owner");
    const ownershipFile = path.join(ownershipDirectory, "session.json");
    await mkdir(ownershipDirectory, { mode: 0o700 });
    await writeFile(ownershipFile, "not json\n", { mode: 0o600 });

    assert.throws(
      () => openCurrentRuntimeSession(environment, userId),
      /Cannot verify existing RoonScape runtime ownership/,
    );
    assert.equal(await readFile(ownershipFile, "utf8"), "not json\n");
  });
});

test("a PID-only Runtime Owner record is preserved as unverifiable", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const ownershipDirectory = path.join(runtimeDirectory, "owner");
    const ownershipFile = path.join(ownershipDirectory, "session.json");
    await mkdir(ownershipDirectory, { mode: 0o700 });
    await writeFile(ownershipFile, '{"processId":4321,"token":"pid-only"}\n', {
      mode: 0o600,
    });

    assert.throws(
      () => openCurrentRuntimeSession(environment, userId),
      /Cannot verify existing RoonScape runtime ownership/,
    );
    assert.equal(
      await readFile(ownershipFile, "utf8"),
      '{"processId":4321,"token":"pid-only"}\n',
    );
  });
});

test("a reused PID permits interrupted acquisition candidate recovery", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const candidateDirectory = path.join(
      runtimeDirectory,
      ".owner-candidate-interrupted",
    );
    await writeOwner(candidateDirectory, {
      processId: 1234,
      processStartTimeTicks: "previous-start",
      token: "candidate",
    });

    const session = openCurrentRuntimeSession(environment, userId);

    try {
      await assert.rejects(access(candidateDirectory), { code: "ENOENT" });
    } finally {
      await session.cleanup();
    }
  });
});

test("a live interrupted acquisition candidate blocks stale owner recovery", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const ownershipDirectory = path.join(runtimeDirectory, "owner");
    const candidateDirectory = path.join(
      runtimeDirectory,
      ".owner-candidate-interrupted",
    );
    const artifact = path.join(runtimeDirectory, "roonscape.sock");
    await writeOwner(ownershipDirectory, {
      processId: 4321,
      processStartTimeTicks: "stale-start",
      token: "stale-owner",
    });
    await writeOwner(candidateDirectory, {
      processId: 5678,
      processStartTimeTicks: "candidate-start",
      token: "candidate",
    });
    await writeFile(artifact, "preserve me");

    assert.throws(
      () =>
        openRuntimeSession({
          environment,
          processId: 1234,
          userId,
          observeProcessIdentity: (processId) => {
            if (processId === 1234) {
              return {
                status: "observed",
                processStartTimeTicks: "current-start",
              };
            }
            if (processId === 5678) {
              return {
                status: "observed",
                processStartTimeTicks: "candidate-start",
              };
            }
            return { status: "absent" };
          },
        }),
      /acquiring runtime ownership/,
    );
    assert.equal(await access(ownershipDirectory), undefined);
    assert.equal(await access(candidateDirectory), undefined);
    assert.equal(await readFile(artifact, "utf8"), "preserve me");
  });
});

test("a reused PID permits interrupted recovery owner recovery", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const recoveryDirectory = path.join(runtimeDirectory, ".recovering");
    await writeOwner(recoveryDirectory, {
      processId: 1234,
      processStartTimeTicks: "previous-start",
      token: "recovery",
    });

    const session = openCurrentRuntimeSession(environment, userId);

    try {
      await assert.rejects(access(recoveryDirectory), { code: "ENOENT" });
    } finally {
      await session.cleanup();
    }
  });
});

test("a reused PID permits interrupted recovery successor recovery", async () => {
  await withRuntimeRoot(async ({ environment, runtimeDirectory, userId }) => {
    const recoveryDirectory = path.join(runtimeDirectory, ".recovering");
    await writeOwner(recoveryDirectory, {
      processId: 1234,
      processStartTimeTicks: "previous-start",
      token: "recovery",
    });
    await writeOwner(path.join(recoveryDirectory, "successor"), {
      processId: 1234,
      processStartTimeTicks: "previous-start",
      token: "successor",
    });

    const session = openCurrentRuntimeSession(environment, userId);

    try {
      await assert.rejects(access(recoveryDirectory), { code: "ENOENT" });
    } finally {
      await session.cleanup();
    }
  });
});

interface RuntimeRoot {
  environment: NodeJS.ProcessEnv;
  runtimeDirectory: string;
  userId: number;
}

async function withRuntimeRoot(
  run: (runtimeRoot: RuntimeRoot) => Promise<void>,
): Promise<void> {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    const runtimeDirectory = path.join(runtimeRoot, "roonscape");
    await mkdir(runtimeDirectory, { mode: 0o700, recursive: true });
    await chmod(runtimeRoot, 0o700);
    await run({
      environment: { XDG_RUNTIME_DIR: runtimeRoot },
      runtimeDirectory,
      userId: process.getuid?.() ?? 1_000,
    });
  });
}

async function writeOwner(
  ownershipDirectory: string,
  owner: {
    processId: number;
    processStartTimeTicks: string;
    token: string;
  },
): Promise<void> {
  await mkdir(ownershipDirectory, { mode: 0o700, recursive: true });
  await writeFile(
    path.join(ownershipDirectory, "session.json"),
    `${JSON.stringify(owner)}\n`,
    { mode: 0o600 },
  );
}

function openCurrentRuntimeSession(
  environment: NodeJS.ProcessEnv,
  userId: number,
) {
  return openRuntimeSession({
    environment,
    processId: 1234,
    userId,
    observeProcessIdentity: () => ({
      status: "observed",
      processStartTimeTicks: "current-start",
    }),
  });
}
