import assert from "node:assert/strict";
import test from "node:test";

import {
  startRoonAvailabilityBridge,
  type AuthorizationStore,
  type RoonExtension,
  type RoonExtensionOptions,
  type RoonStatusService,
} from "../src/roon-availability.js";
import {
  type PresentationSnapshot,
  validateSnapshot,
} from "../src/snapshot.js";

interface RoonBoundary {
  authorizationStore: AuthorizationStore;
  extension: RoonExtension;
  extensionOptions(): RoonExtensionOptions;
  snapshots: PresentationSnapshot[];
  statusUpdates: Array<{ message: string; isError: boolean }>;
}

function createRoonBoundary(): RoonBoundary {
  let persistedState: unknown = {};
  let capturedOptions: RoonExtensionOptions | undefined;
  const snapshots: PresentationSnapshot[] = [];
  const statusUpdates: Array<{ message: string; isError: boolean }> = [];
  const authorizationStore: AuthorizationStore = {
    load: () => persistedState,
    save: (state) => {
      persistedState = state;
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

  startRoonAvailabilityBridge({
    authorizationStore,
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
  extensionOptions.core_paired({ core_id: "core-1" });
  extensionOptions.core_unpaired({ core_id: "core-1" });
  extensionOptions.core_paired({ core_id: "core-1" });

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

test("registers RoonScape with only read-only Image and extension Status services", () => {
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

  startRoonAvailabilityBridge({
    authorizationStore: boundary.authorizationStore,
    createRoonServices: (options) => {
      extensionOptions = options;
      return {
        extension: boundary.extension,
        requiredServices: [{ services: [{ name: "com.roonlabs.image:1" }] }],
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
        names: ["com.roonlabs.image:1", "com.roonlabs.status:1"],
      },
      discoveryStarted: true,
    },
  );
});

test("reports each availability condition through Roon extension status", () => {
  const boundary = createRoonBoundary();
  const extensionOptions = boundary.extensionOptions();

  extensionOptions.core_paired({ core_id: "core-1" });
  extensionOptions.core_unpaired({ core_id: "core-1" });

  assert.deepEqual(boundary.statusUpdates, [
    {
      message: "Pairing required: enable RoonScape in a Roon client",
      isError: false,
    },
    {
      message: "Connected: Display Output is not configured",
      isError: false,
    },
    { message: "Disconnected from Roon", isError: true },
  ]);
});
