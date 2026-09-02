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
  startSnapshotPublisher,
  type SnapshotPublisher,
} from "../src/fixture-publisher.js";
import { loadSnapshot, type PresentationSnapshot } from "../src/snapshot.js";
import { withTaskDirectory } from "./support.js";

test("sends the current complete snapshot when a renderer connects", async () => {
  await withTaskDirectory(async (runtimeDirectory) => {
    const socketPath = path.join(runtimeDirectory, "roonscape.sock");
    const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
    const publisher = await startSnapshotPublisher(snapshot, socketPath);

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

test("accepts lyric visibility only when a renderer reports its revision", async () => {
  await withTaskDirectory(async (runtimeDirectory) => {
    const socketPath = path.join(runtimeDirectory, "roonscape.sock");
    const snapshot = await loadSnapshot(
      "src/shared/fixtures/lyrics-one-line.json",
    );
    const visibleRevisions: number[] = [];
    let reportVisibility: (() => void) | undefined;
    const visibilityReported = new Promise<void>((resolve) => {
      reportVisibility = resolve;
    });
    const publisher = await startSnapshotPublisher(snapshot, socketPath, {
      onLyricsVisible: (revision) => {
        visibleRevisions.push(revision);
        reportVisibility?.();
      },
    });

    try {
      assert.deepEqual(visibleRevisions, []);
      const client = createConnection(socketPath);
      const lines = createInterface({ input: client });
      await once(lines, "line");
      assert.deepEqual(visibleRevisions, []);
      client.write(
        `${JSON.stringify({ type: "lyricsVisible", revision: snapshot.revision })}\n`,
      );
      await visibilityReported;
      assert.deepEqual(visibleRevisions, [snapshot.revision]);
      client.destroy();
    } finally {
      await publisher.close();
    }
  });
});

test("accepts coalesced bounded visibility reports without closing snapshots", async () => {
  await withTaskDirectory(async (runtimeDirectory) => {
    const socketPath = path.join(runtimeDirectory, "roonscape.sock");
    const snapshot = await loadSnapshot(
      "src/shared/fixtures/lyrics-one-line.json",
    );
    const visibleRevisions: number[] = [];
    let reportsComplete: (() => void) | undefined;
    const allReports = new Promise<void>((resolve) => {
      reportsComplete = resolve;
    });
    const publisher = await startSnapshotPublisher(snapshot, socketPath, {
      onLyricsVisible: (revision) => {
        visibleRevisions.push(revision);
        if (visibleRevisions.length === 40) {
          reportsComplete?.();
        }
      },
    });

    try {
      const client = createConnection(socketPath);
      const lines = createInterface({ input: client });
      await once(lines, "line");
      client.write(
        Array.from(
          { length: 40 },
          () =>
            `${JSON.stringify({ type: "lyricsVisible", revision: snapshot.revision })}\n`,
        ).join(""),
      );
      await Promise.race([
        allReports,
        new Promise<never>((_, reject) =>
          setTimeout(
            () => reject(new Error("visibility reports timed out")),
            1_000,
          ),
        ),
      ]);

      const nextSnapshot = { ...snapshot, revision: snapshot.revision + 1 };
      publisher.publish(nextSnapshot);
      const [line] = (await once(lines, "line")) as [string];
      assert.deepEqual(JSON.parse(line), nextSnapshot);
      assert.equal(visibleRevisions.length, 40);
      client.destroy();
    } finally {
      await publisher.close();
    }
  });
});

test("re-anchors Playing at fixture launch before using the shared publisher", async () => {
  await withTaskDirectory(async (runtimeDirectory) => {
    const socketPath = path.join(runtimeDirectory, "roonscape.sock");
    const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
    const publisher = await startFixturePublisher(
      snapshot,
      socketPath,
      new Date("2030-01-02T03:04:05Z"),
    );

    try {
      const client = createConnection(socketPath);
      const lines = createInterface({ input: client });
      const [line] = (await once(lines, "line")) as [string];
      const published = JSON.parse(line) as PresentationSnapshot;

      assert.deepEqual(published.timing, {
        position: {
          seconds: 171,
          sampledAt: "2030-01-02T03:04:05.000Z",
        },
        durationSeconds: 266,
      });
      assert.equal(
        snapshot.timing?.position?.sampledAt,
        "2026-08-15T19:20:00Z",
      );
      client.destroy();
    } finally {
      await publisher.close();
    }
  });
});

test("does not re-anchor Paused or Starting fixture timing", async () => {
  for (const fixtureName of ["paused.json", "loading.json"]) {
    await withTaskDirectory(async (runtimeDirectory) => {
      const socketPath = path.join(runtimeDirectory, "roonscape.sock");
      const snapshot = await loadSnapshot(`src/shared/fixtures/${fixtureName}`);
      const publisher = await startFixturePublisher(
        snapshot,
        socketPath,
        new Date("2030-01-02T03:04:05Z"),
      );

      try {
        const client = createConnection(socketPath);
        const lines = createInterface({ input: client });
        const [line] = (await once(lines, "line")) as [string];
        const published = JSON.parse(line) as PresentationSnapshot;

        assert.deepEqual(published.timing, snapshot.timing);
        client.destroy();
      } finally {
        await publisher.close();
      }
    });
  }
});

test("replaces the current snapshot without retaining event history", async () => {
  const pairingRequired = await loadSnapshot(
    "src/shared/fixtures/pairing-required.json",
  );
  const disconnected = await loadSnapshot(
    "src/shared/fixtures/disconnected.json",
  );

  await withPublisher(pairingRequired, async ({ publisher, socketPath }) => {
    publisher.publish(disconnected);
    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), disconnected);
    client.destroy();
  });
});

test("rejects a display string beyond its shared limit without replacing the current snapshot", async () => {
  const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");

  await withPublisher(snapshot, async ({ publisher, socketPath }) => {
    assert.throws(
      () => publisher.publish(withTitle(snapshot, "\ud800")),
      /Title contains invalid Unicode/,
    );
    assert.throws(
      () => publisher.publish(withTitle(snapshot, "🌌".repeat(1_025))),
      /Title exceeds 1,024 Unicode code points/,
    );

    const client = createConnection(socketPath);
    const lines = createInterface({ input: client });
    const [line] = (await once(lines, "line")) as [string];

    assert.deepEqual(JSON.parse(line), snapshot);
    client.destroy();
  });
});

test("accepts 65,536 serialized bytes including the newline and rejects one more", async () => {
  const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
  const bounded = withSerializedBytes(snapshot, MAX_SNAPSHOT_BYTES);
  const oversized = withSerializedBytes(snapshot, MAX_SNAPSHOT_BYTES + 1);

  await withTaskDirectory(async (runtimeDirectory) => {
    const publisher = await startSnapshotPublisher(
      bounded,
      path.join(runtimeDirectory, "roonscape.sock"),
    );
    await publisher.close();
  });
  await withTaskDirectory(async (runtimeDirectory) => {
    await assert.rejects(
      startSnapshotPublisher(
        oversized,
        path.join(runtimeDirectory, "roonscape.sock"),
      ),
      /Snapshot exceeds 64 KiB/,
    );
  });
});

test(
  "coalesces updates to the latest complete snapshot during backpressure",
  { timeout: 5_000 },
  async () => {
    const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
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
            withSerializedBytes({ ...snapshot, revision }, 60 * 1024),
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
  const pairingRequired = await loadSnapshot(
    "src/shared/fixtures/pairing-required.json",
  );
  const disconnected = await loadSnapshot(
    "src/shared/fixtures/disconnected.json",
  );
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
    const snapshot = await loadSnapshot(
      "src/shared/fixtures/pairing-required.json",
    );
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
    const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
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

      publisher = await startSnapshotPublisher(snapshot, socketPath);
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
  const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
  await withPublisher(snapshot, async ({ socketPath }) => {
    await assert.rejects(
      startSnapshotPublisher(snapshot, socketPath),
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
    const publisher = await startSnapshotPublisher(snapshot, socketPath);
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

function withSerializedBytes(
  snapshot: PresentationSnapshot,
  serializedBytes: number,
): PresentationSnapshot {
  const base = {
    ...snapshot,
    artwork: { revision: snapshot.revision, path: "" },
  };
  const baseBytes = Buffer.byteLength(`${JSON.stringify(base)}\n`, "utf8");
  assert.ok(serializedBytes > baseBytes);
  return {
    ...base,
    artwork: {
      ...base.artwork,
      path: "x".repeat(serializedBytes - baseBytes),
    },
  };
}
