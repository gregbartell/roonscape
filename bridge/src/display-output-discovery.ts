import type { DiscoverableDisplayOutput } from "./display-configuration-command.js";
import {
  type AuthorizationStore,
  type CreateRoonServices,
} from "./roon-bridge.js";
import { initializeRoonExtension } from "./roon-extension.js";

interface DisplayOutputDiscoveryOptions {
  authorizationStore: AuthorizationStore;
  createRoonServices: CreateRoonServices;
  timeoutMilliseconds?: number;
}

export function discoverDisplayOutputs({
  authorizationStore,
  createRoonServices,
  timeoutMilliseconds = 60_000,
}: DisplayOutputDiscoveryOptions): Promise<DiscoverableDisplayOutput[]> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const services = initializeRoonExtension({
      authorizationStore,
      createRoonServices,
      corePaired: (core) => {
        core.services.RoonApiTransport.subscribe_zones((response, event) => {
          if (settled || response !== "Subscribed") {
            return;
          }

          settled = true;
          clearTimeout(timeout);
          services.extension.stop_discovery();
          services.extension.disconnect_all();
          resolve(
            (event.zones ?? []).flatMap((zone) =>
              zone.outputs.map((output) => ({
                outputId: output.output_id,
                displayName: output.display_name,
                displayZoneName: zone.display_name,
              })),
            ),
          );
        });
      },
      coreUnpaired: () => undefined,
    });

    services.status.set_status("Discovering Display Outputs", false);
    const timeout = setTimeout(() => {
      settled = true;
      services.extension.stop_discovery();
      services.extension.disconnect_all();
      reject(
        new Error(
          "Timed out waiting for Roon; confirm RoonScape is enabled in Settings → Extensions",
        ),
      );
    }, timeoutMilliseconds);
    services.extension.start_discovery();
  });
}
