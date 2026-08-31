import assert from "node:assert/strict";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  ArtworkFileStore,
  type ArtworkFiles,
} from "../src/artwork-file-store.js";
import {
  startRoonBridge,
  type AuthorizationStore,
  type RoonCore,
  type RoonExtension,
  type RoonExtensionOptions,
  type RoonZone,
  type RoonZoneEvent,
  type RoonZoneSubscriptionResponse,
  type RoonStatusService,
} from "../src/roon-bridge.js";
import type { DisplayConfigurationStore } from "../src/display-configuration.js";
import {
  type PresentationSnapshot,
  validateSnapshot,
} from "../src/snapshot.js";

interface RoonBoundary {
  authorizationStore: AuthorizationStore;
  core(): RoonCore;
  emitZones(response: RoonZoneSubscriptionResponse, event: RoonZoneEvent): void;
  extension: RoonExtension;
  extensionOptions(): RoonExtensionOptions;
  imageRequests: Array<{
    imageKey: string;
    options: {
      scale: "fit";
      width: number;
      height: number;
      format: "image/jpeg";
    };
  }>;
  failImage(error?: string): void;
  retryDelays: number[];
  runNextArtworkRetry(): void;
  resolveImage(contentType: string, image: Buffer): void;
  resolveImageRequest(index: number, contentType: string, image: Buffer): void;
  snapshots: PresentationSnapshot[];
  statusUpdates: Array<{ message: string; isError: boolean }>;
}

function createRoonBoundary(
  trackedOutputId?: string,
  artworkFiles: ArtworkFiles = unusedArtworkFiles(),
  now: () => Date = () => new Date("2026-08-15T19:20:00Z"),
): RoonBoundary {
  let persistedState: unknown = {};
  let capturedOptions: RoonExtensionOptions | undefined;
  let zoneListener:
    | ((response: RoonZoneSubscriptionResponse, event: RoonZoneEvent) => void)
    | undefined;
  const snapshots: PresentationSnapshot[] = [];
  const statusUpdates: Array<{ message: string; isError: boolean }> = [];
  const imageRequests: RoonBoundary["imageRequests"] = [];
  const imageCallbacks: Array<
    (error: string | false, contentType?: string, image?: Buffer) => void
  > = [];
  const artworkRetries: Array<{ cancelled: boolean; retry(): void }> = [];
  const retryDelays: number[] = [];
  const authorizationStore: AuthorizationStore = {
    load: () => persistedState,
    save: (state) => {
      persistedState = state;
    },
  };
  const displayConfigurationStore: DisplayConfigurationStore = {
    load: () => (trackedOutputId === undefined ? null : { trackedOutputId }),
    save: () => undefined,
  };
  const status: RoonStatusService = {
    services: [{ name: "com.roonlabs.status:1" }],
    set_status: (message, isError) => {
      statusUpdates.push({ message, isError });
    },
  };
  const extension: RoonExtension = {
    init_services: () => undefined,
    start_discovery: () => undefined,
    stop_discovery: () => undefined,
    disconnect_all: () => undefined,
  };
  const core: RoonCore = {
    core_id: "core-1",
    services: {
      RoonApiImage: {
        get_image: (imageKey, options, callback) => {
          imageRequests.push({ imageKey, options });
          imageCallbacks.push(callback);
        },
      },
      RoonApiTransport: {
        subscribe_zones: (listener) => {
          zoneListener = listener;
        },
      },
    },
  };

  startRoonBridge({
    authorizationStore,
    artworkFiles,
    displayConfigurationStore,
    createRoonServices: (options) => {
      capturedOptions = options;
      return {
        extension,
        requiredServices: [{ services: [{ name: "com.roonlabs.image:1" }] }],
        status,
      };
    },
    publish: (snapshot) => snapshots.push(snapshot),
    scheduleArtworkRetry: (retry, delayMilliseconds) => {
      const scheduledRetry = { cancelled: false, retry };
      artworkRetries.push(scheduledRetry);
      retryDelays.push(delayMilliseconds);
      return () => {
        scheduledRetry.cancelled = true;
      };
    },
    now,
  });

  return {
    authorizationStore,
    core: () => core,
    emitZones: (response, event) => {
      assert.ok(zoneListener);
      zoneListener(response, event);
    },
    extension,
    extensionOptions: () => {
      assert.ok(capturedOptions);
      return capturedOptions;
    },
    failImage: (error = "transient image failure") => {
      const callback = imageCallbacks.at(-1);
      assert.ok(callback);
      callback(error);
    },
    imageRequests,
    retryDelays,
    runNextArtworkRetry: () => {
      const retry = artworkRetries.shift();
      assert.ok(retry);
      if (!retry.cancelled) {
        retry.retry();
      }
    },
    resolveImage: (contentType, image) => {
      const callback = imageCallbacks.at(-1);
      assert.ok(callback);
      callback(false, contentType, image);
    },
    resolveImageRequest: (index, contentType, image) => {
      const callback = imageCallbacks[index];
      assert.ok(callback);
      callback(false, contentType, image);
    },
    snapshots,
    statusUpdates,
  };
}

function artworkZone(imageKey: string, title = "A Moment Apart"): RoonZone {
  return {
    zone_id: "zone-living-room",
    display_name: "Living Room",
    state: "playing",
    outputs: [
      { output_id: "output-speaker-system", display_name: "Speaker System" },
    ],
    now_playing: {
      image_key: imageKey,
      three_line: { line1: title, line2: "ODESZA" },
    },
  };
}

async function withArtworkTestBoundary(
  run: (boundary: RoonBoundary) => Promise<void>,
  wrapArtworkFiles: (store: ArtworkFiles) => ArtworkFiles = (store) => store,
): Promise<void> {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bridge-test."),
  );
  const store = await ArtworkFileStore.open(
    path.join(taskDirectory, "artwork"),
  );
  const boundary = createRoonBoundary(
    "output-speaker-system",
    wrapArtworkFiles(store),
  );

  try {
    await run(boundary);
  } finally {
    await store.clear();
    await rm(taskDirectory, { recursive: true });
  }
}

test("retries unchanged Now Playing artwork after a transient download failure", async () => {
  await withArtworkTestBoundary(async (boundary) => {
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", {
      zones: [artworkZone("unchanged-artwork-key")],
    });
    boundary.failImage();

    assert.deepEqual(boundary.retryDelays, [1_000]);
    assert.equal(boundary.imageRequests.length, 1);

    boundary.runNextArtworkRetry();
    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["unchanged-artwork-key", "unchanged-artwork-key"],
    );
    boundary.resolveImage("image/jpeg", Buffer.from("recovered artwork"));
    await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);

    const artworkPath = boundary.snapshots.at(-1)?.artwork?.path;
    assert.ok(artworkPath);
    assert.equal(await readFile(artworkPath, "utf8"), "recovered artwork");
  });
});

test("retries unchanged artwork after validation, staging, and commit failures", async (t) => {
  for (const failure of ["validation", "stage", "commit"] as const) {
    await t.test(failure, async () => {
      let shouldFail = true;
      await withArtworkTestBoundary(
        async (boundary) => {
          boundary.extensionOptions().core_paired(boundary.core());
          boundary.emitZones("Subscribed", {
            zones: [artworkZone(`artwork-with-${failure}-failure`)],
          });
          if (failure === "validation") {
            boundary.resolveImage("image/png", Buffer.from("wrong format"));
          } else {
            boundary.resolveImage("image/jpeg", Buffer.from("first attempt"));
          }
          await waitFor(() => boundary.retryDelays.length === 1);

          boundary.runNextArtworkRetry();
          boundary.resolveImage("image/jpeg", Buffer.from("recovered artwork"));
          await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);

          assert.equal(boundary.imageRequests.length, 2);
          const artworkPath = boundary.snapshots.at(-1)?.artwork?.path;
          assert.ok(artworkPath);
          assert.equal(
            await readFile(artworkPath, "utf8"),
            "recovered artwork",
          );
        },
        (store) => ({
          stage: (revision, image) => {
            if (failure === "stage" && shouldFail) {
              shouldFail = false;
              return Promise.reject(new Error("transient stage failure"));
            }
            return store.stage(revision, image);
          },
          commit: (reference) => {
            if (failure === "commit" && shouldFail) {
              shouldFail = false;
              return Promise.reject(new Error("transient commit failure"));
            }
            return store.commit(reference);
          },
          discard: (reference) => store.discard(reference),
          clear: () => store.clear(),
        }),
      );
    });
  }
});

test("caps artwork retry backoff while Now Playing remains unchanged", () => {
  const boundary = createRoonBoundary("output-speaker-system");

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [artworkZone("persistent-failure")],
  });
  for (let failure = 0; failure < 7; failure += 1) {
    boundary.failImage();
    boundary.runNextArtworkRetry();
  }

  assert.deepEqual(
    boundary.retryDelays,
    [1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000],
  );
});

test("cancels an artwork retry when Now Playing changes", async () => {
  const firstZone = artworkZone("first-artwork-key");
  const secondZone: RoonZone = {
    ...firstZone,
    now_playing: {
      image_key: "second-artwork-key",
      three_line: { line1: "Across the Room", line2: "ODESZA" },
    },
  };

  await withArtworkTestBoundary(async (boundary) => {
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", { zones: [firstZone] });
    boundary.failImage();
    boundary.emitZones("Changed", { zones_changed: [secondZone] });
    boundary.runNextArtworkRetry();

    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["first-artwork-key", "second-artwork-key"],
    );

    boundary.resolveImage("image/jpeg", Buffer.from("second artwork"));
    await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);
    const artworkPath = boundary.snapshots.at(-1)?.artwork?.path;
    assert.ok(artworkPath);
    assert.equal(await readFile(artworkPath, "utf8"), "second artwork");
  });
});

function unusedArtworkFiles(): ArtworkFiles {
  return {
    stage: () => Promise.reject(new Error("Artwork was not expected")),
    commit: () => Promise.resolve(),
    discard: () => Promise.resolve(),
    clear: () => Promise.resolve(),
  };
}

interface ArtworkTestContext {
  artwork: NonNullable<PresentationSnapshot["artwork"]>;
  artworkDirectory: string;
  boundary: RoonBoundary;
  cleanup(): Promise<void>;
  zone: RoonZone;
}

async function prepareArtworkTestContext({
  image = "stable artwork",
  imageKey = "same-track-artwork",
  now,
  output = {
    output_id: "output-speaker-system",
    display_name: "Speaker System",
  },
}: {
  image?: string;
  imageKey?: string;
  now?: () => Date;
  output?: RoonZone["outputs"][number];
} = {}): Promise<ArtworkTestContext> {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bridge-test."),
  );
  const artworkDirectory = path.join(taskDirectory, "artwork");
  const artworkFiles = await ArtworkFileStore.open(artworkDirectory);
  const boundary = createRoonBoundary(
    "output-speaker-system",
    artworkFiles,
    now,
  );
  const zone: RoonZone = {
    zone_id: "zone-living-room",
    display_name: "Living Room",
    state: "playing",
    outputs: [output],
    now_playing: {
      image_key: imageKey,
      seek_position: 82,
      length: 234,
      three_line: {
        line1: "A Moment Apart",
        line2: "ODESZA",
        line3: "A Moment Apart",
      },
    },
  };

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", { zones: [zone] });
  boundary.resolveImage("image/jpeg", Buffer.from(image));
  await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);
  const artwork = boundary.snapshots.at(-1)?.artwork;
  assert.ok(artwork);

  return {
    artwork,
    artworkDirectory,
    boundary,
    cleanup: async () => {
      await artworkFiles.clear();
      await rm(taskDirectory, { recursive: true });
    },
    zone,
  };
}

test("publishes truthful availability across authorization and connection events", async () => {
  const boundary = createRoonBoundary();
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.set_persisted_state({
    paired_core_id: "core-1",
    tokens: { "core-1": "authorization-token" },
  });
  extensionOptions.core_paired(boundary.core());
  extensionOptions.core_unpaired(boundary.core());
  extensionOptions.core_paired(boundary.core());

  assert.deepEqual(
    boundary.snapshots,
    [
      "pairingRequired",
      "outputUnavailable",
      "disconnected",
      "outputUnavailable",
    ].map((availability, revision) => ({
      schemaVersion: 2,
      revision,
      availability,
      playback: null,
      trackedOutput: null,
      trackedZone: null,
      nowPlaying: null,
      progress: null,
      artwork: null,
    })),
  );

  await Promise.all(boundary.snapshots.map(validateSnapshot));
});

test("resolves the configured Tracked Output from the initial full zone state", async () => {
  const boundary = createRoonBoundary("output-speaker-system");
  const extensionOptions = boundary.extensionOptions();
  const core = boundary.core();

  extensionOptions.core_paired(core);
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-office",
        display_name: "Office",
        state: "playing",
        outputs: [
          { output_id: "output-office", display_name: "Office Speaker" },
        ],
      },
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "paused",
        now_playing: {
          three_line: {
            line1: "A Moment Apart",
            line2: "ODESZA",
            line3: "A Moment Apart",
          },
        },
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
      },
    ],
  });

  assert.deepEqual(boundary.snapshots.at(-1), {
    schemaVersion: 2,
    revision: 2,
    availability: "available",
    playback: "paused",
    trackedOutput: { name: "Speaker System" },
    trackedZone: { name: "Living Room" },
    nowPlaying: {
      title: "A Moment Apart",
      artist: "ODESZA",
      album: "A Moment Apart",
    },
    progress: null,
    artwork: null,
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

test("publishes prepared display lines with compressed artwork from Roon Image", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bridge-test."),
  );
  const artworkDirectory = path.join(taskDirectory, "artwork");
  const artworkFiles = await ArtworkFileStore.open(artworkDirectory);
  const boundary = createRoonBoundary("output-speaker-system", artworkFiles);

  try {
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", {
      zones: [
        {
          zone_id: "zone-living-room",
          display_name: "Living Room",
          state: "playing",
          now_playing: {
            image_key: "opaque-roon-image-key",
            seek_position: 82,
            length: 234,
            three_line: {
              line1: "A Moment Apart",
              line2: "ODESZA",
              line3: "A Moment Apart",
            },
          },
          outputs: [
            {
              output_id: "output-speaker-system",
              display_name: "Speaker System",
            },
          ],
        },
      ],
    });

    assert.deepEqual(boundary.imageRequests, [
      {
        imageKey: "opaque-roon-image-key",
        options: {
          scale: "fit",
          width: 1600,
          height: 1600,
          format: "image/jpeg",
        },
      },
    ]);
    assert.equal(boundary.snapshots.at(-1)?.artwork, null);

    boundary.emitZones("Changed", {
      zones_seek_changed: [{ zone_id: "zone-living-room", seek_position: 30 }],
    });
    assert.equal(boundary.imageRequests.length, 1);

    boundary.resolveImage("image/jpeg", Buffer.from("compressed artwork"));
    await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);

    const snapshot = boundary.snapshots.at(-1);
    assert.deepEqual(
      snapshot === undefined
        ? undefined
        : {
            ...snapshot,
            artwork:
              snapshot.artwork === null
                ? null
                : { revision: snapshot.artwork.revision },
          },
      {
        schemaVersion: 2,
        revision: 4,
        availability: "available",
        playback: "playing",
        trackedOutput: { name: "Speaker System" },
        trackedZone: { name: "Living Room" },
        nowPlaying: {
          title: "A Moment Apart",
          artist: "ODESZA",
          album: "A Moment Apart",
        },
        progress: {
          positionSeconds: 30,
          durationSeconds: 234,
          sampledAt: "2026-08-15T19:20:00.000Z",
        },
        artwork: { revision: 4 },
      },
    );
    assert.equal(path.dirname(snapshot?.artwork?.path ?? ""), artworkDirectory);
    assert.match(
      path.basename(snapshot?.artwork?.path ?? ""),
      /^artwork-4-.+\.jpg$/,
    );
    await validateSnapshot(snapshot);
    assert.equal(
      await readFile(snapshot?.artwork?.path ?? "", "utf8"),
      "compressed artwork",
    );

    boundary.emitZones("Changed", {
      zones_changed: [
        {
          zone_id: "zone-living-room",
          display_name: "Living Room",
          state: "loading",
          now_playing: {
            image_key: "loading-artwork-key",
            seek_position: 30,
            length: 234,
            three_line: {
              line1: "A Moment Apart",
              line2: "ODESZA",
              line3: "A Moment Apart",
            },
          },
          outputs: [
            {
              output_id: "output-speaker-system",
              display_name: "Speaker System",
            },
          ],
        },
      ],
    });

    assert.deepEqual(boundary.snapshots.at(-1), {
      ...snapshot,
      revision: 5,
      playback: "loading",
    });
    boundary.resolveImage("image/jpeg", Buffer.from("loading artwork"));
    await waitFor(() => boundary.snapshots.at(-1)?.artwork?.revision === 6);

    boundary.emitZones("Changed", {
      zones_changed: [
        {
          zone_id: "zone-living-room",
          display_name: "Living Room",
          state: "playing",
          now_playing: {
            image_key: "revised-artwork-key",
            seek_position: 31,
            length: 234,
            three_line: {
              line1: "Across the Room",
              line2: "ODESZA",
              line3: "A Moment Apart",
            },
          },
          outputs: [
            {
              output_id: "output-speaker-system",
              display_name: "Speaker System",
            },
          ],
        },
      ],
    });
    boundary.resolveImage("image/jpeg", Buffer.from("revised artwork"));
    await waitFor(async () => {
      const files = await readdir(artworkDirectory);
      return files.length === 1 && /^artwork-8-.+\.jpg$/.test(files[0] ?? "");
    });

    assert.equal(boundary.snapshots.at(-1)?.artwork?.revision, 8);
    assert.match(
      path.basename(boundary.snapshots.at(-1)?.artwork?.path ?? ""),
      /^artwork-8-.+\.jpg$/,
    );
    assert.equal(
      await readFile(boundary.snapshots.at(-1)?.artwork?.path ?? "", "utf8"),
      "revised artwork",
    );

    boundary.emitZones("Changed", {
      zones_changed: [
        {
          zone_id: "zone-living-room",
          display_name: "Living Room",
          state: "stopped",
          now_playing: {
            image_key: "opaque-roon-image-key",
            seek_position: 31,
            length: 234,
            three_line: { line1: "Across the Room" },
          },
          outputs: [
            {
              output_id: "output-speaker-system",
              display_name: "Speaker System",
            },
          ],
        },
      ],
    });

    assert.deepEqual(boundary.snapshots.at(-1), {
      schemaVersion: 2,
      revision: 9,
      availability: "available",
      playback: "stopped",
      trackedOutput: { name: "Speaker System" },
      trackedZone: { name: "Living Room" },
      nowPlaying: null,
      progress: null,
      artwork: null,
    });
    assert.equal(boundary.imageRequests.length, 3);
    await waitFor(async () => (await readdir(artworkDirectory)).length === 0);
    assert.deepEqual(
      boundary.snapshots.map(({ revision }) => revision),
      boundary.snapshots.map((_, revision) => revision),
    );
    await Promise.all(boundary.snapshots.map(validateSnapshot));
  } finally {
    await artworkFiles.clear();
    await rm(taskDirectory, { recursive: true });
  }
});

test("retains artwork through a full same-track timing update", async () => {
  const sampleTimes = [
    new Date("2026-08-15T19:20:00Z"),
    new Date("2026-08-15T19:20:05Z"),
  ];
  const context = await prepareArtworkTestContext({
    now: () => sampleTimes.shift() ?? new Date("2026-08-15T19:20:05Z"),
  });
  const { artwork, boundary, zone } = context;

  try {
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...zone,
          now_playing: { ...zone.now_playing, seek_position: 90 },
        },
      ],
    });

    const timingSnapshots = boundary.snapshots.slice(snapshotCount);
    assert.equal(boundary.imageRequests.length, 1);
    assert.deepEqual(
      timingSnapshots.map((snapshot) => ({
        artwork: snapshot.artwork,
        progress: snapshot.progress,
      })),
      [
        {
          artwork,
          progress: {
            positionSeconds: 90,
            durationSeconds: 234,
            sampledAt: "2026-08-15T19:20:05.000Z",
          },
        },
      ],
    );
  } finally {
    await context.cleanup();
  }
});

test("retains artwork while pause and resume update playback truthfully", async () => {
  const sampleTimes = [
    new Date("2026-08-15T19:20:00Z"),
    new Date("2026-08-15T19:20:05Z"),
    new Date("2026-08-15T19:20:10Z"),
  ];
  const context = await prepareArtworkTestContext({
    now: () => sampleTimes.shift() ?? new Date("2026-08-15T19:20:10Z"),
  });
  const { artwork, boundary, zone } = context;

  try {
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...zone,
          state: "paused",
          now_playing: { ...zone.now_playing, seek_position: 90 },
        },
      ],
    });
    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...zone,
          now_playing: { ...zone.now_playing, seek_position: 91 },
        },
      ],
    });

    const playbackSnapshots = boundary.snapshots.slice(snapshotCount);
    assert.equal(boundary.imageRequests.length, 1);
    assert.deepEqual(
      playbackSnapshots.map((snapshot) => ({
        playback: snapshot.playback,
        progress: snapshot.progress,
      })),
      [
        {
          playback: "paused",
          progress: {
            positionSeconds: 90,
            durationSeconds: 234,
            sampledAt: "2026-08-15T19:20:05.000Z",
          },
        },
        {
          playback: "playing",
          progress: {
            positionSeconds: 91,
            durationSeconds: 234,
            sampledAt: "2026-08-15T19:20:10.000Z",
          },
        },
      ],
    );
    for (const snapshot of playbackSnapshots) {
      assert.deepEqual(snapshot.artwork, artwork);
    }
  } finally {
    await context.cleanup();
  }
});

test("ignores a volume-only Tracked Zone update", async () => {
  const sampleTimes = [
    new Date("2026-08-15T19:20:00Z"),
    new Date("2026-08-15T19:20:05Z"),
  ];
  const output = {
    output_id: "output-speaker-system",
    display_name: "Speaker System",
    volume: { type: "number", min: 0, max: 100, value: 30, step: 1 },
  };
  const context = await prepareArtworkTestContext({
    now: () => sampleTimes.shift() ?? new Date("2026-08-15T19:20:05Z"),
    output,
  });
  const { boundary, zone } = context;

  try {
    const snapshots = [...boundary.snapshots];
    const updatedOutput = {
      ...output,
      volume: { ...output.volume, value: 35 },
    };
    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...zone,
          outputs: [updatedOutput],
        },
      ],
    });

    assert.deepEqual(boundary.snapshots, snapshots);
    assert.equal(boundary.imageRequests.length, 1);
  } finally {
    await context.cleanup();
  }
});

test("transitions once and cleans up when artwork identity changes", async () => {
  const context = await prepareArtworkTestContext({
    image: "first artwork",
    imageKey: "first-artwork-key",
  });
  const { artwork: firstArtwork, artworkDirectory, boundary, zone } = context;

  try {
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...zone,
          now_playing: {
            ...zone.now_playing,
            image_key: "second-artwork-key",
            seek_position: 0,
            three_line: {
              line1: "Across the Room",
              line2: "ODESZA",
              line3: "A Moment Apart",
            },
          },
        },
      ],
    });

    assert.equal(boundary.snapshots.at(-1)?.artwork, null);
    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["first-artwork-key", "second-artwork-key"],
    );

    boundary.resolveImage("image/jpeg", Buffer.from("second artwork"));
    await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);
    await waitFor(async () => (await readdir(artworkDirectory)).length === 1);

    const transitionSnapshots = boundary.snapshots.slice(snapshotCount);
    const secondArtwork = transitionSnapshots.at(-1)?.artwork;
    assert.equal(transitionSnapshots.length, 2);
    assert.equal(transitionSnapshots[0]?.artwork, null);
    assert.notDeepEqual(secondArtwork, firstArtwork);
    assert.equal(secondArtwork?.revision, transitionSnapshots.at(-1)?.revision);
    assert.equal(
      await readFile(secondArtwork?.path ?? "", "utf8"),
      "second artwork",
    );
  } finally {
    await context.cleanup();
  }
});

test("leaves absent prepared display lines absent without inventing fallbacks", () => {
  const boundary = createRoonBoundary("output-speaker-system");

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "playing",
        now_playing: { three_line: {} },
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
      },
    ],
  });

  assert.deepEqual(boundary.snapshots.at(-1)?.nowPlaying, {
    title: null,
    artist: null,
    album: null,
  });
});

test("a stale artwork response cannot delete a newer presentation file", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bridge-test."),
  );
  const artworkDirectory = path.join(taskDirectory, "artwork");
  const artworkFiles = await ArtworkFileStore.open(artworkDirectory);
  const boundary = createRoonBoundary("output-speaker-system", artworkFiles);
  const trackedZone = {
    zone_id: "zone-living-room",
    display_name: "Living Room",
    state: "playing" as const,
    now_playing: {
      image_key: "stale-artwork-key",
      three_line: { line1: "A Moment Apart", line2: "ODESZA" },
    },
    outputs: [
      { output_id: "output-speaker-system", display_name: "Speaker System" },
    ],
  };
  const updatedTrackedZone = {
    ...trackedZone,
    now_playing: {
      ...trackedZone.now_playing,
      image_key: "current-artwork-key",
    },
  };

  try {
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", { zones: [trackedZone] });
    boundary.resolveImageRequest(0, "image/jpeg", Buffer.from("stale artwork"));
    boundary.emitZones("Changed", { zones_changed: [updatedTrackedZone] });
    boundary.resolveImageRequest(
      1,
      "image/jpeg",
      Buffer.from("current artwork"),
    );
    await waitFor(() => boundary.snapshots.at(-1)?.artwork !== null);
    for (let turn = 0; turn < 10; turn += 1) {
      await new Promise<void>((resolve) => setImmediate(resolve));
    }

    const currentArtworkPath = boundary.snapshots.at(-1)?.artwork?.path;
    assert.ok(currentArtworkPath);
    await waitFor(async () => (await readdir(artworkDirectory)).length === 1);
    assert.equal(await readFile(currentArtworkPath, "utf8"), "current artwork");
  } finally {
    await artworkFiles.clear();
    await rm(taskDirectory, { recursive: true });
  }
});

async function waitFor(
  condition: () => boolean | Promise<boolean>,
): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await condition()) {
      return;
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 1));
  }
  assert.fail("Timed out waiting for asynchronous bridge work");
}

test("publishes meaningful progress from the Tracked Zone timing sample", async () => {
  const boundary = createRoonBoundary("output-speaker-system");

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "playing",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
        now_playing: {
          seek_position: 82,
          length: 234,
        },
      },
    ],
  });

  assert.deepEqual(boundary.snapshots.at(-1)?.progress, {
    positionSeconds: 82,
    durationSeconds: 234,
    sampledAt: "2026-08-15T19:20:00.000Z",
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

test("clamps source progress at duration", () => {
  const boundary = createRoonBoundary("output-speaker-system");

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "playing",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
        now_playing: { seek_position: 300, length: 234 },
      },
    ],
  });

  assert.equal(boundary.snapshots.at(-1)?.progress?.positionSeconds, 234);
});

test("omits progress when Roon timing is not meaningful", () => {
  const invalidTiming = [
    { seek_position: Number.NaN, length: 234 },
    { seek_position: Number.POSITIVE_INFINITY, length: 234 },
    { seek_position: -1, length: 234 },
    { seek_position: 82, length: 0 },
    { seek_position: 82, length: -1 },
    { seek_position: 82, length: Number.POSITIVE_INFINITY },
    { seek_position: 82 },
    { length: 234 },
  ];

  for (const nowPlaying of invalidTiming) {
    const boundary = createRoonBoundary("output-speaker-system");
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", {
      zones: [
        {
          zone_id: "zone-living-room",
          display_name: "Living Room",
          state: "playing",
          outputs: [
            {
              output_id: "output-speaker-system",
              display_name: "Speaker System",
            },
          ],
          now_playing: nowPlaying,
        },
      ],
    });

    assert.equal(boundary.snapshots.at(-1)?.progress, null);
  }
});

test("publishes each playback state and clears timing when stopped", () => {
  const boundary = createRoonBoundary("output-speaker-system");
  boundary.extensionOptions().core_paired(boundary.core());
  const zone: Omit<RoonZone, "state"> = {
    zone_id: "zone-living-room",
    display_name: "Living Room",
    outputs: [
      { output_id: "output-speaker-system", display_name: "Speaker System" },
    ],
    now_playing: {
      seek_position: 82,
      length: 234,
      three_line: {
        line1: "A Moment Apart",
        line2: "ODESZA",
        line3: "A Moment Apart",
      },
    },
  };

  boundary.emitZones("Subscribed", {
    zones: [{ ...zone, state: "playing" }],
  });
  for (const state of ["paused", "loading", "stopped"] as const) {
    boundary.emitZones("Changed", {
      zones_changed: [{ ...zone, state }],
    });
  }

  assert.deepEqual(
    boundary.snapshots.slice(-4).map((snapshot) => ({
      playback: snapshot.playback,
      title: snapshot.nowPlaying?.title ?? null,
      progress: snapshot.progress?.positionSeconds ?? null,
    })),
    [
      { playback: "playing", title: "A Moment Apart", progress: 82 },
      { playback: "paused", title: "A Moment Apart", progress: 82 },
      { playback: "loading", title: "A Moment Apart", progress: 82 },
      { playback: "stopped", title: null, progress: null },
    ],
  );
});

test("merges a seek-position-only delta before publishing a complete snapshot", async () => {
  const sampleTimes = [
    new Date("2026-08-15T19:20:00Z"),
    new Date("2026-08-15T19:20:05Z"),
  ];
  const boundary = createRoonBoundary(
    "output-speaker-system",
    undefined,
    () => sampleTimes.shift() ?? new Date("2026-08-15T19:20:05Z"),
  );
  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "playing",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
        now_playing: {
          seek_position: 82,
          length: 234,
          three_line: {
            line1: "A Moment Apart",
            line2: "ODESZA",
            line3: "A Moment Apart",
          },
        },
      },
    ],
  });

  boundary.emitZones("Changed", {
    zones_seek_changed: [{ zone_id: "zone-living-room", seek_position: 30 }],
  });

  assert.deepEqual(boundary.snapshots.at(-1), {
    schemaVersion: 2,
    revision: 3,
    availability: "available",
    playback: "playing",
    trackedOutput: { name: "Speaker System" },
    trackedZone: { name: "Living Room" },
    nowPlaying: {
      title: "A Moment Apart",
      artist: "ODESZA",
      album: "A Moment Apart",
    },
    progress: {
      positionSeconds: 30,
      durationSeconds: 234,
      sampledAt: "2026-08-15T19:20:05.000Z",
    },
    artwork: null,
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

test("follows the configured Tracked Output through grouping and ungrouping", () => {
  const boundary = createRoonBoundary("output-speaker-system");
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "playing",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
      },
      {
        zone_id: "zone-kitchen",
        display_name: "Kitchen",
        state: "playing",
        outputs: [
          { output_id: "output-kitchen", display_name: "Kitchen Speaker" },
        ],
      },
    ],
  });

  boundary.emitZones("Changed", {
    zones_removed: ["zone-living-room", "zone-kitchen"],
    zones_added: [
      {
        zone_id: "zone-whole-home",
        display_name: "Whole Home",
        state: "playing",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
          { output_id: "output-kitchen", display_name: "Kitchen Speaker" },
        ],
      },
    ],
  });
  assert.deepEqual(boundary.snapshots.at(-1)?.trackedZone, {
    name: "Whole Home",
  });
  assert.deepEqual(boundary.snapshots.at(-1)?.trackedOutput, {
    name: "Speaker System",
  });

  boundary.emitZones("Changed", {
    zones_removed: ["zone-whole-home"],
    zones_added: [
      {
        zone_id: "zone-living-room-new",
        display_name: "Living Room",
        state: "paused",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
      },
      {
        zone_id: "zone-kitchen-new",
        display_name: "Kitchen",
        state: "playing",
        outputs: [
          { output_id: "output-kitchen", display_name: "Kitchen Speaker" },
        ],
      },
    ],
  });

  assert.deepEqual(
    boundary.snapshots.slice(-2).map((snapshot) => ({
      revision: snapshot.revision,
      playback: snapshot.playback,
      trackedOutput: snapshot.trackedOutput,
      trackedZone: snapshot.trackedZone,
    })),
    [
      {
        revision: 3,
        playback: "playing",
        trackedOutput: { name: "Speaker System" },
        trackedZone: { name: "Whole Home" },
      },
      {
        revision: 4,
        playback: "paused",
        trackedOutput: { name: "Speaker System" },
        trackedZone: { name: "Living Room" },
      },
    ],
  );
});

test("publishes Tracked Output and Zone renames but ignores unrelated zones", () => {
  let sampledSecond = 0;
  const boundary = createRoonBoundary(
    "output-speaker-system",
    undefined,
    () =>
      new Date(`2026-08-15T19:20:${String(sampledSecond++).padStart(2, "0")}Z`),
  );
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "paused",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
        now_playing: { seek_position: 82, length: 234 },
      },
      {
        zone_id: "zone-office",
        display_name: "Office",
        state: "paused",
        outputs: [
          { output_id: "output-office", display_name: "Office Speaker" },
        ],
      },
    ],
  });
  const snapshotCount = boundary.snapshots.length;

  boundary.emitZones("Changed", {
    zones_changed: [
      {
        zone_id: "zone-office",
        display_name: "Office",
        state: "playing",
        outputs: [
          { output_id: "output-office", display_name: "Office Speaker" },
        ],
      },
    ],
  });

  assert.equal(boundary.snapshots.length, snapshotCount);
  assert.deepEqual(boundary.snapshots.at(-1)?.trackedZone, {
    name: "Living Room",
  });
  assert.deepEqual(boundary.snapshots.at(-1)?.trackedOutput, {
    name: "Speaker System",
  });

  boundary.emitZones("Changed", {
    zones_changed: [
      {
        zone_id: "zone-living-room",
        display_name: "Listening Room",
        state: "paused",
        outputs: [
          { output_id: "output-speaker-system", display_name: "USB DAC" },
        ],
      },
    ],
  });

  assert.equal(boundary.snapshots.length, snapshotCount + 1);
  assert.deepEqual(boundary.snapshots.at(-1)?.trackedZone, {
    name: "Listening Room",
  });
  assert.deepEqual(boundary.snapshots.at(-1)?.trackedOutput, {
    name: "USB DAC",
  });
});

test("clears presentation state when the configured Tracked Output is removed", async () => {
  const boundary = createRoonBoundary("output-speaker-system");
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "playing",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
      },
      {
        zone_id: "zone-office",
        display_name: "Office",
        state: "playing",
        outputs: [
          { output_id: "output-office", display_name: "Office Speaker" },
        ],
      },
    ],
  });
  boundary.emitZones("Changed", {
    zones_removed: ["zone-living-room"],
  });

  assert.deepEqual(boundary.snapshots.at(-1), {
    schemaVersion: 2,
    revision: 3,
    availability: "outputUnavailable",
    playback: null,
    trackedOutput: null,
    trackedZone: null,
    nowPlaying: null,
    progress: null,
    artwork: null,
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

test("registers the public extension identity and only observer services", () => {
  const boundary = createRoonBoundary();
  let extensionOptions: RoonExtensionOptions | undefined;
  let initializedServices: unknown;
  let discoveryStarted = false;
  boundary.extension.init_services = (services) => {
    initializedServices = {
      keys: Object.keys(services),
      names: [
        ...services.required_services,
        ...services.provided_services,
      ].flatMap((service) => service.services.map(({ name }) => name)),
    };
  };
  boundary.extension.start_discovery = () => {
    discoveryStarted = true;
  };

  startRoonBridge({
    authorizationStore: boundary.authorizationStore,
    artworkFiles: unusedArtworkFiles(),
    displayConfigurationStore: {
      load: () => null,
      save: () => undefined,
    },
    createRoonServices: (options) => {
      extensionOptions = options;
      return {
        extension: boundary.extension,
        requiredServices: [
          { services: [{ name: "com.roonlabs.image:1" }] },
          { services: [{ name: "com.roonlabs.transport:2" }] },
        ],
        status: {
          services: [{ name: "com.roonlabs.status:1" }],
          set_status: () => undefined,
        },
      };
    },
    publish: () => undefined,
  });

  assert.deepEqual(
    {
      identity: {
        extension_id: extensionOptions?.extension_id,
        display_name: extensionOptions?.display_name,
        publisher: extensionOptions?.publisher,
        email: extensionOptions?.email,
        website: extensionOptions?.website,
      },
      initializedServices,
      discoveryStarted,
    },
    {
      identity: {
        extension_id: "io.roonscape.bridge",
        display_name: "RoonScape",
        publisher: "Gregory Bartell",
        email: "5353310+gregbartell@users.noreply.github.com",
        website: "https://github.com/gregbartell/roonscape",
      },
      initializedServices: {
        keys: ["required_services", "provided_services"],
        names: [
          "com.roonlabs.image:1",
          "com.roonlabs.transport:2",
          "com.roonlabs.status:1",
        ],
      },
      discoveryStarted: true,
    },
  );
});

test("reports each availability condition through Roon extension status", () => {
  const boundary = createRoonBoundary();
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  extensionOptions.core_unpaired(boundary.core());

  assert.deepEqual(boundary.statusUpdates, [
    {
      message: "Pairing required: enable RoonScape in a Roon client",
      isError: false,
    },
    {
      message: "Connected: Tracked Output unavailable",
      isError: false,
    },
    { message: "Disconnected from Roon", isError: true },
  ]);
});
