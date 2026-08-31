import assert from "node:assert/strict";
import test from "node:test";

import { discoverTrackedOutputs } from "../src/tracked-output-discovery.js";
import type {
  RoonExtension,
  RoonExtensionOptions,
  RoonServices,
} from "../src/roon-bridge.js";
import { createSupportedRoonServices } from "../src/roon-services.js";

interface DelayedDiscoveryExtension extends RoonExtension {
  _sood: {
    on(): void;
    query(): void;
    start(callback: () => void): void;
    stop(): void;
  };
  _sood_conns: Record<string, never>;
  scanIntervalId: NodeJS.Timeout | 0;
}

test("discovers physical Tracked Outputs from Roon's initial full zone state", async () => {
  let extensionOptions: RoonExtensionOptions | undefined;
  let discoveryStarted = false;
  let discoveryStopped = false;
  let disconnected = false;
  const services: RoonServices = {
    extension: {
      init_services: () => undefined,
      start_discovery: () => {
        discoveryStarted = true;
      },
      stop_discovery: () => {
        discoveryStopped = true;
      },
      disconnect_all: () => {
        disconnected = true;
      },
    },
    requiredServices: [{ services: [{ name: "com.roonlabs.transport:2" }] }],
    status: {
      services: [{ name: "com.roonlabs.status:1" }],
      set_status: () => undefined,
    },
  };
  const discovery = discoverTrackedOutputs({
    authorizationStore: { load: () => ({}), save: () => undefined },
    createRoonServices: (options) => {
      extensionOptions = options;
      return services;
    },
    timeoutMilliseconds: 1_000,
  });

  assert.ok(extensionOptions);
  extensionOptions.core_paired({
    core_id: "core-1",
    services: {
      RoonApiTransport: {
        subscribe_zones: (listener) =>
          listener("Subscribed", {
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
              },
              {
                zone_id: "zone-group",
                display_name: "Downstairs",
                state: "playing",
                outputs: [
                  {
                    output_id: "output-kitchen",
                    display_name: "Kitchen Speaker",
                  },
                  {
                    output_id: "output-living",
                    display_name: "Living Room Speaker",
                  },
                ],
              },
            ],
          }),
      },
    },
  });

  assert.equal(discoveryStarted, true);
  assert.deepEqual(await discovery, [
    {
      trackedOutputId: "output-speaker-system",
      trackedOutputName: "Speaker System",
      trackedZoneName: "Living Room",
    },
    {
      trackedOutputId: "output-kitchen",
      trackedOutputName: "Kitchen Speaker",
      trackedZoneName: "Downstairs",
    },
    {
      trackedOutputId: "output-living",
      trackedOutputName: "Living Room Speaker",
      trackedZoneName: "Downstairs",
    },
  ]);
  assert.deepEqual(
    { discoveryStopped, disconnected },
    {
      discoveryStopped: true,
      disconnected: true,
    },
  );
});

test("indefinite discovery can be cancelled cleanly", async () => {
  let discoveryStopped = false;
  let disconnected = false;
  const controller = new AbortController();
  const discovery = discoverTrackedOutputs({
    authorizationStore: { load: () => ({}), save: () => undefined },
    createRoonServices: () => ({
      extension: {
        init_services: () => undefined,
        start_discovery: () => undefined,
        stop_discovery: () => {
          discoveryStopped = true;
        },
        disconnect_all: () => {
          disconnected = true;
        },
      },
      requiredServices: [{ services: [] }],
      status: { services: [], set_status: () => undefined },
    }),
    timeoutMilliseconds: null,
    signal: controller.signal,
  });

  controller.abort();

  await assert.rejects(discovery, { name: "AbortError" });
  assert.deepEqual(
    { discoveryStopped, disconnected },
    { discoveryStopped: true, disconnected: true },
  );
});

test("successful discovery stays stopped after deferred Roon API startup", async () => {
  let extensionOptions: RoonExtensionOptions | undefined;
  let extension: DelayedDiscoveryExtension | undefined;
  const discovery = discoverTrackedOutputs({
    authorizationStore: { load: () => ({}), save: () => undefined },
    createRoonServices: (options) => {
      extensionOptions = options;
      const services = createSupportedRoonServices(options);
      extension = services.extension as DelayedDiscoveryExtension;
      extension._sood = {
        on: () => undefined,
        query: () => undefined,
        start: (callback) => setTimeout(callback, 20),
        stop: () => undefined,
      };
      extension._sood_conns = {};
      return services;
    },
    timeoutMilliseconds: null,
  });
  assert.ok(extensionOptions);
  extensionOptions.core_paired({
    core_id: "core-1",
    services: {
      RoonApiTransport: {
        subscribe_zones: (listener) => listener("Subscribed", { zones: [] }),
      },
    },
  });

  await discovery;
  await new Promise((resolve) => setTimeout(resolve, 250));

  assert.ok(extension);
  try {
    assert.equal(extension.scanIntervalId, 0);
  } finally {
    clearInterval(extension.scanIntervalId);
  }
});
