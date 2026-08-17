import type {
  AuthorizationStore,
  CreateRoonServices,
  RoonCore,
  RoonServices,
} from "./roon-bridge.js";

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
