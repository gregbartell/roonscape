import type { Availability, PresentationSnapshot } from "./snapshot.js";

type Unavailable = Exclude<Availability, "available">;

export interface AuthorizationStore {
  load(): unknown;
  save(state: unknown): void;
}

export interface RoonCore {
  core_id: string;
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

export interface RoonAvailabilityBridge {
  currentSnapshot(): PresentationSnapshot;
  stop(): void;
}

export type CreateRoonServices = (
  options: RoonExtensionOptions,
) => RoonServices;

interface StartRoonAvailabilityBridgeOptions {
  authorizationStore: AuthorizationStore;
  createRoonServices: CreateRoonServices;
  publish(snapshot: PresentationSnapshot): void;
}

const extensionIdentity = {
  extension_id: "io.roonscape.bridge",
  display_name: "RoonScape",
  display_version: "0.1.0",
  publisher: "Gregory Bartell",
  email: "gregorybartell@gmail.com",
  website: "https://github.com/gregbartell/roonscape",
  log_level: "none",
} as const;

export function startRoonAvailabilityBridge({
  authorizationStore,
  createRoonServices,
  publish,
}: StartRoonAvailabilityBridgeOptions): RoonAvailabilityBridge {
  let revision = 0;
  let currentAvailability: Unavailable = hasAuthorization(
    authorizationStore.load(),
  )
    ? "disconnected"
    : "pairingRequired";
  let currentSnapshot = unavailableSnapshot(revision, currentAvailability);
  let updateStatus: (availability: Availability) => void = () => undefined;

  const changeAvailability = (availability: Unavailable): void => {
    if (currentAvailability === availability) {
      return;
    }

    currentAvailability = availability;
    revision += 1;
    currentSnapshot = unavailableSnapshot(revision, availability);
    publish(currentSnapshot);
    updateStatus(availability);
  };

  const services = createRoonServices({
    ...extensionIdentity,
    core_paired: () => changeAvailability("outputUnavailable"),
    core_unpaired: () => changeAvailability("disconnected"),
    get_persisted_state: () => authorizationStore.load(),
    set_persisted_state: (state) => authorizationStore.save(state),
  });
  const status = services.status;
  updateStatus = (availability) => setExtensionStatus(status, availability);
  services.extension.init_services({
    required_services: services.requiredServices,
    provided_services: [status],
  });
  publish(currentSnapshot);
  setExtensionStatus(status, currentAvailability);
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
    schemaVersion: 1,
    revision,
    availability,
    playback: null,
    displayZone: null,
    nowPlaying: null,
    progress: null,
    artwork: null,
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
      message: "Connected: Display Output is not configured",
      isError: false,
    },
    available: { message: "Connected", isError: false },
  };
  const nextStatus = extensionStatus[availability];
  status.set_status(nextStatus.message, nextStatus.isError);
}
