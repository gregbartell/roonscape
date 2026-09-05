import RoonApi from "node-roon-api";
import RoonApiImage from "node-roon-api-image";
import RoonApiTransport from "node-roon-api-transport";

import type {
  LyricNowPlayingZone,
  LyricFeedEndpoint,
  LyricFeedConnection,
  LyricFeedConnectionFactory,
  PrivateLyricEvent,
} from "./lyric-feed.js";
import { trackedNowPlaying } from "./lyric-feed.js";
import type { RoonExtensionOptions } from "./roon-bridge.js";

export const lyricFeedIdentity = {
  extension_id: "com.roonlabs.display_zone",
  display_name: "Roon API Display Zone",
  display_version: "1.0.0",
  publisher: "Roon Labs, LLC",
} as const;

interface PrivateRequest {
  send_complete(name: string, body?: unknown): void;
  send_continue(name: string, body?: unknown): void;
}

interface PrivateServiceSpecification {
  subscriptions: Array<{
    subscribe_name: string;
    unsubscribe_name: string;
    start(request: PrivateRequest): void;
  }>;
  methods: Record<string, (request: PrivateRequest) => void>;
}

export interface PrivateRoonApi {
  init_services(services: {
    required_services: unknown[];
    provided_services: unknown[];
  }): void;
  register_service(
    name: string,
    specification: PrivateServiceSpecification,
  ): { name: string };
  ws_connect(options: LyricFeedEndpoint & { onclose?: () => void }): {
    transport: { close(): void };
  };
}

interface PrivateRoonCore {
  core_id: string;
  moo?: {
    send_request?(name: string, body: unknown, callback?: () => void): void;
  };
  services: {
    RoonApiTransport: {
      subscribe_zones(
        callback: (
          response: string,
          event: PrivateZoneSubscriptionEvent,
        ) => void,
      ): void;
    };
  };
}

export interface PrivateZoneSubscriptionEvent extends PrivateLyricEvent {
  zones?: LyricNowPlayingZone[];
  zones_added?: LyricNowPlayingZone[];
  zones_changed?: LyricNowPlayingZone[];
  zones_removed?: string[];
}

export type PrivateRoonApiOptions = Omit<
  RoonExtensionOptions,
  "core_paired" | "core_unpaired"
> & {
  core_paired(core: PrivateRoonCore): void;
  core_unpaired(core: PrivateRoonCore): void;
};

interface PrivateRoonServices {
  extension: PrivateRoonApi;
  requiredServices: unknown[];
}

type CreatePrivateRoonServices = (
  options: PrivateRoonApiOptions,
) => PrivateRoonServices;

export function createPrivateLyricFeedConnection(
  {
    endpoint,
    expectedCoreId,
    onEvent,
    onDisconnect,
  }: Parameters<LyricFeedConnectionFactory>[0],
  createServices: CreatePrivateRoonServices = createPrivateRoonServices,
): LyricFeedConnection {
  let registrationState: unknown = {};
  let core: PrivateRoonCore | undefined;
  let stopped = false;
  let disconnected = false;
  let socket: { transport: { close(): void } } | undefined;
  const zones = new Map<string, LyricNowPlayingZone>();
  const disconnect = (): void => {
    core = undefined;
    zones.clear();
    registrationState = {};
    if (!stopped && !disconnected) {
      disconnected = true;
      onDisconnect();
    }
  };
  const services = createServices({
    ...lyricFeedIdentity,
    email: "display@roonlabs.com",
    website: "https://roonlabs.com/",
    log_level: "none",
    get_persisted_state: () => registrationState,
    set_persisted_state: (state) => {
      registrationState = state;
    },
    core_paired: (pairedCore) => {
      if (pairedCore.core_id !== expectedCoreId) {
        socket?.transport.close();
        return;
      }
      core = pairedCore;
      pairedCore.services.RoonApiTransport.subscribe_zones(
        (response, event) => {
          if (response === "Subscribed") {
            zones.clear();
            for (const zone of event.zones ?? []) {
              zones.set(zone.zone_id, zone);
            }
            return;
          }
          if (response === "Changed") {
            for (const zoneId of event.zones_removed ?? []) {
              zones.delete(zoneId);
            }
            for (const zone of [
              ...(event.zones_added ?? []),
              ...(event.zones_changed ?? []),
            ]) {
              zones.set(zone.zone_id, zone);
            }
            return;
          }
          if (response === "LyricsChanged") {
            const zone =
              typeof event.zone_id === "string"
                ? zones.get(event.zone_id)
                : undefined;
            onEvent(
              event,
              zone === undefined
                ? null
                : (trackedNowPlaying(zone)?.nowPlayingIdentity ?? null),
            );
          }
        },
      );
    },
    core_unpaired: (unpairedCore) => {
      if (core === unpairedCore) {
        socket?.transport.close();
        socket = undefined;
        disconnect();
      }
    },
  });
  const zoneDisplay = services.extension.register_service(
    "com.roonlabs.zonedisplay:1",
    inertZoneDisplaySpecification(),
  );
  services.extension.init_services({
    required_services: services.requiredServices,
    provided_services: [{ services: [zoneDisplay] }],
  });
  socket = services.extension.ws_connect({
    ...endpoint,
    onclose: () => {
      socket = undefined;
      disconnect();
    },
  });

  return {
    reportViewed: (key) => {
      core?.moo?.send_request?.("com.roonlabs.transport:2/report_lrc_viewed", {
        key,
      });
    },
    stop: () => {
      if (stopped) {
        return;
      }
      stopped = true;
      core = undefined;
      zones.clear();
      registrationState = {};
      socket?.transport.close();
      socket = undefined;
    },
  };
}

export function lyricFeedEndpointForCore(
  core: unknown,
): LyricFeedEndpoint | null {
  if (!isRecord(core) || !isRecord(core.moo) || !isRecord(core.moo.transport)) {
    return null;
  }
  const { host, port } = core.moo.transport;
  return typeof host === "string" &&
    typeof port === "number" &&
    Number.isInteger(port) &&
    port > 0 &&
    port <= 65_535
    ? { host, port }
    : null;
}

function createPrivateRoonServices(
  options: PrivateRoonApiOptions,
): PrivateRoonServices {
  return {
    extension: new RoonApi(
      options as unknown as RoonExtensionOptions,
    ) as unknown as PrivateRoonApi,
    requiredServices: [RoonApiImage, RoonApiTransport],
  };
}

function inertZoneDisplaySpecification(): PrivateServiceSpecification {
  const completeWithoutDisplays = (request: PrivateRequest): void => {
    request.send_complete("Success", { displays: [] });
  };
  const acknowledgeWithoutMutation = (request: PrivateRequest): void => {
    request.send_complete("Success");
  };
  return {
    subscriptions: [
      {
        subscribe_name: "subscribe_displays",
        unsubscribe_name: "unsubscribe_displays",
        start: (request) =>
          request.send_continue("Subscribed", { displays: [] }),
      },
    ],
    methods: {
      get_displays: completeWithoutDisplays,
      subscribe_displays: () => undefined,
      unsubscribe_displays: () => undefined,
      activate: acknowledgeWithoutMutation,
      deactivate: acknowledgeWithoutMutation,
      update_settings: acknowledgeWithoutMutation,
    },
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
