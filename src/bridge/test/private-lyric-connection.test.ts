import assert from "node:assert/strict";
import test from "node:test";

import {
  createPrivateLyricFeedConnection,
  lyricFeedIdentity,
  type PrivateRoonApi,
  type PrivateRoonApiOptions,
  type PrivateZoneSubscriptionEvent,
} from "../src/private-lyric-connection.js";

test("uses the paired Roon Server, first-party identity, and inert display service", () => {
  let options: PrivateRoonApiOptions | undefined;
  let initialized:
    { required_services: unknown[]; provided_services: unknown[] } | undefined;
  let endpoint: unknown;
  const methodNames: string[] = [];
  const extension: PrivateRoonApi = {
    init_services: (services) => {
      initialized = services;
    },
    register_service: (_name, specification) => {
      methodNames.push(...Object.keys(specification.methods));
      return { name: "com.roonlabs.zonedisplay:1" };
    },
    ws_connect: (nextEndpoint) => {
      endpoint = { host: nextEndpoint.host, port: nextEndpoint.port };
      return { transport: { close: () => undefined } };
    },
  };

  createPrivateLyricFeedConnection(
    {
      endpoint: { host: "roon.local", port: 9330 },
      expectedCoreId: "core-1",
      onEvent: () => undefined,
      onDisconnect: () => undefined,
    },
    (nextOptions) => {
      options = nextOptions;
      return {
        extension,
        requiredServices: [
          { services: [{ name: "com.roonlabs.image:1" }] },
          { services: [{ name: "com.roonlabs.transport:2" }] },
        ],
      };
    },
  );

  assert.deepEqual(
    {
      identity: {
        extension_id: options?.extension_id,
        display_name: options?.display_name,
        display_version: options?.display_version,
        publisher: options?.publisher,
      },
      endpoint,
      required: initialized?.required_services,
      provided: initialized?.provided_services,
      methods: methodNames.sort(),
    },
    {
      identity: lyricFeedIdentity,
      endpoint: { host: "roon.local", port: 9330 },
      required: [
        { services: [{ name: "com.roonlabs.image:1" }] },
        { services: [{ name: "com.roonlabs.transport:2" }] },
      ],
      provided: [{ services: [{ name: "com.roonlabs.zonedisplay:1" }] }],
      methods: [
        "activate",
        "deactivate",
        "get_displays",
        "subscribe_displays",
        "unsubscribe_displays",
        "update_settings",
      ],
    },
  );
});

test("keeps registration state in memory and never reports or activates implicitly", () => {
  let options: PrivateRoonApiOptions | undefined;
  let closeCount = 0;
  let disconnectCount = 0;
  const requests: Array<{ name: string; body: unknown }> = [];
  const extension: PrivateRoonApi = {
    init_services: () => undefined,
    register_service: () => ({ name: "com.roonlabs.zonedisplay:1" }),
    ws_connect: () => ({
      transport: {
        close: () => {
          closeCount += 1;
        },
      },
    }),
  };
  const connection = createPrivateLyricFeedConnection(
    {
      endpoint: { host: "roon.local", port: 9330 },
      expectedCoreId: "core-1",
      onEvent: () => undefined,
      onDisconnect: () => {
        disconnectCount += 1;
      },
    },
    (nextOptions) => {
      options = nextOptions;
      return {
        extension,
        requiredServices: [],
      };
    },
  );

  options?.set_persisted_state({ tokens: { "core-1": "memory-only" } });
  assert.deepEqual(options?.get_persisted_state(), {
    tokens: { "core-1": "memory-only" },
  });
  const core = {
    core_id: "core-1",
    services: {
      RoonApiTransport: {
        subscribe_zones: () => undefined,
      },
    },
    moo: {
      send_request: (name: string, body: unknown) =>
        requests.push({ name, body }),
    },
  };
  options?.core_paired(core);
  connection.reportViewed("lyric-key");
  options?.core_unpaired(core);
  assert.deepEqual(options?.get_persisted_state(), {});
  connection.stop();

  assert.deepEqual(requests, [
    {
      name: "com.roonlabs.transport:2/report_lrc_viewed",
      body: { key: "lyric-key" },
    },
  ]);
  assert.equal(closeCount, 1);
  assert.equal(disconnectCount, 1);
});

test("correlates lyrics with Now Playing from the same private zone subscription", () => {
  let options: PrivateRoonApiOptions | undefined;
  let zoneListener:
    | ((response: string, event: PrivateZoneSubscriptionEvent) => void)
    | undefined;
  const observed: Array<{
    event: unknown;
    nowPlayingIdentity?: string | null;
  }> = [];
  const extension: PrivateRoonApi = {
    init_services: () => undefined,
    register_service: () => ({ name: "com.roonlabs.zonedisplay:1" }),
    ws_connect: () => ({ transport: { close: () => undefined } }),
  };
  createPrivateLyricFeedConnection(
    {
      endpoint: { host: "roon.local", port: 9330 },
      expectedCoreId: "core-1",
      onEvent: (event, nowPlayingIdentity) =>
        observed.push({ event, nowPlayingIdentity }),
      onDisconnect: () => undefined,
    },
    (nextOptions) => {
      options = nextOptions;
      return { extension, requiredServices: [] };
    },
  );
  options?.core_paired({
    core_id: "core-1",
    services: {
      RoonApiTransport: {
        subscribe_zones: (listener) => {
          zoneListener = listener;
        },
      },
    },
  });
  const zone = {
    zone_id: "zone-a",
    state: "playing",
    now_playing: {
      image_key: "track-b-artwork",
      length: 140,
      three_line: { line1: "Track B", line2: "Artist" },
    },
  };
  zoneListener?.("Subscribed", { zones: [zone] });
  const lyricEvent = {
    zone_id: "zone-a",
    key: "track-b-key",
    lrc: "[00:01.00]Track B cue",
  };
  zoneListener?.("LyricsChanged", lyricEvent);

  assert.deepEqual(observed, [
    {
      event: lyricEvent,
      nowPlayingIdentity: JSON.stringify([
        "track-b-artwork",
        140,
        { line1: "Track B", line2: "Artist" },
      ]),
    },
  ]);
});
