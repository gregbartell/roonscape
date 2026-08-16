import assert from "node:assert/strict";
import test from "node:test";

import { discoverTrackedOutputs } from "../src/tracked-output-discovery.js";
import type { RoonExtensionOptions, RoonServices } from "../src/roon-bridge.js";

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
                zone_id: "zone-gallery",
                display_name: "Gallery",
                state: "paused",
                outputs: [
                  {
                    output_id: "output-gallery",
                    display_name: "NUC HDMI",
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
      trackedOutputId: "output-gallery",
      trackedOutputName: "NUC HDMI",
      trackedZoneName: "Gallery",
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

test("indefinite discovery can be cancelled cleanly for Retry or Quit", async () => {
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
