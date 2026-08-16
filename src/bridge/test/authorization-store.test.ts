import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, stat } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  FileAuthorizationStore,
  authorizationFilePath,
} from "../src/authorization-store.js";

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

test("uses XDG state independently from Display Configuration", () => {
  assert.equal(
    authorizationFilePath({
      HOME: "/home/owner",
      ROONSCAPE_DISPLAY_CONFIG: "/provided/display.json",
      XDG_CONFIG_HOME: "/var/config",
      XDG_STATE_HOME: "/var/state",
    }),
    "/var/state/roonscape/authorization.json",
  );
});
