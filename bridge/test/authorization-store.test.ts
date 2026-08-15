import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, stat } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import { FileAuthorizationStore } from "../src/authorization-store.js";

test("persists Roon authorization in a private dedicated file", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const stateDirectory = path.join(taskDirectory, "state");
  const authorizationFile = path.join(stateDirectory, "authorization.json");
  const store = new FileAuthorizationStore(authorizationFile);
  const authorization = {
    paired_core_id: "core-1",
    tokens: { "core-1": "test-token" },
  };

  try {
    assert.deepEqual(store.load(), {});
    store.save(authorization);

    assert.deepEqual(
      {
        authorization: store.load(),
        directoryMode: (await stat(stateDirectory)).mode & 0o777,
        fileMode: (await stat(authorizationFile)).mode & 0o777,
      },
      {
        authorization,
        directoryMode: 0o700,
        fileMode: 0o600,
      },
    );
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
});
