import type {
  AuthorizationStore,
  CreateRoonServices,
  RoonCore,
  RoonServices,
} from "./roon-bridge.js";
import { discoverRoonServer } from "./roon-server-discovery.js";
import type { RoonServerHost } from "./roon-server-host.js";

export const roonScapeExtensionIdentity = {
  extension_id: "io.roonscape.bridge",
  display_name: "RoonScape",
  display_version: "1.0.0",
  publisher: "Gregory Bartell",
  email: "5353310+gregbartell@users.noreply.github.com",
  website: "https://github.com/gregbartell/roonscape",
  log_level: "none",
} as const;

interface InitializeRoonExtensionOptions {
  authorizationStore: AuthorizationStore;
  createRoonServices: CreateRoonServices;
  corePaired(core: RoonCore): void;
  coreUnpaired(core: RoonCore): void;
}

export function initializeRoonExtension({
  authorizationStore,
  createRoonServices,
  corePaired,
  coreUnpaired,
}: InitializeRoonExtensionOptions): RoonServices {
  const services = createRoonServices({
    ...roonScapeExtensionIdentity,
    core_paired: corePaired,
    core_unpaired: coreUnpaired,
    get_persisted_state: () => authorizationStore.load(),
    set_persisted_state: (state) => authorizationStore.save(state),
  });
  services.extension.init_services({
    required_services: services.requiredServices,
    provided_services: [services.status],
  });
  return services;
}

export function connectRoonExtension(
  extension: RoonServices["extension"],
  roonServerHost?: RoonServerHost,
): { stop(): void } {
  if (roonServerHost === undefined) {
    extension.start_discovery();
    return onceStopped(() => {
      extension.stop_discovery();
      extension.disconnect_all();
    });
  }

  if (extension.ws_connect === undefined) {
    throw new Error("This Roon API does not support direct connections");
  }
  const connect = extension.ws_connect.bind(extension);
  const cancellation = new AbortController();
  let connection: ReturnType<typeof connect> | undefined;
  void discoverRoonServer(roonServerHost, cancellation.signal)
    .then((endpoint) => {
      if (cancellation.signal.aborted) {
        return;
      }
      connection = connect({
        ...endpoint,
        onclose: () => undefined,
      });
    })
    .catch(() => undefined);
  return onceStopped(() => {
    cancellation.abort();
    connection?.transport.close();
  });
}

function onceStopped(stop: () => void): { stop(): void } {
  let stopped = false;
  return {
    stop: () => {
      if (stopped) {
        return;
      }
      stopped = true;
      stop();
    },
  };
}
