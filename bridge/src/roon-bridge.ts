import type { Availability, PresentationSnapshot } from "./snapshot.js";
import type { ArtworkFiles } from "./artwork-file-store.js";
import type { DisplayConfigurationStore } from "./display-configuration.js";
import { initializeRoonExtension } from "./roon-extension.js";

type Unavailable = Exclude<Availability, "available">;
type SnapshotState = Omit<PresentationSnapshot, "revision">;
type PublishState = (state: SnapshotState) => boolean;

export interface AuthorizationStore {
  load(): unknown;
  save(state: unknown): void;
}

export interface RoonCore {
  core_id: string;
  services: {
    RoonApiImage?: RoonImageService;
    RoonApiTransport: RoonTransportService;
  };
}

export interface RoonImageOptions {
  scale: "fit";
  width: number;
  height: number;
  format: "image/jpeg";
}

export interface RoonImageService {
  get_image(
    imageKey: string,
    options: RoonImageOptions,
    callback: (
      error: string | false,
      contentType?: string,
      image?: Buffer,
    ) => void,
  ): void;
}

export interface RoonOutput {
  output_id: string;
  display_name: string;
}

export interface RoonNowPlaying {
  image_key?: string;
  seek_position?: number;
  length?: number;
  three_line?: {
    line1?: string;
    line2?: string;
    line3?: string;
  };
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
  stop(): Promise<void>;
}

export type CreateRoonServices = (
  options: RoonExtensionOptions,
) => RoonServices;

interface StartRoonBridgeOptions {
  authorizationStore: AuthorizationStore;
  artworkFiles: ArtworkFiles;
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
  artworkFiles,
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

  const publishState: PublishState = (state) => {
    if (samePresentation(currentSnapshot, state)) {
      return false;
    }

    revision += 1;
    currentSnapshot = { revision, ...state };
    publish(currentSnapshot);
    updateStatus(state.availability);
    return true;
  };
  const artworkPresentation = new ArtworkPresentationCoordinator({
    artworkFiles,
    publishState,
    currentRevision: () => revision,
    currentSnapshot: () => currentSnapshot,
  });

  const changeAvailability = (availability: Unavailable): void => {
    publishState(unavailableState(availability));
    void artworkPresentation.cancelAndClear().catch(reportArtworkError);
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
            const retainedZone = zones.get(zone.zone_id);
            zones.set(zone.zone_id, {
              zone,
              sampledAt:
                retainedZone !== undefined &&
                sameZonePresentationSource(retainedZone.zone, zone)
                  ? retainedZone.sampledAt
                  : sampledAt,
            });
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

        const displayZoneWasUpdated =
          response === "Subscribed" ||
          [...(event.zones_added ?? []), ...(event.zones_changed ?? [])].some(
            (zone) => zone.zone_id === displayZone.zone.zone_id,
          );
        artworkPresentation.present(core, displayZone, displayZoneWasUpdated);
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
    stop: async () => {
      services.extension.stop_discovery();
      services.extension.disconnect_all();
      await artworkPresentation.cancelAndClear();
    },
  };
}

interface ArtworkPresentationCoordinatorOptions {
  artworkFiles: ArtworkFiles;
  publishState: PublishState;
  currentRevision(): number;
  currentSnapshot(): PresentationSnapshot;
}

class ArtworkPresentationCoordinator {
  readonly #artworkFiles: ArtworkFiles;
  readonly #publishState: PublishState;
  readonly #currentRevision: () => number;
  readonly #currentSnapshot: () => PresentationSnapshot;
  #artworkIdentity: string | null | undefined;
  #request = 0;

  constructor({
    artworkFiles,
    publishState,
    currentRevision,
    currentSnapshot,
  }: ArtworkPresentationCoordinatorOptions) {
    this.#artworkFiles = artworkFiles;
    this.#publishState = publishState;
    this.#currentRevision = currentRevision;
    this.#currentSnapshot = currentSnapshot;
  }

  cancelAndClear(): Promise<void> {
    this.#request += 1;
    this.#artworkIdentity = undefined;
    return this.#artworkFiles.clear();
  }

  present(
    core: RoonCore,
    retainedZone: RetainedZone,
    refreshArtwork: boolean,
  ): void {
    const { zone } = retainedZone;
    const state = availableState(retainedZone);
    const currentSnapshot = this.#currentSnapshot();
    const stateChanged = !samePresentationExceptArtwork(currentSnapshot, state);

    if (zone.state === "stopped") {
      if (stateChanged) {
        this.#publishState(state);
      }
      void this.cancelAndClear().catch(reportArtworkError);
      return;
    }

    const imageKey = zone.now_playing?.image_key;
    const artworkIdentity = imageKey ?? null;
    const artworkIdentityChanged =
      refreshArtwork && this.#artworkIdentity !== artworkIdentity;

    if (!stateChanged && !artworkIdentityChanged) {
      return;
    }

    if (!artworkIdentityChanged) {
      this.#publishState({ ...state, artwork: currentSnapshot.artwork });
      return;
    }

    this.#artworkIdentity = artworkIdentity;
    const request = ++this.#request;
    const retainArtworkWhileLoading =
      zone.state === "loading" && imageKey !== undefined;
    if (stateChanged) {
      this.#publishState({
        ...state,
        artwork: retainArtworkWhileLoading ? currentSnapshot.artwork : null,
      });
    }

    const imageService = core.services.RoonApiImage;
    if (imageKey === undefined || imageService === undefined) {
      this.#publishLatestWithArtwork(null);
      void this.#artworkFiles.clear().catch(reportArtworkError);
      return;
    }

    if (stateChanged && !retainArtworkWhileLoading) {
      void this.#artworkFiles.clear().catch(reportArtworkError);
    }

    imageService.get_image(
      imageKey,
      {
        scale: "fit",
        width: 1600,
        height: 1600,
        format: "image/jpeg",
      },
      (error, contentType, image) => {
        if (request !== this.#request) {
          return;
        }
        if (
          error !== false ||
          contentType !== "image/jpeg" ||
          image === undefined
        ) {
          this.#publishLatestWithArtwork(null);
          void this.#artworkFiles.clear().catch(reportArtworkError);
          return;
        }

        void this.#publishArtwork(request, image).catch(reportArtworkError);
      },
    );
  }

  async #publishArtwork(request: number, image: Buffer): Promise<void> {
    const reference = await this.#artworkFiles.stage(
      this.#currentRevision() + 1,
      image,
    );
    if (request !== this.#request) {
      await this.#artworkFiles.discard(reference);
      return;
    }

    this.#publishLatestWithArtwork(reference);
    await this.#artworkFiles.commit(reference);
  }

  #publishLatestWithArtwork(artwork: PresentationSnapshot["artwork"]): void {
    const latest = this.#currentSnapshot();
    this.#publishState({
      schemaVersion: latest.schemaVersion,
      availability: latest.availability,
      playback: latest.playback,
      displayZone: latest.displayZone,
      nowPlaying: latest.nowPlaying,
      progress: latest.progress,
      artwork,
    });
  }
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
  const retainsNowPlaying = zone.state !== "stopped";
  const displayLines = retainsNowPlaying
    ? zone.now_playing?.three_line
    : undefined;

  return {
    schemaVersion: 1,
    availability: "available",
    playback: zone.state,
    displayZone: { name: zone.display_name },
    nowPlaying:
      displayLines === undefined
        ? null
        : {
            title: displayLines.line1 ?? null,
            artist: displayLines.line2 ?? null,
            album: displayLines.line3 ?? null,
          },
    progress: retainsNowPlaying
      ? meaningfulProgress(zone.now_playing, sampledAt)
      : null,
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

function samePresentationExceptArtwork(
  snapshot: PresentationSnapshot,
  state: SnapshotState,
): boolean {
  return samePresentation(snapshot, {
    ...state,
    artwork: snapshot.artwork,
  });
}

function sameZonePresentationSource(left: RoonZone, right: RoonZone): boolean {
  const comparisonSample = "";
  return (
    JSON.stringify(
      availableState({ zone: left, sampledAt: comparisonSample }),
    ) ===
    JSON.stringify(availableState({ zone: right, sampledAt: comparisonSample }))
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

function reportArtworkError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`RoonScape artwork: ${message}\n`);
}
