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

test("replaces the current snapshot without retaining event history", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const runtimeDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const socketPath = path.join(runtimeDirectory, "roonscape.sock");
  const pairingRequired = await loadSnapshot("fixtures/pairing-required.json");
  const disconnected = await loadSnapshot("fixtures/disconnected.json");
  const publisher = await startFixturePublisher(pairingRequired, socketPath);

  try {
    publisher.publish(disconnected);
    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), disconnected);
    client.destroy();
  } finally {
    await publisher.close();
    await rm(runtimeDirectory, { recursive: true });
  }
});

test("sends availability transitions over the current renderer connection", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const runtimeDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const socketPath = path.join(runtimeDirectory, "roonscape.sock");
  const pairingRequired = await loadSnapshot("fixtures/pairing-required.json");
  const disconnected = await loadSnapshot("fixtures/disconnected.json");
  const publisher = await startFixturePublisher(pairingRequired, socketPath);
  const client = createConnection(socketPath);
  const lines = createInterface({ input: client });
  const snapshots = lines[Symbol.asyncIterator]();

  try {
    const initial = await snapshots.next();
    publisher.publish(disconnected);
    const transition = await snapshots.next();

    assert.deepEqual(
      [initial, transition].map(({ value }) =>
        value === undefined ? undefined : JSON.parse(value),
      ),
      [pairingRequired, disconnected],
    );
  } finally {
    client.destroy();
    await publisher.close();
    await rm(runtimeDirectory, { recursive: true });
  }
});

test("refuses a shared runtime directory without changing its permissions", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const sharedDirectory = path.join(taskDirectory, "shared");
  const socketPath = path.join(sharedDirectory, "roonscape.sock");
  const snapshot = await loadSnapshot("fixtures/pairing-required.json");
  await mkdir(sharedDirectory, { mode: 0o755 });
  let startError: unknown;

  try {
    const publisher = await startFixturePublisher(snapshot, socketPath);
    await publisher.close();
  } catch (error) {
    startError = error;
  }

  try {
    assert.deepEqual(
      {
        error: startError instanceof Error ? startError.message : undefined,
        mode: (await stat(sharedDirectory)).mode & 0o777,
      },
      {
        error: `Runtime directory must be private: ${sharedDirectory}`,
        mode: 0o755,
      },
    );
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
});
