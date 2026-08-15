import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import { createInterface } from "node:readline";
import path from "node:path";
import test from "node:test";

import { startFixturePublisher } from "../src/fixture-publisher.js";
import { loadSnapshot } from "../src/snapshot.js";

test("sends the current complete snapshot when a renderer connects", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const runtimeDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const socketPath = path.join(runtimeDirectory, "roonscape.sock");
  const snapshot = await loadSnapshot("fixtures/playing.json");
  const publisher = await startFixturePublisher(snapshot, socketPath);

  try {
    assert.equal((await stat(runtimeDirectory)).mode & 0o777, 0o700);

    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), snapshot);
    client.destroy();
  } finally {
    await publisher.close();
    await rm(runtimeDirectory, { recursive: true });
  }

  await assert.rejects(readFile(socketPath), /ENOENT/);
});
