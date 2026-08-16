import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { loadSnapshot, validateSnapshot } from "../src/snapshot.js";

test("loads the shared Playing fixture as a complete snapshot", async () => {
  const snapshot = await loadSnapshot("fixtures/playing.json");

  assert.deepEqual(snapshot, {
    schemaVersion: 2,
    revision: 7,
    availability: "available",
    playback: "playing",
    trackedOutput: { name: "AudioDevice" },
    trackedZone: { name: "Living Room" },
    nowPlaying: {
      title: "Last Light on Phobos",
      artist: "Evelyn Lark & The Orbital Choir",
      album: "Signals from the Quiet Sea",
    },
    progress: {
      positionSeconds: 171,
      durationSeconds: 266,
      sampledAt: "2026-08-15T19:20:00Z",
    },
    artwork: {
      revision: 3,
      path: "fixtures/artwork/playing.svg",
    },
  });
});

test("ships the selected prototype artwork byte for byte", async () => {
  const [fixtureArtwork, prototypeArtwork] = await Promise.all([
    readFile(new URL("../../../fixtures/artwork/playing.svg", import.meta.url)),
    readFile(
      new URL(
        "../../../prototype/gallery-split-font-study/album-art.svg",
        import.meta.url,
      ),
    ),
  ]);

  assert.deepEqual(fixtureArtwork, prototypeArtwork);
});

test("keeps the selected fictional release coherent across related fixtures", async () => {
  const expected = {
    title: "Last Light on Phobos",
    artist: "Evelyn Lark & The Orbital Choir",
    album: "Signals from the Quiet Sea",
    trackedOutput: { name: "AudioDevice" },
    trackedZone: { name: "Living Room" },
  };
  const relatedFixtures = [
    "playing.json",
    "paused.json",
    "loading.json",
    "missing-artwork.json",
    "artwork-revision-changed.json",
  ];

  for (const fixtureName of relatedFixtures) {
    const snapshot = await loadSnapshot(`fixtures/${fixtureName}`);

    assert.deepEqual(snapshot.nowPlaying, {
      title: expected.title,
      artist: expected.artist,
      album: expected.album,
    });
    assert.deepEqual(snapshot.trackedOutput, expected.trackedOutput);
    assert.deepEqual(snapshot.trackedZone, expected.trackedZone);
  }

  const missingArtist = await loadSnapshot("fixtures/missing-artist.json");
  assert.deepEqual(missingArtist.nowPlaying, {
    title: expected.title,
    artist: null,
    album: expected.album,
  });

  const missingAlbum = await loadSnapshot("fixtures/missing-album.json");
  assert.deepEqual(missingAlbum.nowPlaying, {
    title: expected.title,
    artist: expected.artist,
    album: null,
  });

  const missingDetails = await loadSnapshot("fixtures/missing-metadata.json");
  assert.deepEqual(missingDetails.nowPlaying, {
    title: expected.title,
    artist: null,
    album: null,
  });
});

test("uses the reference progress sample across playback fixtures", async () => {
  for (const fixtureName of ["playing.json", "paused.json", "loading.json"]) {
    const snapshot = await loadSnapshot(`fixtures/${fixtureName}`);

    assert.deepEqual(
      snapshot.progress === null
        ? null
        : {
            positionSeconds: snapshot.progress.positionSeconds,
            durationSeconds: snapshot.progress.durationSeconds,
          },
      { positionSeconds: 171, durationSeconds: 266 },
    );
  }
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
    title: "Last Light on Phobos",
    artist: null,
    album: null,
  });
  assert.equal(missingArtist.nowPlaying?.artist, null);
  assert.equal(missingArtist.nowPlaying?.album, "Signals from the Quiet Sea");
  assert.equal(
    missingAlbum.nowPlaying?.artist,
    "Evelyn Lark & The Orbital Choir",
  );
  assert.equal(missingAlbum.nowPlaying?.album, null);
  assert.ok((long.nowPlaying?.title?.length ?? 0) > 80);
  assert.ok((long.nowPlaying?.artist?.length ?? 0) > 80);
  assert.ok((long.nowPlaying?.album?.length ?? 0) > 80);
  assert.ok((extreme.nowPlaying?.title?.length ?? 0) > 250);
});

test("loads non-square artwork and long identity edge-case fixtures", async () => {
  const nonSquareArtwork = await loadSnapshot(
    "fixtures/non-square-artwork.json",
  );
  const longIdentities = await loadSnapshot("fixtures/long-identities.json");

  assert.deepEqual(nonSquareArtwork.artwork, {
    revision: 19,
    path: "fixtures/artwork/non-square.svg",
  });
  assert.deepEqual(nonSquareArtwork.nowPlaying, {
    title: "Last Light on Phobos",
    artist: "Evelyn Lark & The Orbital Choir",
    album: "Signals from the Quiet Sea",
  });
  assert.ok((longIdentities.trackedOutput?.name.length ?? 0) > 80);
  assert.ok((longIdentities.trackedZone?.name.length ?? 0) > 80);
  assert.deepEqual(longIdentities.nowPlaying, nonSquareArtwork.nowPlaying);
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
    durationSeconds: 266,
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
