import type { Availability, PresentationSnapshot } from "./snapshot.js";
import type { DisplayConfigurationStore } from "./display-configuration.js";
import { initializeRoonExtension } from "./roon-extension.js";

type Unavailable = Exclude<Availability, "available">;
type SnapshotState = Omit<PresentationSnapshot, "revision">;

export interface AuthorizationStore {
  load(): unknown;
  save(state: unknown): void;
}

export interface RoonCore {
  core_id: string;
  services: {
    RoonApiTransport: RoonTransportService;
  };
}

export interface RoonOutput {
  output_id: string;
  display_name: string;
}

export interface RoonNowPlaying {
  seek_position?: number;
  length?: number;
}

export interface RoonZone {
  zone_id: string;
  display_name: string;
  state: NonNullable<PresentationSnapshot["playback"]>;
  outputs: RoonOutput[];
  now_playing?: RoonNowPlaying;
}

export type RoonZoneSubscriptionResponse =
  "Subscribed" | "Changed" | "Unsubscribed";

export interface RoonZoneEvent {
  zones?: RoonZone[];
  zones_added?: RoonZone[];
  zones_changed?: RoonZone[];
  zones_seek_changed?: Array<{ zone_id: string; seek_position: number }>;
  zones_removed?: string[];
}

export interface RoonTransportService {
  subscribe_zones(
    callback: (
      response: RoonZoneSubscriptionResponse,
      event: RoonZoneEvent,
    ) => void,
  ): void;
}

export interface RoonExtensionOptions {
  extension_id: string;
  display_name: string;
  display_version: string;
  publisher: string;
  email: string;
  website: string;
  log_level: string;
  core_paired(core: RoonCore): void;
  core_unpaired(core: RoonCore): void;
  get_persisted_state(): unknown;
  set_persisted_state(state: unknown): void;
}

export interface RoonServiceDescriptor {
  services: Array<{ name: string }>;
}

export interface RoonStatusService extends RoonServiceDescriptor {
  set_status(message: string, isError: boolean): void;
}

export interface RoonExtension {
  init_services(services: {
    required_services: RoonServiceDescriptor[];
    provided_services: RoonServiceDescriptor[];
  }): void;
  start_discovery(): void;
  stop_discovery(): void;
  disconnect_all(): void;
}

export interface RoonServices {
  extension: RoonExtension;
  requiredServices: RoonServiceDescriptor[];
  status: RoonStatusService;
}

export interface RoonBridge {
  currentSnapshot(): PresentationSnapshot;
  stop(): void;
}

export type CreateRoonServices = (
  options: RoonExtensionOptions,
) => RoonServices;

interface StartRoonBridgeOptions {
  authorizationStore: AuthorizationStore;
  displayConfigurationStore: DisplayConfigurationStore;
  createRoonServices: CreateRoonServices;
  publish(snapshot: PresentationSnapshot): void;
  now?: () => Date;
}

interface RetainedZone {
  zone: RoonZone;
  sampledAt: string;
}

export function startRoonBridge({
  authorizationStore,
  displayConfigurationStore,
  createRoonServices,
  publish,
  now = () => new Date(),
}: StartRoonBridgeOptions): RoonBridge {
  let revision = 0;
  const initialAvailability: Unavailable = hasAuthorization(
    authorizationStore.load(),
  )
    ? "disconnected"
    : "pairingRequired";
  let currentSnapshot = unavailableSnapshot(revision, initialAvailability);
  let updateStatus: (availability: Availability) => void = () => undefined;
  let activeCore: RoonCore | undefined;

  const publishState = (state: SnapshotState): void => {
    if (samePresentation(currentSnapshot, state)) {
      return;
    }

    revision += 1;
    currentSnapshot = { revision, ...state };
    publish(currentSnapshot);
    updateStatus(state.availability);
  };

  const changeAvailability = (availability: Unavailable): void => {
    publishState(unavailableState(availability));
  };

  const services = initializeRoonExtension({
    authorizationStore,
    createRoonServices,
    corePaired: (core) => {
      activeCore = core;
      changeAvailability("outputUnavailable");
      const configuration = displayConfigurationStore.load();
      if (configuration === null) {
        return;
      }

      const zones = new Map<string, RetainedZone>();
      core.services.RoonApiTransport.subscribe_zones((response, event) => {
        if (activeCore !== core) {
          return;
        }

        const sampledAt = now().toISOString();
        if (response === "Subscribed") {
          zones.clear();
          for (const zone of event.zones ?? []) {
            zones.set(zone.zone_id, { zone, sampledAt });
          }
        } else if (response === "Changed") {
          for (const zoneId of event.zones_removed ?? []) {
            zones.delete(zoneId);
          }
          for (const zone of [
            ...(event.zones_added ?? []),
            ...(event.zones_changed ?? []),
          ]) {
            zones.set(zone.zone_id, { zone, sampledAt });
          }
          for (const seekChange of event.zones_seek_changed ?? []) {
            const retainedZone = zones.get(seekChange.zone_id);
            if (retainedZone?.zone.now_playing === undefined) {
              continue;
            }

            zones.set(seekChange.zone_id, {
              sampledAt,
              zone: {
                ...retainedZone.zone,
                now_playing: {
                  ...retainedZone.zone.now_playing,
                  seek_position: seekChange.seek_position,
                },
              },
            });
          }
        } else {
          return;
        }

        const displayZone = [...zones.values()].find(({ zone }) =>
          zone.outputs.some(
            (output) => output.output_id === configuration.displayOutputId,
          ),
        );
        if (displayZone === undefined) {
          changeAvailability("outputUnavailable");
          return;
        }

        publishState(availableState(displayZone));
      });
    },
    coreUnpaired: (core) => {
      if (activeCore !== core) {
        return;
      }
      activeCore = undefined;
      changeAvailability("disconnected");
    },
  });
  const status = services.status;
  updateStatus = (availability) => setExtensionStatus(status, availability);
  publish(currentSnapshot);
  setExtensionStatus(status, currentSnapshot.availability);
  services.extension.start_discovery();

  return {
    currentSnapshot: () => currentSnapshot,
    stop: () => {
      services.extension.stop_discovery();
      services.extension.disconnect_all();
    },
  };
}

export function initialAvailabilitySnapshot(
  authorizationStore: AuthorizationStore,
): PresentationSnapshot {
  return unavailableSnapshot(
    0,
    hasAuthorization(authorizationStore.load())
      ? "disconnected"
      : "pairingRequired",
  );
}

function unavailableSnapshot(
  revision: number,
  availability: Unavailable,
): PresentationSnapshot {
  return {
    revision,
    ...unavailableState(availability),
  };
}

function unavailableState(availability: Unavailable): SnapshotState {
  return {
    schemaVersion: 1,
    availability,
    playback: null,
    displayZone: null,
    nowPlaying: null,
    progress: null,
    artwork: null,
  };
}

function availableState({ zone, sampledAt }: RetainedZone): SnapshotState {
  return {
    schemaVersion: 1,
    availability: "available",
    playback: zone.state,
    displayZone: { name: zone.display_name },
    nowPlaying: null,
    progress:
      zone.state === "stopped"
        ? null
        : meaningfulProgress(zone.now_playing, sampledAt),
    artwork: null,
  };
}

function meaningfulProgress(
  nowPlaying: RoonNowPlaying | undefined,
  sampledAt: string,
): PresentationSnapshot["progress"] {
  const positionSeconds = nowPlaying?.seek_position;
  const durationSeconds = nowPlaying?.length;
  if (
    positionSeconds === undefined ||
    !Number.isFinite(positionSeconds) ||
    positionSeconds < 0 ||
    durationSeconds === undefined ||
    !Number.isFinite(durationSeconds) ||
    durationSeconds <= 0
  ) {
    return null;
  }

  return {
    positionSeconds: Math.min(positionSeconds, durationSeconds),
    durationSeconds,
    sampledAt,
  };
}

function samePresentation(
  snapshot: PresentationSnapshot,
  state: SnapshotState,
): boolean {
  return (
    JSON.stringify(snapshot) ===
    JSON.stringify({ revision: snapshot.revision, ...state })
  );
}

function hasAuthorization(state: unknown): boolean {
  if (!isRecord(state) || !isRecord(state.tokens)) {
    return false;
  }

  return Object.keys(state.tokens).length > 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function setExtensionStatus(
  status: RoonStatusService,
  availability: Availability,
): void {
  const extensionStatus: Record<
    Availability,
    { message: string; isError: boolean }
  > = {
    pairingRequired: {
      message: "Pairing required: enable RoonScape in a Roon client",
      isError: false,
    },
    disconnected: { message: "Disconnected from Roon", isError: true },
    outputUnavailable: {
      message: "Connected: Display Output unavailable",
      isError: false,
    },
    available: { message: "Connected", isError: false },
  };
  const nextStatus = extensionStatus[availability];
  status.set_status(nextStatus.message, nextStatus.isError);
}
