import assert from "node:assert/strict";
import test from "node:test";

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
  snapshots: PresentationSnapshot[];
  statusUpdates: Array<{ message: string; isError: boolean }>;
}

function createRoonBoundary(displayOutputId?: string): RoonBoundary {
  let persistedState: unknown = {};
  let capturedOptions: RoonExtensionOptions | undefined;
  let zoneListener:
    | ((response: RoonZoneSubscriptionResponse, event: RoonZoneEvent) => void)
    | undefined;
  const snapshots: PresentationSnapshot[] = [];
  const statusUpdates: Array<{ message: string; isError: boolean }> = [];
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
      RoonApiTransport: {
        subscribe_zones: (listener) => {
          zoneListener = listener;
        },
      },
    },
  };

  startRoonBridge({
    authorizationStore,
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
    snapshots,
    statusUpdates,
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
    nowPlaying: null,
    progress: null,
    artwork: null,
  });
  await validateSnapshot(boundary.snapshots.at(-1));
});

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
