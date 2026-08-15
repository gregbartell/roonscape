import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, readdir, rm } from "node:fs/promises";
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
  resolveImage(contentType: string, image: Buffer): void;
  resolveImageRequest(index: number, contentType: string, image: Buffer): void;
  snapshots: PresentationSnapshot[];
  statusUpdates: Array<{ message: string; isError: boolean }>;
}

function createRoonBoundary(
  displayOutputId?: string,
  artworkFiles: ArtworkFiles = unusedArtworkFiles(),
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
  const authorizationStore: AuthorizationStore = {
    load: () => persistedState,
    save: (state) => {
      persistedState = state;
    },
  };
  const displayConfigurationStore: DisplayConfigurationStore = {
    load: () => (displayOutputId === undefined ? null : { displayOutputId }),
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
    imageRequests,
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

function unusedArtworkFiles(): ArtworkFiles {
  return {
    stage: () => Promise.reject(new Error("Artwork was not expected")),
    commit: () => Promise.resolve(),
    discard: () => Promise.resolve(),
    clear: () => Promise.resolve(),
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
      schemaVersion: 1,
      revision,
      availability,
      playback: null,
      displayZone: null,
      nowPlaying: null,
      progress: null,
      artwork: null,
    })),
  );

  await Promise.all(boundary.snapshots.map(validateSnapshot));
});

test("resolves the configured Display Output from the initial full zone state", async () => {
  const boundary = createRoonBoundary("output-gallery");
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
        zone_id: "zone-gallery",
        display_name: "Gallery",
        state: "paused",
        now_playing: {
          three_line: {
            line1: "A Moment Apart",
            line2: "ODESZA",
            line3: "A Moment Apart",
          },
        },
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
      },
    ],
  });

  assert.deepEqual(boundary.snapshots.at(-1), {
    schemaVersion: 1,
    revision: 2,
    availability: "available",
    playback: "paused",
    displayZone: { name: "Gallery" },
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
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const artworkDirectory = path.join(taskDirectory, "artwork");
  const artworkFiles = await ArtworkFileStore.open(artworkDirectory);
  const boundary = createRoonBoundary("output-gallery", artworkFiles);

  try {
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", {
      zones: [
        {
          zone_id: "zone-gallery",
          display_name: "Gallery",
          state: "playing",
          now_playing: {
            image_key: "opaque-roon-image-key",
            three_line: {
              line1: "A Moment Apart",
              line2: "ODESZA",
              line3: "A Moment Apart",
            },
          },
          outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
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
        schemaVersion: 1,
        revision: 3,
        availability: "available",
        playback: "playing",
        displayZone: { name: "Gallery" },
        nowPlaying: {
          title: "A Moment Apart",
          artist: "ODESZA",
          album: "A Moment Apart",
        },
        progress: null,
        artwork: { revision: 3 },
      },
    );
    assert.equal(path.dirname(snapshot?.artwork?.path ?? ""), artworkDirectory);
    assert.match(
      path.basename(snapshot?.artwork?.path ?? ""),
      /^artwork-3-.+\.jpg$/,
    );
    await validateSnapshot(snapshot);
    assert.equal(
      await readFile(snapshot?.artwork?.path ?? "", "utf8"),
      "compressed artwork",
    );

    boundary.emitZones("Changed", {
      zones_changed: [
        {
          zone_id: "zone-gallery",
          display_name: "Gallery",
          state: "playing",
          now_playing: {
            image_key: "opaque-roon-image-key",
            three_line: {
              line1: "Across the Room",
              line2: "ODESZA",
              line3: "A Moment Apart",
            },
          },
          outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
        },
      ],
    });
    boundary.resolveImage("image/jpeg", Buffer.from("revised artwork"));
    await waitFor(async () => {
      const files = await readdir(artworkDirectory);
      return files.length === 1 && /^artwork-5-.+\.jpg$/.test(files[0] ?? "");
    });

    assert.equal(boundary.snapshots.at(-1)?.artwork?.revision, 5);
    assert.match(
      path.basename(boundary.snapshots.at(-1)?.artwork?.path ?? ""),
      /^artwork-5-.+\.jpg$/,
    );
    assert.equal(
      await readFile(boundary.snapshots.at(-1)?.artwork?.path ?? "", "utf8"),
      "revised artwork",
    );
  } finally {
    await artworkFiles.clear();
    await rm(taskDirectory, { recursive: true });
  }
});

test("leaves absent prepared display lines absent without inventing fallbacks", () => {
  const boundary = createRoonBoundary("output-gallery");

  boundary.extensionOptions().core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-gallery",
        display_name: "Gallery",
        state: "playing",
        now_playing: { three_line: {} },
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
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
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const artworkDirectory = path.join(taskDirectory, "artwork");
  const artworkFiles = await ArtworkFileStore.open(artworkDirectory);
  const boundary = createRoonBoundary("output-gallery", artworkFiles);
  const displayZone = {
    zone_id: "zone-gallery",
    display_name: "Gallery",
    state: "playing" as const,
    now_playing: {
      image_key: "reused-opaque-key",
      three_line: { line1: "A Moment Apart", line2: "ODESZA" },
    },
    outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
  };

  try {
    boundary.extensionOptions().core_paired(boundary.core());
    boundary.emitZones("Subscribed", { zones: [displayZone] });
    boundary.resolveImageRequest(0, "image/jpeg", Buffer.from("stale artwork"));
    boundary.emitZones("Changed", { zones_changed: [displayZone] });
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

test("follows the configured Display Output through grouping and ungrouping", () => {
  const boundary = createRoonBoundary("output-gallery");
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-gallery",
        display_name: "Gallery",
        state: "playing",
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
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
    zones_removed: ["zone-gallery", "zone-kitchen"],
    zones_added: [
      {
        zone_id: "zone-whole-home",
        display_name: "Whole Home",
        state: "playing",
        outputs: [
          { output_id: "output-gallery", display_name: "NUC HDMI" },
          { output_id: "output-kitchen", display_name: "Kitchen Speaker" },
        ],
      },
    ],
  });
  assert.deepEqual(boundary.snapshots.at(-1)?.displayZone, {
    name: "Whole Home",
  });

  boundary.emitZones("Changed", {
    zones_removed: ["zone-whole-home"],
    zones_added: [
      {
        zone_id: "zone-gallery-new",
        display_name: "Gallery",
        state: "paused",
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
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
      displayZone: snapshot.displayZone,
    })),
    [
      {
        revision: 3,
        playback: "playing",
        displayZone: { name: "Whole Home" },
      },
      {
        revision: 4,
        playback: "paused",
        displayZone: { name: "Gallery" },
      },
    ],
  );
});

test("follows Display Zone renames but ignores unrelated active-zone changes", () => {
  const boundary = createRoonBoundary("output-gallery");
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-gallery",
        display_name: "Gallery",
        state: "paused",
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
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
  assert.deepEqual(boundary.snapshots.at(-1)?.displayZone, {
    name: "Gallery",
  });

  boundary.emitZones("Changed", {
    zones_changed: [
      {
        zone_id: "zone-gallery",
        display_name: "Listening Room",
        state: "paused",
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
      },
    ],
  });

  assert.equal(boundary.snapshots.length, snapshotCount + 1);
  assert.deepEqual(boundary.snapshots.at(-1)?.displayZone, {
    name: "Listening Room",
  });
});

test("clears presentation state when the configured Display Output is removed", async () => {
  const boundary = createRoonBoundary("output-gallery");
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired(boundary.core());
  boundary.emitZones("Subscribed", {
    zones: [
      {
        zone_id: "zone-gallery",
        display_name: "Gallery",
        state: "playing",
        outputs: [{ output_id: "output-gallery", display_name: "NUC HDMI" }],
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
    zones_removed: ["zone-gallery"],
  });

  assert.deepEqual(boundary.snapshots.at(-1), {
    schemaVersion: 1,
    revision: 3,
    availability: "outputUnavailable",
    playback: null,
    displayZone: null,
    nowPlaying: null,
    progress: null,
    artwork: null,
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

test("registers only observer services and extension Status", () => {
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
      },
      initializedServices,
      discoveryStarted,
    },
    {
      identity: {
        extension_id: "io.roonscape.bridge",
        display_name: "RoonScape",
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
      message: "Connected: Display Output unavailable",
      isError: false,
    },
    { message: "Disconnected from Roon", isError: true },
  ]);
});
