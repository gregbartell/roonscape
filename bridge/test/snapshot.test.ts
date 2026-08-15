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

test("loads every shared unavailable fixture without stale Now Playing", async () => {
  const fixtures = [
    ["pairing-required.json", "pairingRequired"],
    ["disconnected.json", "disconnected"],
    ["output-unavailable.json", "outputUnavailable"],
  ] as const;

  for (const [fixture, availability] of fixtures) {
    const snapshot = await loadSnapshot(`fixtures/${fixture}`);

    assert.deepEqual(
      {
        availability: snapshot.availability,
        playback: snapshot.playback,
        displayZone: snapshot.displayZone,
        nowPlaying: snapshot.nowPlaying,
        progress: snapshot.progress,
        artwork: snapshot.artwork,
      },
      {
        availability,
        playback: null,
        displayZone: null,
        nowPlaying: null,
        progress: null,
        artwork: null,
      },
    );
  }
});

test("rejects the shared invalid fixture", async () => {
  await assert.rejects(
    loadSnapshot("fixtures/invalid.json"),
    /Invalid presentation snapshot/,
  );
});
