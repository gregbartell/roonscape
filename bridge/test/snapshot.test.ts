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

test("loads every shared playback state with truthful Now Playing", async () => {
  const expected = [
    ["playing.json", "playing", true, true],
    ["paused.json", "paused", true, true],
    ["loading.json", "loading", true, true],
    ["loading-empty.json", "loading", false, false],
    ["stopped.json", "stopped", false, false],
  ] as const;

  for (const [fixture, playback, hasNowPlaying, hasProgress] of expected) {
    const snapshot = await loadSnapshot(`fixtures/${fixture}`);

    assert.equal(snapshot.availability, "available");
    assert.equal(snapshot.playback, playback);
    assert.equal(snapshot.nowPlaying !== null, hasNowPlaying);
    assert.equal(snapshot.progress !== null, hasProgress);
    if (playback === "stopped") {
      assert.equal(snapshot.artwork, null);
    }
  }
});

test("loads indeterminate progress as absent and permits clamping samples", async () => {
  const indeterminate = await loadSnapshot(
    "fixtures/indeterminate-progress.json",
  );
  const pastDuration = await loadSnapshot(
    "fixtures/playing-past-duration.json",
  );

  assert.equal(indeterminate.progress, null);
  assert.deepEqual(pastDuration.progress, {
    positionSeconds: 300,
    durationSeconds: 234,
    sampledAt: "2026-08-15T19:20:00Z",
  });
});

test("rejects the shared invalid fixture", async () => {
  await assert.rejects(
    loadSnapshot("fixtures/invalid.json"),
    /Invalid presentation snapshot/,
  );
});

test("rejects invalid timing and stopped snapshots with stale Now Playing", async () => {
  await Promise.all(
    ["invalid-progress.json", "invalid-stopped-now-playing.json"].map(
      async (fixture) =>
        assert.rejects(
          loadSnapshot(`fixtures/${fixture}`),
          /Invalid presentation snapshot/,
        ),
    ),
  );
});
