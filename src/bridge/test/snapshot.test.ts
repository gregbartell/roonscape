import assert from "node:assert/strict";
import test from "node:test";

import { loadSnapshot, validateSnapshot } from "../src/snapshot.js";

test("loads the shared Playing fixture as a complete snapshot", async () => {
  const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");

  assert.deepEqual(snapshot, {
    schemaVersion: 2,
    revision: 7,
    availability: "available",
    playback: "playing",
    trackedOutput: { name: "Speaker System" },
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
      path: "src/shared/fixtures/artwork/playing.svg",
    },
  });
});

test("keeps the selected fictional release coherent across related fixtures", async () => {
  const expected = {
    title: "Last Light on Phobos",
    artist: "Evelyn Lark & The Orbital Choir",
    album: "Signals from the Quiet Sea",
    trackedOutput: { name: "Speaker System" },
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
    const snapshot = await loadSnapshot(`src/shared/fixtures/${fixtureName}`);

    assert.deepEqual(snapshot.nowPlaying, {
      title: expected.title,
      artist: expected.artist,
      album: expected.album,
    });
    assert.deepEqual(snapshot.trackedOutput, expected.trackedOutput);
    assert.deepEqual(snapshot.trackedZone, expected.trackedZone);
  }

  const missingArtist = await loadSnapshot(
    "src/shared/fixtures/missing-artist.json",
  );
  assert.deepEqual(missingArtist.nowPlaying, {
    title: expected.title,
    artist: null,
    album: expected.album,
  });

  const missingAlbum = await loadSnapshot(
    "src/shared/fixtures/missing-album.json",
  );
  assert.deepEqual(missingAlbum.nowPlaying, {
    title: expected.title,
    artist: expected.artist,
    album: null,
  });

  const missingDetails = await loadSnapshot(
    "src/shared/fixtures/missing-metadata.json",
  );
  assert.deepEqual(missingDetails.nowPlaying, {
    title: expected.title,
    artist: null,
    album: null,
  });
});

test("uses the reference progress sample unless timing is the named edge", async () => {
  for (const fixtureName of [
    "playing.json",
    "paused.json",
    "loading.json",
    "missing-metadata.json",
    "missing-artist.json",
    "missing-album.json",
    "missing-artwork.json",
    "artwork-revision-changed.json",
    "long-metadata.json",
    "extreme-metadata.json",
  ]) {
    const snapshot = await loadSnapshot(`src/shared/fixtures/${fixtureName}`);

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
  const missingArtwork = await loadSnapshot(
    "src/shared/fixtures/missing-artwork.json",
  );
  const revisedArtwork = await loadSnapshot(
    "src/shared/fixtures/artwork-revision-changed.json",
  );

  assert.equal(missingArtwork.artwork, null);
  assert.deepEqual(revisedArtwork.artwork, {
    revision: 9,
    path: "src/shared/fixtures/artwork/revised.svg",
  });
  assert.equal(revisedArtwork.revision, 9);
});

test("loads the light-artwork visual acceptance fixture", async () => {
  const snapshot = await loadSnapshot("src/shared/fixtures/light-artwork.json");

  assert.deepEqual(snapshot.artwork, {
    revision: 20,
    path: "src/shared/fixtures/artwork/light.svg",
  });
  assert.deepEqual(snapshot.nowPlaying, {
    title: "Last Light on Phobos",
    artist: "Evelyn Lark & The Orbital Choir",
    album: "Signals from the Quiet Sea",
  });
});

test("loads the typography glyph-fallback visual acceptance fixture", async () => {
  const snapshot = await loadSnapshot(
    "src/shared/fixtures/glyph-fallback.json",
  );

  assert.equal(snapshot.nowPlaying?.album, "Signals from the Quiet Sea — 月");
  assert.deepEqual(snapshot.artwork, {
    revision: 3,
    path: "src/shared/fixtures/artwork/playing.svg",
  });
});

test("loads missing, long, and extreme metadata fixtures without inventing values", async () => {
  const missing = await loadSnapshot(
    "src/shared/fixtures/missing-metadata.json",
  );
  const missingArtist = await loadSnapshot(
    "src/shared/fixtures/missing-artist.json",
  );
  const missingAlbum = await loadSnapshot(
    "src/shared/fixtures/missing-album.json",
  );
  const long = await loadSnapshot("src/shared/fixtures/long-metadata.json");
  const extreme = await loadSnapshot(
    "src/shared/fixtures/extreme-metadata.json",
  );

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

test("metadata-only edge fixtures retain the reference artwork", async () => {
  const referenceArtwork = {
    revision: 3,
    path: "src/shared/fixtures/artwork/playing.svg",
  };

  for (const fixtureName of ["long-metadata.json", "extreme-metadata.json"]) {
    const snapshot = await loadSnapshot(`src/shared/fixtures/${fixtureName}`);

    assert.deepEqual(snapshot.artwork, referenceArtwork);
  }
});

test("loads non-square artwork and long identity edge-case fixtures", async () => {
  const nonSquareArtwork = await loadSnapshot(
    "src/shared/fixtures/non-square-artwork.json",
  );
  const longIdentities = await loadSnapshot(
    "src/shared/fixtures/long-identities.json",
  );
  const blankOptionalMetadata = await loadSnapshot(
    "src/shared/fixtures/blank-optional-metadata.json",
  );

  assert.deepEqual(nonSquareArtwork.artwork, {
    revision: 19,
    path: "src/shared/fixtures/artwork/non-square.svg",
  });
  assert.deepEqual(nonSquareArtwork.nowPlaying, {
    title: "Last Light on Phobos",
    artist: "Evelyn Lark & The Orbital Choir",
    album: "Signals from the Quiet Sea",
  });
  assert.ok((longIdentities.trackedOutput?.name.length ?? 0) > 80);
  assert.ok((longIdentities.trackedZone?.name.length ?? 0) > 80);
  assert.deepEqual(longIdentities.nowPlaying, nonSquareArtwork.nowPlaying);
  for (const snapshot of [
    nonSquareArtwork,
    longIdentities,
    blankOptionalMetadata,
  ]) {
    assert.deepEqual(snapshot.progress, {
      positionSeconds: 171,
      durationSeconds: 266,
      sampledAt: "2026-08-15T19:20:00Z",
    });
  }
  assert.deepEqual(blankOptionalMetadata.nowPlaying, {
    title: "Last Light on Phobos",
    artist: "   ",
    album: "\t",
  });
});

test("loads every shared unavailable fixture without stale Now Playing", async () => {
  const fixtures = [
    ["pairing-required.json", "pairingRequired"],
    ["disconnected.json", "disconnected"],
    ["output-unavailable.json", "outputUnavailable"],
  ] as const;

  for (const [fixture, availability] of fixtures) {
    const snapshot = await loadSnapshot(`src/shared/fixtures/${fixture}`);

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
    ["paused-empty.json", "paused", false, false],
    ["loading.json", "loading", true, true],
    ["loading-empty.json", "loading", false, false],
    ["stopped.json", "stopped", false, false],
  ] as const;

  for (const [fixture, playback, hasNowPlaying, hasProgress] of expected) {
    const snapshot = await loadSnapshot(`src/shared/fixtures/${fixture}`);

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
    "src/shared/fixtures/indeterminate-progress.json",
  );
  const pastDuration = await loadSnapshot(
    "src/shared/fixtures/playing-past-duration.json",
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
    loadSnapshot("src/shared/fixtures/invalid.json"),
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
      trackedOutput: { name: "Speaker System" },
      trackedZone: { name: "Living Room" },
      displayZone: { name: "Living Room" },
      nowPlaying: null,
      progress: null,
      artwork: null,
    }),
    /Invalid presentation snapshot/,
  );
});

test("requires both Tracked Output and Tracked Zone for available snapshots", async () => {
  const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
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
          loadSnapshot(`src/shared/fixtures/${fixture}`),
          /Invalid presentation snapshot/,
        ),
    ),
  );
});

test("bounds Roon-supplied display strings by Unicode code points", async () => {
  const snapshot = await loadSnapshot("src/shared/fixtures/playing.json");
  const cases = [
    [
      "Tracked Output",
      256,
      (value: string) => ({ trackedOutput: { name: value } }),
    ],
    [
      "Tracked Zone",
      256,
      (value: string) => ({ trackedZone: { name: value } }),
    ],
    [
      "Title",
      1_024,
      (value: string) => ({
        nowPlaying: { ...snapshot.nowPlaying, title: value },
      }),
    ],
    [
      "Artist",
      1_024,
      (value: string) => ({
        nowPlaying: { ...snapshot.nowPlaying, artist: value },
      }),
    ],
    [
      "Album",
      1_024,
      (value: string) => ({
        nowPlaying: { ...snapshot.nowPlaying, album: value },
      }),
    ],
  ] as const;

  for (const [field, limit, changed] of cases) {
    const multibyteCodePoint = "🌌";
    await assert.doesNotReject(
      validateSnapshot({
        ...snapshot,
        ...changed(multibyteCodePoint.repeat(limit)),
      }),
      `${field} should accept ${limit} Unicode code points`,
    );
    await assert.rejects(
      validateSnapshot({
        ...snapshot,
        ...changed(multibyteCodePoint.repeat(limit + 1)),
      }),
      /must NOT have more than/,
      `${field} should reject ${limit + 1} Unicode code points`,
    );
  }

  await assert.rejects(
    validateSnapshot({
      ...snapshot,
      nowPlaying: { ...snapshot.nowPlaying, title: "\ud800" },
    }),
    /Title contains invalid Unicode/,
  );
});
