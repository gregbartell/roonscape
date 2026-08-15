import assert from "node:assert/strict";
import test from "node:test";

import { loadSnapshot } from "../src/snapshot.js";

test("loads the shared Playing fixture as a complete snapshot", async () => {
  const snapshot = await loadSnapshot("fixtures/playing.json");

  assert.deepEqual(snapshot, {
    schemaVersion: 1,
    revision: 7,
    availability: "available",
    playback: "playing",
    displayZone: { name: "Gallery" },
    nowPlaying: {
      title: "A Moment Apart",
      artist: "ODESZA",
      album: "A Moment Apart",
    },
    progress: {
      positionSeconds: 82,
      durationSeconds: 234,
      sampledAt: "2026-08-15T19:20:00Z",
    },
    artwork: {
      revision: 3,
      path: "fixtures/artwork/playing.svg",
    },
  });
});

test("rejects the shared invalid fixture", async () => {
  await assert.rejects(
    loadSnapshot("fixtures/invalid.json"),
    /Invalid presentation snapshot/,
  );
});
