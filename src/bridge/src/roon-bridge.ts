import type {
  DiagnosticCapture,
  DiagnosticSource,
  DiagnosticContributors,
} from "./diagnostic-capture.js";
import type { Availability, PresentationSnapshot } from "./snapshot.js";
import type { ArtworkFiles } from "./artwork-file-store.js";
import { attemptAllCleanup } from "./cleanup.js";
import type {
  DisplayConfiguration,
  DisplayConfigurationStore,
} from "./display-configuration.js";
import {
  SnapshotPublicationError,
  assertSnapshotPublishable,
} from "./fixture-publisher.js";
import {
  startLyricFeed,
  trackedNowPlaying,
  type LyricFeed,
  type LyricFeedConnectionFactory,
  type TrackedNowPlaying,
} from "./lyric-feed.js";
import {
  createPrivateLyricFeedConnection,
  lyricFeedEndpointForCore,
} from "./private-lyric-connection.js";
import {
  connectRoonExtension,
  initializeRoonExtension,
} from "./roon-extension.js";
import type { RoonServerHost } from "./roon-server-host.js";

type Unavailable = Exclude<Availability, "available">;
type SnapshotState = Omit<PresentationSnapshot, "revision">;
type PublishState = (
  state: SnapshotState,
  contributors?: DiagnosticContributors,
  trigger?: string,
) => boolean;
type ScheduleArtworkRetry = (
  retry: () => void,
  delayMilliseconds: number,
) => () => void;
const INITIAL_ARTWORK_RETRY_DELAY_MILLISECONDS = 1_000;
const MAXIMUM_ARTWORK_RETRY_DELAY_MILLISECONDS = 30_000;

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

export interface RoonConnectionOptions {
  host: string;
  port: number;
  onclose?: () => void;
  onerror?: (connection: unknown) => void;
}

export interface RoonExtension {
  init_services(services: {
    required_services: RoonServiceDescriptor[];
    provided_services: RoonServiceDescriptor[];
  }): void;
  start_discovery(): void;
  stop_discovery(): void;
  disconnect_all(): void;
  ws_connect?(options: RoonConnectionOptions): {
    transport: { close(): void };
  };
}

export interface RoonServices {
  extension: RoonExtension;
  requiredServices: RoonServiceDescriptor[];
  status: RoonStatusService;
}

export interface RoonBridge {
  currentSnapshot(): PresentationSnapshot;
  lyricsVisible(revision: number): void;
  stop(): Promise<void>;
}

export type CreateRoonServices = (
  options: RoonExtensionOptions,
  diagnosticCapture?: DiagnosticCapture,
) => RoonServices;

interface StartRoonBridgeOptions {
  diagnosticCapture?: DiagnosticCapture;
  authorizationStore: AuthorizationStore;
  artworkFiles: ArtworkFiles;
  displayConfigurationStore: DisplayConfigurationStore;
  createRoonServices: CreateRoonServices;
  roonServerHost?: RoonServerHost;
  publish(snapshot: PresentationSnapshot): void;
  reportPublicationFailure?: (reason: string) => void;
  scheduleArtworkRetry?: ScheduleArtworkRetry;
  now?: () => Date;
  createLyricFeedConnection?: LyricFeedConnectionFactory;
}

interface RetainedZone {
  zone: RoonZone;
  sampledAt: string;
}

interface TrackedZoneState extends RetainedZone {
  trackedOutput: RoonOutput;
}

export function startRoonBridge({
  diagnosticCapture,
  authorizationStore,
  artworkFiles,
  displayConfigurationStore,
  createRoonServices,
  roonServerHost,
  publish,
  reportPublicationFailure = reportSnapshotPublicationFailure,
  scheduleArtworkRetry = scheduleRetryWithTimeout,
  now = () => new Date(),
  createLyricFeedConnection = (options) =>
    createPrivateLyricFeedConnection(options, undefined, diagnosticCapture),
}: StartRoonBridgeOptions): RoonBridge {
  const lyricsEnabled =
    displayConfigurationStore.load()?.lyricsEnabled === true;
  let revision = 0;
  const initialAvailability: Unavailable = hasAuthorization(
    authorizationStore.load(),
  )
    ? "disconnected"
    : "pairingRequired";
  let currentSnapshot = unavailableSnapshot(revision, initialAvailability);
  let updateStatus: (availability: Availability) => void = () => undefined;
  let updatePublicationFailureStatus: (reason: string) => void = () =>
    undefined;
  let lastPublicationFailureCode: string | undefined;
  let activeCore: RoonCore | undefined;
  let activeLyricFeed: LyricFeed | undefined;
  let activeLyrics: PresentationSnapshot["lyrics"] = null;
  let lyricSource: DiagnosticSource | undefined;
  let lyricRecovery: { reason: string; input?: DiagnosticSource } | undefined;
  let retainedRoonState: () => unknown = () => ({
    unavailable: "not connected",
  });
  let reconcilingTrackedNowPlaying = false;

  const recoverFromPublicationFailure = (
    state: SnapshotState,
    error: unknown,
  ): SnapshotState | undefined => {
    const failure = snapshotPublicationFailure(error);
    if (failure.code === "snapshotTooLarge" && state.lyrics !== null) {
      activeLyrics = null;
      lyricRecovery = {
        reason: "lyrics omitted after snapshot size failure",
        input: lyricSource,
      };
      const recovered = { ...state, lyrics: null };
      diagnosticCapture?.annotate(recovered, {
        ...diagnosticCapture.sources(state),
        lyrics: undefined,
      });
      return recovered;
    }
    if (failure.code !== lastPublicationFailureCode) {
      lastPublicationFailureCode = failure.code;
      reportPublicationFailure(failure.message);
      updatePublicationFailureStatus(failure.message);
    }
    return undefined;
  };
  const prepareStateForPublication = (
    state: SnapshotState,
  ): SnapshotState | undefined => {
    try {
      assertSnapshotPublishable({ revision: revision + 1, ...state });
      return state;
    } catch (error) {
      const recoveredState = recoverFromPublicationFailure(state, error);
      return recoveredState === undefined
        ? undefined
        : prepareStateForPublication(recoveredState);
    }
  };
  const publishState: PublishState = (
    state,
    contributors = diagnosticCapture?.sources(state) ?? {},
    trigger = diagnosticCapture?.input ? "received input" : "local publication",
  ) => {
    diagnosticCapture?.annotate(state, contributors);
    if (
      samePresentation(currentSnapshot, state) &&
      lastPublicationFailureCode === undefined
    ) {
      return false;
    }

    const publishableState = prepareStateForPublication(state);
    if (publishableState === undefined) {
      return false;
    }
    const candidate = { revision: revision + 1, ...publishableState };
    try {
      publish(candidate);
    } catch (error) {
      const recoveredState = recoverFromPublicationFailure(
        publishableState,
        error,
      );
      return recoveredState === undefined
        ? false
        : publishState(
            recoveredState,
            diagnosticCapture?.sources(recoveredState),
            "publication error recovery",
          );
    }

    diagnosticCapture?.annotate(
      candidate,
      diagnosticCapture.sources(publishableState),
    );
    diagnosticCapture?.record("snapshot", {
      snapshot: candidate,
      contributors: diagnosticCapture.sources(candidate),
      trigger:
        lyricRecovery === undefined
          ? trigger
          : `${trigger}; ${lyricRecovery.reason}`,
      localInterpretation: lyricRecovery,
    });
    revision = candidate.revision;
    currentSnapshot = candidate;
    lastPublicationFailureCode = undefined;
    updateStatus(state.availability);
    return true;
  };
  const artworkPresentation = new ArtworkPresentationCoordinator({
    diagnosticCapture,
    artworkFiles,
    publishState,
    scheduleArtworkRetry,
    currentRevision: () => revision,
    currentSnapshot: () => currentSnapshot,
    currentLyrics: () => activeLyrics,
    currentLyricSource: () => lyricSource,
    prepareStateForPublication,
  });

  diagnosticCapture?.setCheckpointProvider(() => ({
    roonState: retainedRoonState(),
    lyricFeed: {
      enabled: lyricsEnabled,
      state: activeLyricFeed?.diagnosticState() ?? {
        unavailable: "Lyric Feed inactive",
      },
      source: lyricSource ?? { unavailable: "no accepted lyric input" },
      localInterpretation: lyricRecovery,
    },
    pendingSnapshot: artworkPresentation.diagnosticPendingState(),
  }));

  const publishLyrics = (
    lyrics: PresentationSnapshot["lyrics"],
    source?: DiagnosticSource,
  ): void => {
    lyricSource = source;
    lyricRecovery = undefined;
    activeLyrics = lyrics;
    if (reconcilingTrackedNowPlaying) {
      return;
    }
    if (artworkPresentation.updatePendingLyrics(lyrics)) {
      return;
    }
    const latest = currentSnapshot;
    const acceptedLyrics =
      latest.availability === "available" && latest.nowPlaying !== null
        ? lyrics
        : null;
    publishState(
      {
        schemaVersion: latest.schemaVersion,
        availability: latest.availability,
        playback: latest.playback,
        trackedOutput: latest.trackedOutput,
        trackedZone: latest.trackedZone,
        nowPlaying: latest.nowPlaying,
        timing: latest.timing,
        artwork: latest.artwork,
        lyrics: acceptedLyrics,
      },
      { ...diagnosticCapture?.sources(latest), lyrics: source },
    );
  };
  const reconcileLyricFeed = (
    nowPlaying: TrackedNowPlaying | null,
    knownNowPlaying: readonly TrackedNowPlaying[],
  ): void => {
    reconcilingTrackedNowPlaying = true;
    try {
      activeLyricFeed?.track(nowPlaying, knownNowPlaying);
    } finally {
      reconcilingTrackedNowPlaying = false;
    }
  };

  const changeAvailability = (
    availability: Unavailable,
    trackedOutput: PresentationSnapshot["trackedOutput"] = null,
  ): void => {
    publishState(unavailableState(availability, trackedOutput), {
      availability: diagnosticCapture?.input,
    });
    void artworkPresentation.cancelAndClear().catch(reportArtworkError);
  };

  const services = initializeRoonExtension({
    authorizationStore,
    createRoonServices: (options) =>
      createRoonServices(options, diagnosticCapture),
    corePaired: (core) => {
      activeLyricFeed?.stop();
      activeLyricFeed = undefined;
      activeCore = core;
      retainedRoonState = () => ({
        coreId: core.core_id,
        unavailable: "awaiting zone subscription",
      });
      const loadedConfiguration = displayConfigurationStore.load();
      if (loadedConfiguration === null) {
        return;
      }
      let configuration: DisplayConfiguration = loadedConfiguration;
      changeAvailability("outputUnavailable", {
        name: configuration.trackedOutputName,
      });
      const selectedEndpoint = lyricFeedEndpointForCore(core);
      if (lyricsEnabled && selectedEndpoint !== null) {
        try {
          activeLyricFeed = startLyricFeed({
            endpoint: selectedEndpoint,
            expectedCoreId: core.core_id,
            connect: createLyricFeedConnection,
            onTimeline: publishLyrics,
            diagnosticCapture,
          });
        } catch {
          activeLyricFeed = undefined;
        }
      }
      const zones = new Map<string, RetainedZone>();
      let baselineObserved = false;
      core.services.RoonApiTransport.subscribe_zones((response, event) => {
        if (activeCore !== core) {
          return;
        }

        const sampledAt = now().toISOString();
        if (response === "Subscribed") {
          baselineObserved = true;
          zones.clear();
          for (const zone of event.zones ?? []) {
            const retained = { zone, sampledAt };
            diagnosticCapture?.annotate(retained, {
              zone: diagnosticCapture.input,
              timing: diagnosticCapture.input,
            });
            zones.set(zone.zone_id, retained);
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
            const updated = {
              zone,
              sampledAt:
                retainedZone !== undefined &&
                sameZonePresentationSource(retainedZone, zone)
                  ? retainedZone.sampledAt
                  : sampledAt,
            };
            diagnosticCapture?.annotate(updated, {
              zone: diagnosticCapture.input,
              timing: diagnosticCapture.input,
            });
            zones.set(zone.zone_id, updated);
          }
          for (const seekChange of event.zones_seek_changed ?? []) {
            const retainedZone = zones.get(seekChange.zone_id);
            if (retainedZone?.zone.now_playing === undefined) {
              continue;
            }

            const updated = {
              sampledAt,
              zone: {
                ...retainedZone.zone,
                now_playing: {
                  ...retainedZone.zone.now_playing,
                  seek_position: seekChange.seek_position,
                },
              },
            };
            diagnosticCapture?.annotate(updated, {
              ...diagnosticCapture.sources(retainedZone),
              timing: diagnosticCapture.input,
            });
            zones.set(seekChange.zone_id, updated);
          }
        } else {
          return;
        }

        retainedRoonState = () => ({
          coreId: core.core_id,
          status: baselineObserved
            ? "observed"
            : "partial: subscription baseline unavailable",
          zones: [...zones.values()].map((retained) => ({
            ...retained,
            contributors: diagnosticCapture?.sources(retained),
          })),
        });
        const trackedZone = [...zones.values()].find(({ zone }) =>
          zone.outputs.some(
            (output) => output.output_id === configuration.trackedOutputId,
          ),
        );
        const trackedOutput = trackedZone?.zone.outputs.find(
          (output) => output.output_id === configuration.trackedOutputId,
        );
        if (trackedZone === undefined || trackedOutput === undefined) {
          reconcileLyricFeed(null, []);
          changeAvailability("outputUnavailable", {
            name: configuration.trackedOutputName,
          });
          return;
        }

        const knownNowPlaying = [...zones.values()]
          .map(({ zone }) => trackedNowPlaying(zone))
          .filter(
            (candidate): candidate is TrackedNowPlaying => candidate !== null,
          );
        reconcileLyricFeed(
          trackedNowPlaying(trackedZone.zone),
          knownNowPlaying,
        );

        if (configuration.trackedOutputName !== trackedOutput.display_name) {
          configuration = {
            ...configuration,
            trackedOutputName: trackedOutput.display_name,
          };
          displayConfigurationStore.save(configuration);
        }

        const selected = { ...trackedZone, trackedOutput };
        diagnosticCapture?.annotate(
          selected,
          diagnosticCapture.sources(trackedZone),
        );
        artworkPresentation.present(core, selected);
      });
    },
    coreUnpaired: (core) => {
      if (activeCore !== core) {
        return;
      }
      activeCore = undefined;
      retainedRoonState = () => ({ unavailable: "disconnected" });
      activeLyricFeed?.stop();
      activeLyricFeed = undefined;
      changeAvailability("disconnected");
    },
  });
  const status = services.status;
  updateStatus = (availability) => setExtensionStatus(status, availability);
  updatePublicationFailureStatus = (reason) =>
    status.set_status(`Publication failed: ${reason}`, true);
  publish(currentSnapshot);
  diagnosticCapture?.record("snapshot", {
    snapshot: currentSnapshot,
    contributors: {},
    trigger: "startup",
  });
  setExtensionStatus(status, currentSnapshot.availability);
  const directedConnection =
    roonServerHost === undefined
      ? undefined
      : connectRoonExtension(
          services.extension,
          roonServerHost,
          diagnosticCapture,
        );
  if (directedConnection === undefined) {
    services.extension.start_discovery();
  }

  return {
    currentSnapshot: () => currentSnapshot,
    lyricsVisible: (presentedRevision) => {
      if (
        presentedRevision === currentSnapshot.revision &&
        currentSnapshot.lyrics !== null
      ) {
        activeLyricFeed?.markVisible();
      }
    },
    stop: () =>
      attemptAllCleanup(
        "Could not stop RoonScape Bridge",
        directedConnection === undefined
          ? [
              () => services.extension.stop_discovery(),
              () => services.extension.disconnect_all(),
              () => activeLyricFeed?.stop(),
              () => artworkPresentation.cancelAndClear(),
            ]
          : [
              () => directedConnection.stop(),
              () => activeLyricFeed?.stop(),
              () => artworkPresentation.cancelAndClear(),
            ],
      ),
  };
}

interface ArtworkPresentationCoordinatorOptions {
  diagnosticCapture?: DiagnosticCapture;
  currentLyricSource(): DiagnosticSource | undefined;
  artworkFiles: ArtworkFiles;
  publishState: PublishState;
  scheduleArtworkRetry: ScheduleArtworkRetry;
  currentRevision(): number;
  currentSnapshot(): PresentationSnapshot;
  currentLyrics(): PresentationSnapshot["lyrics"];
  prepareStateForPublication(state: SnapshotState): SnapshotState | undefined;
}

class ArtworkPresentationCoordinator {
  readonly #diagnosticCapture: DiagnosticCapture | undefined;
  readonly #currentLyricSource: () => DiagnosticSource | undefined;
  readonly #artworkFiles: ArtworkFiles;
  readonly #publishState: PublishState;
  readonly #scheduleArtworkRetry: ScheduleArtworkRetry;
  readonly #currentRevision: () => number;
  readonly #currentSnapshot: () => PresentationSnapshot;
  readonly #currentLyrics: () => PresentationSnapshot["lyrics"];
  readonly #prepareStateForPublication: (
    state: SnapshotState,
  ) => SnapshotState | undefined;
  #artworkIdentity: { zoneId: string; imageKey: string | null } | undefined;
  #cancelScheduledArtworkRetry: (() => void) | undefined;
  #consecutiveFailures = 0;
  #requestGeneration = 0;
  #pendingState: SnapshotState | undefined;

  constructor({
    diagnosticCapture,
    currentLyricSource,
    artworkFiles,
    publishState,
    scheduleArtworkRetry,
    currentRevision,
    currentSnapshot,
    currentLyrics,
    prepareStateForPublication,
  }: ArtworkPresentationCoordinatorOptions) {
    this.#diagnosticCapture = diagnosticCapture;
    this.#currentLyricSource = currentLyricSource;
    this.#artworkFiles = artworkFiles;
    this.#publishState = publishState;
    this.#scheduleArtworkRetry = scheduleArtworkRetry;
    this.#currentRevision = currentRevision;
    this.#currentSnapshot = currentSnapshot;
    this.#currentLyrics = currentLyrics;
    this.#prepareStateForPublication = prepareStateForPublication;
  }

  diagnosticPendingState(): unknown {
    return this.#pendingState === undefined
      ? { status: "none" }
      : {
          snapshot: this.#pendingState,
          contributors: this.#diagnosticCapture?.sources(this.#pendingState),
        };
  }

  cancelAndClear(): Promise<void> {
    this.#resetArtworkRequest();
    this.#artworkIdentity = undefined;
    return this.#artworkFiles.clear();
  }

  present(core: RoonCore, trackedZone: TrackedZoneState): void {
    const { zone } = trackedZone;
    const currentSnapshot = this.#currentSnapshot();
    const available = availableState(trackedZone);
    const state = {
      ...available,
      lyrics: available.nowPlaying === null ? null : this.#currentLyrics(),
    };

    this.#diagnosticCapture?.annotate(state, {
      ...this.#diagnosticCapture.sources(trackedZone),
      lyrics:
        available.nowPlaying === null ? undefined : this.#currentLyricSource(),
    });

    if (zone.state === "stopped") {
      const stateChanged = !samePresentationExceptArtwork(
        currentSnapshot,
        state,
      );
      const published = this.#publishState(state);
      if (stateChanged && !published) {
        return;
      }
      void this.cancelAndClear().catch(reportArtworkError);
      return;
    }

    const publishableState = this.#prepareStateForPublication(state);
    if (publishableState === undefined) {
      return;
    }

    const imageKey = zone.now_playing?.image_key;
    const artworkIdentity = {
      zoneId: zone.zone_id,
      imageKey: imageKey ?? null,
    };
    const artworkIdentityChanged =
      this.#artworkIdentity === undefined ||
      this.#artworkIdentity.zoneId !== artworkIdentity.zoneId ||
      this.#artworkIdentity.imageKey !== artworkIdentity.imageKey;
    const imageService = core.services.RoonApiImage;

    if (!artworkIdentityChanged) {
      if (this.#pendingState !== undefined) {
        this.#pendingState = publishableState;
        return;
      }
      if (
        !sameNowPlaying(
          currentSnapshot.nowPlaying,
          publishableState.nowPlaying,
        ) &&
        imageKey !== undefined &&
        imageService !== undefined
      ) {
        // Re-resolve a retained key so a following image-key report can
        // supersede this pending composition before either becomes visible.
        this.#beginArtworkRequest(publishableState, imageService, imageKey);
        return;
      }
      this.#publishState(
        {
          ...publishableState,
          artwork: currentSnapshot.artwork,
        },
        {
          ...this.#diagnosticCapture?.sources(publishableState),
          artwork: this.#diagnosticCapture?.sources(currentSnapshot).artwork,
        },
      );
      return;
    }

    this.#artworkIdentity = artworkIdentity;
    if (imageKey === undefined || imageService === undefined) {
      this.#resetArtworkRequest();
      this.#publishState(
        { ...publishableState, artwork: null },
        this.#diagnosticCapture?.sources(publishableState),
      );
      void this.#artworkFiles.clear().catch(reportArtworkError);
      return;
    }

    this.#beginArtworkRequest(publishableState, imageService, imageKey);
  }

  updatePendingLyrics(lyrics: PresentationSnapshot["lyrics"]): boolean {
    const pending = this.#pendingState;
    if (pending === undefined) {
      return false;
    }
    const candidate = {
      ...pending,
      lyrics: pending.nowPlaying === null ? null : lyrics,
    };
    this.#diagnosticCapture?.annotate(candidate, {
      ...this.#diagnosticCapture.sources(pending),
      lyrics:
        pending.nowPlaying === null ? undefined : this.#currentLyricSource(),
    });
    const publishableState = this.#prepareStateForPublication(candidate);
    if (publishableState !== undefined) {
      this.#pendingState = publishableState;
    }
    return true;
  }

  #beginArtworkRequest(
    state: SnapshotState,
    imageService: RoonImageService,
    imageKey: string,
  ): void {
    this.#resetArtworkRequest();
    this.#pendingState = state;
    this.#requestArtwork(imageService, imageKey);
  }

  #resetArtworkRequest(): void {
    this.#cancelScheduledRetry();
    this.#requestGeneration += 1;
    this.#consecutiveFailures = 0;
    this.#pendingState = undefined;
  }

  #requestArtwork(imageService: RoonImageService, imageKey: string): void {
    const requestGeneration = ++this.#requestGeneration;
    imageService.get_image(
      imageKey,
      {
        scale: "fit",
        width: 1600,
        height: 1600,
        format: "image/jpeg",
      },
      (error, contentType, image) => {
        if (requestGeneration !== this.#requestGeneration) {
          return;
        }
        if (
          error !== false ||
          contentType !== "image/jpeg" ||
          image === undefined
        ) {
          this.#recoverFromFailure(
            requestGeneration,
            imageService,
            imageKey,
            this.#diagnosticCapture?.input,
            "artwork response failure",
          );
          return;
        }

        const source = this.#diagnosticCapture?.input;
        void this.#publishArtwork(requestGeneration, image, source).catch(
          (error: unknown) => {
            reportArtworkError(error);
            this.#recoverFromFailure(
              requestGeneration,
              imageService,
              imageKey,
              source,
              "artwork storage error",
            );
          },
        );
      },
    );
  }

  async #publishArtwork(
    requestGeneration: number,
    image: Buffer,
    source?: DiagnosticSource,
  ): Promise<void> {
    const reference = await this.#artworkFiles.stage(
      this.#currentRevision() + 1,
      image,
    );
    if (requestGeneration !== this.#requestGeneration) {
      await this.#artworkFiles.discard(reference);
      return;
    }

    this.#publishPendingWithArtwork(reference, source, "artwork completion");
    await this.#artworkFiles.commit(reference);
    this.#consecutiveFailures = 0;
  }

  #recoverFromFailure(
    requestGeneration: number,
    imageService: RoonImageService,
    imageKey: string,
    source?: DiagnosticSource,
    trigger = "artwork failure",
  ): void {
    if (requestGeneration !== this.#requestGeneration) {
      return;
    }

    this.#publishPendingWithArtwork(null, source, trigger);
    void this.#artworkFiles.clear().catch(reportArtworkError);
    this.#consecutiveFailures += 1;
    const delayMilliseconds = Math.min(
      INITIAL_ARTWORK_RETRY_DELAY_MILLISECONDS *
        2 ** (this.#consecutiveFailures - 1),
      MAXIMUM_ARTWORK_RETRY_DELAY_MILLISECONDS,
    );
    this.#cancelScheduledArtworkRetry = this.#scheduleArtworkRetry(() => {
      this.#cancelScheduledArtworkRetry = undefined;
      if (requestGeneration === this.#requestGeneration) {
        this.#requestArtwork(imageService, imageKey);
      }
    }, delayMilliseconds);
  }

  #cancelScheduledRetry(): void {
    this.#cancelScheduledArtworkRetry?.();
    this.#cancelScheduledArtworkRetry = undefined;
  }

  #publishLatestWithArtwork(
    artwork: PresentationSnapshot["artwork"],
    source?: DiagnosticSource,
    trigger?: string,
  ): void {
    const latest = this.#currentSnapshot();
    this.#publishState(
      {
        schemaVersion: latest.schemaVersion,
        availability: latest.availability,
        playback: latest.playback,
        trackedOutput: latest.trackedOutput,
        trackedZone: latest.trackedZone,
        nowPlaying: latest.nowPlaying,
        timing: latest.timing,
        artwork,
        lyrics: latest.lyrics,
      },
      { ...this.#diagnosticCapture?.sources(latest), artwork: source },
      trigger,
    );
  }

  #publishPendingWithArtwork(
    artwork: PresentationSnapshot["artwork"],
    source?: DiagnosticSource,
    trigger?: string,
  ): void {
    const pending = this.#pendingState;
    this.#pendingState = undefined;
    if (pending === undefined) {
      this.#publishLatestWithArtwork(artwork, source, trigger);
      return;
    }
    this.#publishState(
      { ...pending, artwork },
      { ...this.#diagnosticCapture?.sources(pending), artwork: source },
      trigger,
    );
  }
}

function scheduleRetryWithTimeout(
  retry: () => void,
  delayMilliseconds: number,
): () => void {
  const timeout = setTimeout(retry, delayMilliseconds);
  return () => clearTimeout(timeout);
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

function unavailableState(
  availability: Unavailable,
  trackedOutput: PresentationSnapshot["trackedOutput"] = null,
): SnapshotState {
  return {
    schemaVersion: 4,
    availability,
    playback: null,
    trackedOutput,
    trackedZone: null,
    nowPlaying: null,
    timing: null,
    artwork: null,
    lyrics: null,
  };
}

function availableState({
  zone,
  sampledAt,
  trackedOutput,
}: TrackedZoneState): SnapshotState {
  const { playback, trackedZone, nowPlaying, timing } = zonePresentationSource({
    zone,
    sampledAt,
  });

  return {
    schemaVersion: 4,
    availability: "available",
    playback,
    trackedOutput: { name: trackedOutput.display_name },
    trackedZone,
    nowPlaying,
    timing,
    artwork: null,
    lyrics: null,
  };
}

function meaningfulTiming(
  nowPlaying: RoonNowPlaying | undefined,
  sampledAt: string,
): PresentationSnapshot["timing"] {
  const sourcePosition = nowPlaying?.seek_position;
  const sourceDuration = nowPlaying?.length;
  const durationSeconds =
    sourceDuration !== undefined &&
    Number.isFinite(sourceDuration) &&
    sourceDuration > 0
      ? sourceDuration
      : null;
  const position =
    sourcePosition !== undefined &&
    Number.isFinite(sourcePosition) &&
    sourcePosition >= 0
      ? {
          seconds:
            durationSeconds === null
              ? sourcePosition
              : Math.min(sourcePosition, durationSeconds),
          sampledAt,
        }
      : null;
  if (position === null && durationSeconds === null) {
    return null;
  }

  return { position, durationSeconds };
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

function sameNowPlaying(
  left: PresentationSnapshot["nowPlaying"],
  right: PresentationSnapshot["nowPlaying"],
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function sameZonePresentationSource(
  retainedZone: RetainedZone,
  updatedZone: RoonZone,
): boolean {
  return (
    JSON.stringify(zonePresentationSource(retainedZone)) ===
    JSON.stringify(
      zonePresentationSource({
        zone: updatedZone,
        sampledAt: retainedZone.sampledAt,
      }),
    )
  );
}

function zonePresentationSource({
  zone,
  sampledAt,
}: RetainedZone): Pick<
  SnapshotState,
  "playback" | "trackedZone" | "nowPlaying" | "timing"
> {
  const retainsNowPlaying = zone.state !== "stopped";
  const displayLines = retainsNowPlaying
    ? zone.now_playing?.three_line
    : undefined;

  return {
    playback: zone.state,
    trackedZone: { id: zone.zone_id, name: zone.display_name },
    nowPlaying:
      displayLines === undefined
        ? null
        : {
            title: displayLines.line1 ?? null,
            artist: displayLines.line2 ?? null,
            album: displayLines.line3 ?? null,
          },
    timing: retainsNowPlaying
      ? meaningfulTiming(zone.now_playing, sampledAt)
      : null,
  };
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
      message: "Connected: Tracked Output unavailable",
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

function snapshotPublicationFailure(error: unknown): {
  code: string;
  message: string;
} {
  if (error instanceof SnapshotPublicationError) {
    return { code: error.code, message: error.message };
  }
  return { code: "unknown", message: "Snapshot publication failed" };
}

function reportSnapshotPublicationFailure(reason: string): void {
  process.stderr.write(`RoonScape publication: ${reason}\n`);
}
