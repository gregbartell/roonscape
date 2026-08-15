import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, readFile, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import { createInterface } from "node:readline";
import path from "node:path";
import test from "node:test";

import {
  MAX_SNAPSHOT_BYTES,
  startFixturePublisher,
  type SnapshotPublisher,
} from "../src/fixture-publisher.js";
import { loadSnapshot, type PresentationSnapshot } from "../src/snapshot.js";
import { withTaskDirectory } from "./support.js";

test("sends the current complete snapshot when a renderer connects", async () => {
  await withTaskDirectory(async (runtimeDirectory) => {
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
    }

    await assert.rejects(readFile(socketPath), /ENOENT/);
  });
});

test("replaces the current snapshot without retaining event history", async () => {
  const pairingRequired = await loadSnapshot("fixtures/pairing-required.json");
  const disconnected = await loadSnapshot("fixtures/disconnected.json");

  await withPublisher(pairingRequired, async ({ publisher, socketPath }) => {
    publisher.publish(disconnected);
    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), disconnected);
    client.destroy();
  });
});

test("rejects an oversized snapshot without replacing the current snapshot", async () => {
  const snapshot = await loadSnapshot("fixtures/playing.json");

  await withPublisher(snapshot, async ({ publisher, socketPath }) => {
    assert.throws(
      () =>
        publisher.publish(withTitle(snapshot, "x".repeat(MAX_SNAPSHOT_BYTES))),
      /Snapshot exceeds 64 KiB/,
    );

    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), snapshot);
    client.destroy();
  });
});

test(
  "coalesces updates to the latest complete snapshot during backpressure",
  { timeout: 5_000 },
  async () => {
    const snapshot = await loadSnapshot("fixtures/playing.json");
    await withPublisher(snapshot, async ({ publisher, socketPath }) => {
      const client = createConnection(socketPath);
      const lines = createInterface({ input: client });
      const snapshots = lines[Symbol.asyncIterator]();

      try {
        await snapshots.next();
        client.pause();

        const finalRevision = 107;
        for (let revision = 8; revision <= finalRevision; revision += 1) {
          publisher.publish(
            withTitle(snapshot, "x".repeat(60 * 1024), revision),
          );
        }

        const freshClient = createConnection(socketPath);
        const freshLines = createInterface({ input: freshClient });
        const [freshLine] = (await once(freshLines, "line")) as [string];
        assert.equal(
          (JSON.parse(freshLine) as PresentationSnapshot).revision,
          finalRevision,
        );
        freshClient.destroy();

        client.resume();
        const observedRevisions: number[] = [];
        while (observedRevisions.at(-1) !== finalRevision) {
          const next = await snapshots.next();
          assert.equal(next.done, false);
          observedRevisions.push(
            (JSON.parse(next.value ?? "null") as PresentationSnapshot).revision,
          );
        }

        assert.ok(observedRevisions.length < 100);
      } finally {
        client.destroy();
      }
    });
  },
);

test("sends availability transitions over the current renderer connection", async () => {
  const pairingRequired = await loadSnapshot("fixtures/pairing-required.json");
  const disconnected = await loadSnapshot("fixtures/disconnected.json");
  await withPublisher(pairingRequired, async ({ publisher, socketPath }) => {
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
    }
  });
});

test("refuses a shared runtime directory without changing its permissions", async () => {
  await withTaskDirectory(async (taskDirectory) => {
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
  });
});

test("reclaims a stale socket left by an abruptly terminated publisher", async () => {
  await withTaskDirectory(async (runtimeDirectory) => {
    const socketPath = path.join(runtimeDirectory, "roonscape.sock");
    const snapshot = await loadSnapshot("fixtures/playing.json");
    const stalePublisher = spawn(
      process.execPath,
      [
        "--eval",
        "const net = require('node:net'); net.createServer().listen(process.argv[1], () => process.stdout.write('ready\\n'));",
        socketPath,
      ],
      { stdio: ["ignore", "pipe", "inherit"] },
    );
    let publisher: SnapshotPublisher | undefined;

    try {
      assert.ok(stalePublisher.stdout);
      await once(createInterface({ input: stalePublisher.stdout }), "line");
      const closed = once(stalePublisher, "close");
      stalePublisher.kill("SIGKILL");
      await closed;

      publisher = await startFixturePublisher(snapshot, socketPath);
      const client = createConnection(socketPath);
      const lines = createInterface({ input: client });
      const [line] = (await once(lines, "line")) as [string];

      assert.deepEqual(JSON.parse(line), snapshot);
      client.destroy();
    } finally {
      stalePublisher.kill("SIGKILL");
      await publisher?.close();
    }
  });
});

test("does not replace an active publisher socket", async () => {
  const snapshot = await loadSnapshot("fixtures/playing.json");
  await withPublisher(snapshot, async ({ socketPath }) => {
    await assert.rejects(
      startFixturePublisher(snapshot, socketPath),
      /EADDRINUSE/,
    );
    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), snapshot);
    client.destroy();
  });
});

interface PublisherFixture {
  publisher: SnapshotPublisher;
  socketPath: string;
}

async function withPublisher(
  snapshot: PresentationSnapshot,
  run: (fixture: PublisherFixture) => Promise<void>,
): Promise<void> {
  await withTaskDirectory(async (runtimeDirectory) => {
    const socketPath = path.join(runtimeDirectory, "roonscape.sock");
    const publisher = await startFixturePublisher(snapshot, socketPath);
    try {
      await run({ publisher, socketPath });
    } finally {
      await publisher.close();
    }
  });
}

function withTitle(
  snapshot: PresentationSnapshot,
  title: string,
  revision = snapshot.revision,
): PresentationSnapshot {
  assert.ok(snapshot.nowPlaying);
  return {
    ...snapshot,
    revision,
    nowPlaying: { ...snapshot.nowPlaying, title },
  };
}
