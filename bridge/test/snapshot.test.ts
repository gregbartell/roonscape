import assert from "node:assert/strict";
import test from "node:test";

import { loadSnapshot, validateSnapshot } from "../src/snapshot.js";

test("loads the shared Playing fixture as a complete snapshot", async () => {
  const snapshot = await loadSnapshot("fixtures/playing.json");

  assert.deepEqual(snapshot, {
    schemaVersion: 2,
    revision: 7,
    availability: "available",
    playback: "playing",
    trackedOutput: { name: "NUC HDMI" },
    trackedZone: { name: "Gallery" },
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

test("loads missing artwork and artwork revision fixtures", async () => {
  const missingArtwork = await loadSnapshot("fixtures/missing-artwork.json");
  const revisedArtwork = await loadSnapshot(
    "fixtures/artwork-revision-changed.json",
  );

  assert.equal(missingArtwork.artwork, null);
  assert.deepEqual(revisedArtwork.artwork, {
    revision: 9,
    path: "fixtures/artwork/revised.svg",
  });
  assert.equal(revisedArtwork.revision, 9);
});

test("loads missing, long, and extreme metadata fixtures without inventing values", async () => {
  const missing = await loadSnapshot("fixtures/missing-metadata.json");
  const missingArtist = await loadSnapshot("fixtures/missing-artist.json");
  const missingAlbum = await loadSnapshot("fixtures/missing-album.json");
  const long = await loadSnapshot("fixtures/long-metadata.json");
  const extreme = await loadSnapshot("fixtures/extreme-metadata.json");

  assert.deepEqual(missing.nowPlaying, {
    title: "An Ending (Ascent)",
    artist: null,
    album: null,
  });
  assert.equal(missingArtist.nowPlaying?.artist, null);
  assert.equal(
    missingArtist.nowPlaying?.album,
    "Apollo: Atmospheres and Soundtracks",
  );
  assert.equal(missingAlbum.nowPlaying?.artist, "Brian Eno");
  assert.equal(missingAlbum.nowPlaying?.album, null);
  assert.ok((long.nowPlaying?.title?.length ?? 0) > 80);
  assert.ok((long.nowPlaying?.artist?.length ?? 0) > 80);
  assert.ok((long.nowPlaying?.album?.length ?? 0) > 80);
  assert.ok((extreme.nowPlaying?.title?.length ?? 0) > 250);
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
        trackedOutput: snapshot.trackedOutput,
        trackedZone: snapshot.trackedZone,
        nowPlaying: snapshot.nowPlaying,
        progress: snapshot.progress,
        artwork: snapshot.artwork,
      },
      {
        availability,
        playback: null,
        trackedOutput: null,
        trackedZone: null,
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

test("rejects the removed displayZone snapshot field", async () => {
  await assert.rejects(
    validateSnapshot({
      schemaVersion: 2,
      revision: 1,
      availability: "available",
      playback: "playing",
      trackedOutput: { name: "NUC HDMI" },
      trackedZone: { name: "Gallery" },
      displayZone: { name: "Gallery" },
      nowPlaying: null,
      progress: null,
      artwork: null,
    }),
    /Invalid presentation snapshot/,
  );
});

test("requires both Tracked Output and Tracked Zone for available snapshots", async () => {
  const snapshot = await loadSnapshot("fixtures/playing.json");
  const missingTrackedOutput: Partial<typeof snapshot> = { ...snapshot };
  const missingTrackedZone: Partial<typeof snapshot> = { ...snapshot };
  Reflect.deleteProperty(missingTrackedOutput, "trackedOutput");
  Reflect.deleteProperty(missingTrackedZone, "trackedZone");

  await assert.rejects(
    validateSnapshot(missingTrackedOutput),
    /Invalid presentation snapshot/,
  );
  await assert.rejects(
    validateSnapshot(missingTrackedZone),
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
