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
import type {
  LyricFeedConnectionFactory,
  PrivateLyricEvent,
} from "../src/lyric-feed.js";
import { trackedNowPlaying } from "../src/lyric-feed.js";
import type {
  DisplayConfiguration,
  DisplayConfigurationStore,
} from "../src/display-configuration.js";
import { assertSnapshotPublishable } from "../src/fixture-publisher.js";
import {
  type PresentationSnapshot,
  validateSnapshot,
} from "../src/snapshot.js";
import { parseSynchronizedLyrics } from "../src/synchronized-lyrics.js";

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
  publicationDiagnostics: string[];
  savedConfigurations: DisplayConfiguration[];
  currentSnapshot(): PresentationSnapshot;
  lyricsVisible(revision: number): void;
  stop(): Promise<void>;
}

function createRoonBoundary(
  configuredOutput?: string | DisplayConfiguration,
  artworkFiles: ArtworkFiles = unusedArtworkFiles(),
  now: () => Date = () => new Date("2026-08-15T19:20:00Z"),
  createLyricFeedConnection?: LyricFeedConnectionFactory,
): RoonBoundary {
  let persistedState: unknown = {};
  let capturedOptions: RoonExtensionOptions | undefined;
  let zoneListener:
    | ((response: RoonZoneSubscriptionResponse, event: RoonZoneEvent) => void)
    | undefined;
  const snapshots: PresentationSnapshot[] = [];
  const statusUpdates: Array<{ message: string; isError: boolean }> = [];
  const publicationDiagnostics: string[] = [];
  const savedConfigurations: DisplayConfiguration[] = [];
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
  let displayConfiguration =
    typeof configuredOutput === "string"
      ? { trackedOutputId: configuredOutput }
      : (configuredOutput ?? null);
  const displayConfigurationStore: DisplayConfigurationStore = {
    load: () => displayConfiguration,
    save: (configuration) => {
      displayConfiguration = configuration;
      savedConfigurations.push(configuration);
    },
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
  const core: RoonCore & {
    moo?: { transport: { host: string; port: number } };
  } = {
    core_id: "core-1",
    moo:
      createLyricFeedConnection === undefined
        ? undefined
        : { transport: { host: "roon.local", port: 9330 } },
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

  const bridge = startRoonBridge({
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
    publish: (snapshot) => {
      assertSnapshotPublishable(snapshot);
      snapshots.push(snapshot);
    },
    reportPublicationFailure: (reason) => publicationDiagnostics.push(reason),
    scheduleArtworkRetry: (retry, delayMilliseconds) => {
      const scheduledRetry = { cancelled: false, retry };
      artworkRetries.push(scheduledRetry);
      retryDelays.push(delayMilliseconds);
      return () => {
        scheduledRetry.cancelled = true;
      };
    },
    now,
    createLyricFeedConnection,
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
    publicationDiagnostics,
    savedConfigurations,
    currentSnapshot: () => bridge.currentSnapshot(),
    lyricsVisible: (revision) => bridge.lyricsVisible(revision),
    stop: () => bridge.stop(),
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

test("stop attempts service, discovery, and artwork cleanup and preserves every failure", async () => {
  const events: string[] = [];
  const boundary = createRoonBoundary(undefined, {
    ...unusedArtworkFiles(),
    clear: async () => {
      events.push("artwork cleared");
      throw new Error("artwork cleanup failed");
    },
  });
  boundary.extension.disconnect_all = () => {
    events.push("services disconnected");
    throw new Error("service cleanup failed");
  };
  boundary.extension.stop_discovery = () => {
    events.push("discovery stopped");
    throw new Error("discovery cleanup failed");
  };

  await assert.rejects(boundary.stop(), (error) => {
    assert.ok(error instanceof AggregateError);
    assert.deepEqual(
      error.errors.map((failure: unknown) =>
        failure instanceof Error ? failure.message : String(failure),
      ),
      [
        "discovery cleanup failed",
        "service cleanup failed",
        "artwork cleanup failed",
      ],
    );
    return true;
  });
  assert.deepEqual(events, [
    "discovery stopped",
    "services disconnected",
    "artwork cleared",
  ]);
});

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
  createLyricFeedConnection,
  output = {
    output_id: "output-speaker-system",
    display_name: "Speaker System",
  },
}: {
  image?: string;
  imageKey?: string;
  now?: () => Date;
  createLyricFeedConnection?: LyricFeedConnectionFactory;
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
    createLyricFeedConnection,
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
      schemaVersion: 3,
      revision,
      availability,
      playback: null,
      trackedOutput: null,
      trackedZone: null,
      nowPlaying: null,
      progress: null,
      artwork: null,
      lyrics: null,
    })),
  );

  await Promise.all(boundary.snapshots.map(validateSnapshot));
});

test("publishes the saved Tracked Output identity only when that output is unavailable", async () => {
  const boundary = createRoonBoundary({
    trackedOutputId: "output-speaker-system",
    trackedOutputName: "Speaker System",
  });
  const extensionOptions = boundary.extensionOptions();
  const core = boundary.core();

  extensionOptions.core_paired(core);
  assert.deepEqual(boundary.snapshots.at(-1), {
    schemaVersion: 3,
    revision: 1,
    availability: "outputUnavailable",
    playback: null,
    trackedOutput: { name: "Speaker System" },
    trackedZone: null,
    nowPlaying: null,
    progress: null,
    artwork: null,
    lyrics: null,
  });

  extensionOptions.core_unpaired(core);
  assert.equal(boundary.snapshots.at(-1)?.availability, "disconnected");
  assert.equal(boundary.snapshots.at(-1)?.trackedOutput, null);
  await Promise.all(boundary.snapshots.map(validateSnapshot));
});

test("backfills and refreshes the persisted Tracked Output name", () => {
  const inactivity = {
    gracePeriodSeconds: 240,
    dimmedOpacity: 0.3,
    repositionCadenceSeconds: 45,
  };
  const boundary = createRoonBoundary({
    trackedOutputId: "output-speaker-system",
    inactivity,
  });

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "stopped",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Speaker System",
          },
        ],
      },
    ],
  });
  boundary.emitZones("Changed", {
    zones_changed: [
      {
        zone_id: "zone-living-room",
        display_name: "Living Room",
        state: "stopped",
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "Main Speakers",
          },
        ],
      },
    ],
  });

  assert.deepEqual(boundary.savedConfigurations, [
    {
      trackedOutputId: "output-speaker-system",
      trackedOutputName: "Speaker System",
      inactivity,
    },
    {
      trackedOutputId: "output-speaker-system",
      trackedOutputName: "Main Speakers",
      inactivity,
    },
  ]);
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
    schemaVersion: 3,
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
    lyrics: null,
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
        schemaVersion: 3,
        revision: 2,
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
        artwork: { revision: 2 },
        lyrics: null,
      },
    );
    assert.equal(path.dirname(snapshot?.artwork?.path ?? ""), artworkDirectory);
    assert.match(
      path.basename(snapshot?.artwork?.path ?? ""),
      /^artwork-2-.+\.jpg$/,
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

    assert.deepEqual(boundary.snapshots.at(-1), snapshot);
    boundary.resolveImage("image/jpeg", Buffer.from("loading artwork"));
    await waitFor(() => boundary.snapshots.at(-1)?.artwork?.revision === 3);
    const loadingSnapshot = boundary.snapshots.at(-1);
    assert.deepEqual(loadingSnapshot, {
      ...snapshot,
      revision: 3,
      playback: "loading",
      artwork: {
        revision: 3,
        path: loadingSnapshot?.artwork?.path ?? "",
      },
    });

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
    assert.deepEqual(boundary.snapshots.at(-1), loadingSnapshot);
    boundary.resolveImage("image/jpeg", Buffer.from("revised artwork"));
    await waitFor(async () => {
      const files = await readdir(artworkDirectory);
      return files.length === 1 && /^artwork-4-.+\.jpg$/.test(files[0] ?? "");
    });

    assert.equal(boundary.snapshots.at(-1)?.artwork?.revision, 4);
    assert.match(
      path.basename(boundary.snapshots.at(-1)?.artwork?.path ?? ""),
      /^artwork-4-.+\.jpg$/,
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
      schemaVersion: 3,
      revision: 5,
      availability: "available",
      playback: "stopped",
      trackedOutput: { name: "Speaker System" },
      trackedZone: { name: "Living Room" },
      nowPlaying: null,
      progress: null,
      artwork: null,
      lyrics: null,
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

test("publishes changed Now Playing and artwork as one Presentation Snapshot", async () => {
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

    assert.equal(boundary.snapshots.length, snapshotCount);
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "A Moment Apart",
    );
    assert.deepEqual(boundary.currentSnapshot().artwork, firstArtwork);
    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["first-artwork-key", "second-artwork-key"],
    );

    boundary.resolveImage("image/jpeg", Buffer.from("second artwork"));
    await waitFor(() => boundary.snapshots.length === snapshotCount + 1);
    await waitFor(async () => (await readdir(artworkDirectory)).length === 1);

    const transitionSnapshots = boundary.snapshots.slice(snapshotCount);
    const secondArtwork = transitionSnapshots.at(-1)?.artwork;
    assert.equal(transitionSnapshots.length, 1);
    assert.equal(transitionSnapshots[0]?.nowPlaying?.title, "Across the Room");
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

test("supersedes pending artwork when grouping selects a retained Tracked Zone", async () => {
  await withArtworkTestBoundary(async (boundary) => {
    const outgoingZone = artworkZone("outgoing-artwork-key", "Outgoing Track");
    const incomingZone: RoonZone = {
      ...artworkZone("incoming-artwork-key", "Incoming Track"),
      zone_id: "zone-whole-home",
      display_name: "Whole Home",
    };

    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", {
      zones: [outgoingZone, incomingZone],
    });
    const snapshotCount = boundary.snapshots.length;

    boundary.emitZones("Changed", {
      zones_removed: [outgoingZone.zone_id],
    });

    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["outgoing-artwork-key", "incoming-artwork-key"],
    );

    boundary.resolveImageRequest(
      0,
      "image/jpeg",
      Buffer.from("superseded artwork"),
    );
    assert.equal(boundary.snapshots.length, snapshotCount);

    boundary.resolveImageRequest(
      1,
      "image/jpeg",
      Buffer.from("incoming artwork"),
    );
    await waitFor(() => boundary.snapshots.length === snapshotCount + 1);
    assert.equal(boundary.currentSnapshot().trackedZone?.name, "Whole Home");
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "Incoming Track",
    );
    assert.equal(
      await readFile(boundary.currentSnapshot().artwork?.path ?? "", "utf8"),
      "incoming artwork",
    );
  });
});

test("clears artwork when grouping selects a retained zone without an image", async () => {
  await withArtworkTestBoundary(async (boundary) => {
    const outgoingZone = artworkZone("outgoing-artwork-key", "Outgoing Track");
    const incomingZone: RoonZone = {
      ...artworkZone("unused-artwork-key", "Incoming Track"),
      zone_id: "zone-whole-home",
      display_name: "Whole Home",
      now_playing: {
        three_line: { line1: "Incoming Track", line2: "Incoming Artist" },
      },
    };

    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", {
      zones: [outgoingZone, incomingZone],
    });
    boundary.resolveImage("image/jpeg", Buffer.from("outgoing artwork"));
    await waitFor(() => boundary.currentSnapshot().artwork !== null);
    const snapshotCount = boundary.snapshots.length;

    boundary.emitZones("Changed", {
      zones_removed: [outgoingZone.zone_id],
    });

    assert.equal(boundary.snapshots.length, snapshotCount + 1);
    assert.equal(boundary.currentSnapshot().trackedZone?.name, "Whole Home");
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "Incoming Track",
    );
    assert.equal(boundary.currentSnapshot().artwork, null);
    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["outgoing-artwork-key"],
    );
  });
});

test("publishes only the latest playback state when incoming artwork arrives", async () => {
  const context = await prepareArtworkTestContext();
  const { boundary, zone } = context;

  try {
    const incomingNowPlaying = {
      image_key: "incoming-artwork-key",
      three_line: { line1: "Incoming Track", line2: "Incoming Artist" },
    };
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", {
      zones_changed: [
        { ...zone, state: "loading", now_playing: incomingNowPlaying },
      ],
    });
    boundary.emitZones("Changed", {
      zones_changed: [
        { ...zone, state: "playing", now_playing: incomingNowPlaying },
      ],
    });

    assert.equal(boundary.snapshots.length, snapshotCount);

    boundary.resolveImage("image/jpeg", Buffer.from("incoming artwork"));
    await waitFor(() => boundary.snapshots.length === snapshotCount + 1);
    assert.equal(boundary.currentSnapshot().playback, "playing");
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "Incoming Track",
    );
    assert.ok(boundary.currentSnapshot().artwork);
  } finally {
    await context.cleanup();
  }
});

test("coalesces metadata that precedes its changed artwork identity", async () => {
  const context = await prepareArtworkTestContext();
  const { boundary, zone } = context;

  try {
    const metadataChanged: RoonZone = {
      ...zone,
      now_playing: {
        ...zone.now_playing,
        three_line: { line1: "Incoming Track", line2: "Incoming Artist" },
      },
    };
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", { zones_changed: [metadataChanged] });

    assert.equal(boundary.snapshots.length, snapshotCount);
    assert.deepEqual(
      boundary.imageRequests.map(({ imageKey }) => imageKey),
      ["same-track-artwork", "same-track-artwork"],
    );

    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...metadataChanged,
          now_playing: {
            ...metadataChanged.now_playing,
            image_key: "incoming-artwork-key",
          },
        },
      ],
    });
    boundary.resolveImageRequest(
      1,
      "image/jpeg",
      Buffer.from("superseded artwork"),
    );
    assert.equal(boundary.snapshots.length, snapshotCount);

    boundary.resolveImageRequest(
      2,
      "image/jpeg",
      Buffer.from("incoming artwork"),
    );
    await waitFor(() => boundary.snapshots.length === snapshotCount + 1);
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "Incoming Track",
    );
    assert.equal(
      await readFile(boundary.currentSnapshot().artwork?.path ?? "", "utf8"),
      "incoming artwork",
    );
  } finally {
    await context.cleanup();
  }
});

test("coalesces an artwork identity that precedes its changed metadata", async () => {
  const context = await prepareArtworkTestContext();
  const { boundary, zone } = context;

  try {
    const artworkIdentityChanged: RoonZone = {
      ...zone,
      now_playing: {
        ...zone.now_playing,
        image_key: "incoming-artwork-key",
      },
    };
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", {
      zones_changed: [artworkIdentityChanged],
    });
    assert.equal(boundary.snapshots.length, snapshotCount);

    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...artworkIdentityChanged,
          now_playing: {
            ...artworkIdentityChanged.now_playing,
            three_line: {
              line1: "Incoming Track",
              line2: "Incoming Artist",
            },
          },
        },
      ],
    });
    assert.equal(boundary.snapshots.length, snapshotCount);

    boundary.resolveImage("image/jpeg", Buffer.from("incoming artwork"));
    await waitFor(() => boundary.snapshots.length === snapshotCount + 1);
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "Incoming Track",
    );
    assert.equal(
      await readFile(boundary.currentSnapshot().artwork?.path ?? "", "utf8"),
      "incoming artwork",
    );
  } finally {
    await context.cleanup();
  }
});

test("holds incoming lyrics with the pending Now Playing composition", async () => {
  let emitLyrics:
    | ((
        event: PrivateLyricEvent,
        observedNowPlayingIdentity?: string | null,
      ) => void)
    | undefined;
  const context = await prepareArtworkTestContext({
    createLyricFeedConnection: (options) => {
      emitLyrics = options.onEvent;
      return { reportViewed: () => undefined, stop: () => undefined };
    },
  });
  const { boundary, zone } = context;

  try {
    const incomingZone: RoonZone = {
      ...zone,
      now_playing: {
        image_key: "incoming-artwork-key",
        seek_position: 0,
        length: 180,
        three_line: { line1: "Incoming Track", line2: "Incoming Artist" },
      },
    };
    const snapshotCount = boundary.snapshots.length;
    boundary.emitZones("Changed", { zones_changed: [incomingZone] });
    emitLyrics?.(
      {
        zone_id: incomingZone.zone_id,
        key: "incoming-lyrics-key",
        lrc: "[00:01.00]Incoming lyric",
      },
      trackedNowPlaying(incomingZone)?.nowPlayingIdentity,
    );

    assert.equal(boundary.snapshots.length, snapshotCount);

    boundary.resolveImage("image/jpeg", Buffer.from("incoming artwork"));
    await waitFor(() => boundary.snapshots.length === snapshotCount + 1);
    assert.equal(
      boundary.currentSnapshot().nowPlaying?.title,
      "Incoming Track",
    );
    assert.equal(
      boundary.currentSnapshot().lyrics?.cues[0]?.text,
      "Incoming lyric",
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
    schemaVersion: 3,
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
    lyrics: null,
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

test("correlates the optional Lyric Feed with current Tracked Zone Now Playing", () => {
  let emitLyrics:
    | ((
        event: PrivateLyricEvent,
        observedNowPlayingIdentity?: string | null,
      ) => void)
    | undefined;
  const reports: string[] = [];
  const connectionEndpoints: unknown[] = [];
  const createLyricFeedConnection: LyricFeedConnectionFactory = (options) => {
    connectionEndpoints.push({
      endpoint: options.endpoint,
      coreId: options.expectedCoreId,
    });
    emitLyrics = options.onEvent;
    return {
      reportViewed: (key) => reports.push(key),
      stop: () => undefined,
    };
  };
  const boundary = createRoonBoundary(
    "output-speaker-system",
    unusedArtworkFiles(),
    () => new Date("2026-08-15T19:20:00Z"),
    createLyricFeedConnection,
  );
  boundary.extensionOptions().core_paired(boundary.core());
  const trackAZone: RoonZone = {
    ...artworkZone("track-a-artwork", "Track A"),
    now_playing: {
      seek_position: 1,
      length: 120,
      three_line: { line1: "Track A", line2: "Artist" },
    },
  };
  boundary.emitZones("Subscribed", {
    zones: [trackAZone],
  });

  emitLyrics?.({
    zone_id: "zone-office",
    key: "wrong-zone-key",
    lrc: "[00:01.00]Wrong zone",
  });
  emitLyrics?.(
    {
      zone_id: "zone-living-room",
      key: "track-a-key",
      lrc: "[00:01.00]First\n[00:04.00]Second",
    },
    trackedNowPlaying(trackAZone)?.nowPlayingIdentity,
  );

  assert.deepEqual(connectionEndpoints, [
    { endpoint: { host: "roon.local", port: 9330 }, coreId: "core-1" },
  ]);
  assert.deepEqual(boundary.currentSnapshot().lyrics, {
    cues: [
      { atSeconds: 1, text: "First" },
      { atSeconds: 4, text: "Second" },
    ],
  });
  assert.deepEqual(reports, []);
  boundary.lyricsVisible(boundary.currentSnapshot().revision);
  boundary.lyricsVisible(boundary.currentSnapshot().revision);
  assert.deepEqual(reports, ["track-a-key"]);

  const trackTransitionSnapshotStart = boundary.snapshots.length;
  const trackBZone: RoonZone = {
    ...artworkZone("track-b-artwork", "Track B"),
    now_playing: {
      seek_position: 0,
      length: 140,
      three_line: { line1: "Track B", line2: "Artist" },
    },
  };
  emitLyrics?.(
    {
      zone_id: "zone-living-room",
      key: "track-b-key",
      lrc: "[00:01.00]Track B cue",
    },
    trackedNowPlaying(trackBZone)?.nowPlayingIdentity,
  );
  assert.equal(boundary.currentSnapshot().nowPlaying?.title, "Track A");
  assert.equal(boundary.currentSnapshot().lyrics?.cues[0]?.text, "First");

  boundary.emitZones("Changed", {
    zones_changed: [trackBZone],
  });
  assert.equal(boundary.currentSnapshot().nowPlaying?.title, "Track B");
  assert.equal(boundary.currentSnapshot().lyrics?.cues[0]?.text, "Track B cue");
  assert.ok(
    boundary.snapshots
      .slice(trackTransitionSnapshotStart)
      .every(
        (snapshot) =>
          snapshot.nowPlaying?.title !== "Track A" ||
          snapshot.lyrics?.cues[0]?.text !== "Track B cue",
      ),
  );

  emitLyrics?.(
    {
      zone_id: "zone-living-room",
      key: "track-b-key",
      lrc: "[00:02.00]Track B updated cue",
    },
    trackedNowPlaying(trackBZone)?.nowPlayingIdentity,
  );
  assert.equal(
    boundary.currentSnapshot().lyrics?.cues[0]?.text,
    "Track B updated cue",
  );
  emitLyrics?.(
    {
      zone_id: "zone-living-room",
      key: "track-a-key",
      lrc: "[00:02.00]Late stale text",
    },
    trackedNowPlaying(trackBZone)?.nowPlayingIdentity,
  );

  assert.equal(boundary.currentSnapshot().nowPlaying?.title, "Track B");
  assert.equal(
    boundary.currentSnapshot().lyrics?.cues[0]?.text,
    "Track B updated cue",
  );
});

test("publishes an accepted lyric timeline when progress becomes meaningful", () => {
  let emitLyrics: ((event: PrivateLyricEvent) => void) | undefined;
  const boundary = createRoonBoundary(
    "output-speaker-system",
    unusedArtworkFiles(),
    () => new Date("2026-08-15T19:20:00Z"),
    (options) => {
      emitLyrics = options.onEvent;
      return { reportViewed: () => undefined, stop: () => undefined };
    },
  );
  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        ...artworkZone("track-a-artwork", "Track A"),
        now_playing: {
          length: 120,
          three_line: { line1: "Track A", line2: "Artist" },
        },
      },
    ],
  });
  emitLyrics?.({
    zone_id: "zone-living-room",
    key: "track-a-key",
    lrc: "[00:01.00]First\n[00:04.00]Second",
  });
  assert.equal(boundary.currentSnapshot().lyrics, null);

  boundary.emitZones("Changed", {
    zones_seek_changed: [{ zone_id: "zone-living-room", seek_position: 2 }],
  });

  assert.deepEqual(boundary.currentSnapshot().lyrics, {
    cues: [
      { atSeconds: 1, text: "First" },
      { atSeconds: 4, text: "Second" },
    ],
  });
});

test("drops an unpublishable lyric timeline without blocking ordinary snapshots", () => {
  let emitLyrics: ((event: PrivateLyricEvent) => void) | undefined;
  const boundary = createRoonBoundary(
    "output-speaker-system",
    unusedArtworkFiles(),
    () => new Date("2026-08-15T19:20:00Z"),
    (options) => {
      emitLyrics = options.onEvent;
      return { reportViewed: () => undefined, stop: () => undefined };
    },
  );
  boundary.extensionOptions().core_paired(boundary.core());
  const initialZone: RoonZone = {
    ...artworkZone("track-a-artwork", "Track A"),
    now_playing: {
      seek_position: 1,
      length: 300,
      three_line: { line1: "Track A", line2: "Artist" },
    },
  };
  boundary.emitZones("Subscribed", { zones: [initialZone] });

  const largeLrc = Array.from({ length: 256 }, (_, index) => {
    const minutes = String(Math.floor(index / 60)).padStart(2, "0");
    const seconds = String(index % 60).padStart(2, "0");
    return `[${minutes}:${seconds}.00]${"🎵".repeat(60)}`;
  }).join("\n");
  assert.ok(Buffer.byteLength(largeLrc, "utf8") <= 64 * 1024);
  const largeTimeline = parseSynchronizedLyrics(largeLrc);
  assert.ok(largeTimeline);
  assert.throws(
    () =>
      assertSnapshotPublishable({
        ...boundary.currentSnapshot(),
        revision: boundary.currentSnapshot().revision + 1,
        lyrics: largeTimeline,
      }),
    /Snapshot exceeds 64 KiB/u,
  );
  emitLyrics?.({
    zone_id: "zone-living-room",
    key: "large-track-a-key",
    lrc: largeLrc,
  });

  assert.equal(boundary.currentSnapshot().lyrics, null);
  assert.deepEqual(boundary.publicationDiagnostics, []);

  boundary.emitZones("Changed", {
    zones_changed: [
      {
        ...initialZone,
        now_playing: {
          ...initialZone.now_playing,
          three_line: { line1: "Track A (Remastered)", line2: "Artist" },
        },
      },
    ],
  });

  assert.equal(
    boundary.currentSnapshot().nowPlaying?.title,
    "Track A (Remastered)",
  );
  assert.equal(boundary.currentSnapshot().lyrics, null);
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
    schemaVersion: 3,
    revision: 3,
    availability: "outputUnavailable",
    playback: null,
    trackedOutput: { name: "Speaker System" },
    trackedZone: null,
    nowPlaying: null,
    progress: null,
    artwork: null,
    lyrics: null,
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

test("rejects invalid Live Mode candidates atomically and recovers publication status", () => {
  const boundary = createRoonBoundary("output-speaker-system");
  boundary.extensionOptions().core_paired(boundary.core());
  const validZone: RoonZone = {
    zone_id: "zone-living-room",
    display_name: "Living Room",
    state: "playing",
    outputs: [
      { output_id: "output-speaker-system", display_name: "Speaker System" },
    ],
    now_playing: {
      three_line: {
        line1: "Last Light on Phobos",
        line2: "Evelyn Lark",
        line3: "Signals from the Quiet Sea",
      },
    },
  };
  boundary.emitZones("Subscribed", { zones: [validZone] });
  const lastValid = boundary.snapshots.at(-1);
  assert.ok(lastValid);

  const invalidUnicodeTitle = "rejected-unicode-\ud800";
  for (let attempt = 0; attempt < 2; attempt += 1) {
    boundary.emitZones("Changed", {
      zones_changed: [
        {
          ...validZone,
          now_playing: {
            image_key: "rejected-artwork",
            three_line: {
              ...validZone.now_playing?.three_line,
              line1: invalidUnicodeTitle,
            },
          },
        },
      ],
    });
  }

  const oversizedTitle = `rejected-title-${"x".repeat(1_025)}`;
  boundary.emitZones("Changed", {
    zones_changed: [
      {
        ...validZone,
        now_playing: {
          image_key: "rejected-artwork",
          three_line: {
            ...validZone.now_playing?.three_line,
            line1: oversizedTitle,
          },
        },
      },
    ],
  });
  boundary.emitZones("Changed", {
    zones_changed: [
      {
        ...validZone,
        now_playing: {
          image_key: "rejected-artwork",
          three_line: {
            ...validZone.now_playing?.three_line,
            line1: oversizedTitle,
          },
        },
      },
    ],
  });

  const oversizedOutput = `rejected-output-${"x".repeat(257)}`;
  boundary.emitZones("Changed", {
    zones_changed: [
      {
        ...validZone,
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: oversizedOutput,
          },
        ],
      },
    ],
  });

  assert.deepEqual(boundary.snapshots.at(-1), lastValid);
  assert.deepEqual(boundary.currentSnapshot(), lastValid);
  assert.deepEqual(boundary.imageRequests, []);
  assert.deepEqual(boundary.publicationDiagnostics, [
    "Title contains invalid Unicode",
    "Title exceeds 1,024 Unicode code points",
    "Tracked Output name exceeds 256 Unicode code points",
  ]);
  assert.ok(
    boundary.publicationDiagnostics.every(
      (diagnostic) =>
        !diagnostic.includes(oversizedTitle) &&
        !diagnostic.includes(oversizedOutput) &&
        !diagnostic.includes(invalidUnicodeTitle),
    ),
  );
  assert.deepEqual(boundary.statusUpdates.slice(-3), [
    {
      message: "Publication failed: Title contains invalid Unicode",
      isError: true,
    },
    {
      message: "Publication failed: Title exceeds 1,024 Unicode code points",
      isError: true,
    },
    {
      message:
        "Publication failed: Tracked Output name exceeds 256 Unicode code points",
      isError: true,
    },
  ]);

  boundary.emitZones("Changed", {
    zones_changed: [validZone],
  });

  assert.equal(boundary.snapshots.at(-1)?.revision, lastValid.revision + 1);
  assert.equal(
    boundary.snapshots.at(-1)?.nowPlaying?.title,
    "Last Light on Phobos",
  );
  assert.deepEqual(boundary.statusUpdates.at(-1), {
    message: "Connected",
    isError: false,
  });
});

test("republishes the last valid Idle state to recover publication status", () => {
  const boundary = createRoonBoundary("output-speaker-system");
  boundary.extensionOptions().core_paired(boundary.core());
  const idleZone: RoonZone = {
    zone_id: "zone-living-room",
    display_name: "Living Room",
    state: "stopped",
    outputs: [
      { output_id: "output-speaker-system", display_name: "Speaker System" },
    ],
  };
  boundary.emitZones("Subscribed", { zones: [idleZone] });
  const lastValidRevision = boundary.currentSnapshot().revision;

  boundary.emitZones("Changed", {
    zones_changed: [
      {
        ...idleZone,
        outputs: [
          {
            output_id: "output-speaker-system",
            display_name: "x".repeat(257),
          },
        ],
      },
    ],
  });
  assert.equal(boundary.currentSnapshot().revision, lastValidRevision);
  assert.equal(boundary.statusUpdates.at(-1)?.isError, true);

  boundary.emitZones("Changed", { zones_changed: [idleZone] });

  assert.equal(boundary.currentSnapshot().revision, lastValidRevision + 1);
  assert.equal(boundary.currentSnapshot().playback, "stopped");
  assert.deepEqual(boundary.statusUpdates.at(-1), {
    message: "Connected",
    isError: false,
  });
});
