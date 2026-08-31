import type { DiscoverableTrackedOutput } from "./display-configuration-command.js";
import {
  type AuthorizationStore,
  type CreateRoonServices,
  type RoonServices,
} from "./roon-bridge.js";
import { initializeRoonExtension } from "./roon-extension.js";

interface TrackedOutputDiscoveryOptions {
  authorizationStore: AuthorizationStore;
  createRoonServices: CreateRoonServices;
  timeoutMilliseconds?: number | null;
  signal?: AbortSignal;
}

// The pinned node-roon-api finishes discovery startup on a 200 ms callback
// that stop_discovery does not cancel.
const roonDiscoveryStartupMilliseconds = 200;

export function discoverTrackedOutputs({
  authorizationStore,
  createRoonServices,
  timeoutMilliseconds = 60_000,
  signal,
}: TrackedOutputDiscoveryOptions): Promise<DiscoverableTrackedOutput[]> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let timeout: NodeJS.Timeout | undefined;
    const settleAfterCleanup = (settlePromise: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      if (timeout !== undefined) {
        clearTimeout(timeout);
      }
      signal?.removeEventListener("abort", handleAbort);
      stopRoonServices(services);
      const deferredStop = setTimeout(
        () => stopRoonServices(services),
        roonDiscoveryStartupMilliseconds + 1,
      );
      deferredStop.unref();
      settlePromise();
    };
    const handleAbort = (): void => {
      settleAfterCleanup(() =>
        reject(new DOMException("Roon discovery cancelled", "AbortError")),
      );
    };
    const services: RoonServices = initializeRoonExtension({
      authorizationStore,
      createRoonServices,
      corePaired: (core) => {
        if (settled) {
          return;
        }
        core.services.RoonApiTransport.subscribe_zones((response, event) => {
          if (settled || response !== "Subscribed") {
            return;
          }

          settleAfterCleanup(() =>
            resolve(
              (event.zones ?? []).flatMap((zone) =>
                zone.outputs.map((output) => ({
                  trackedOutputId: output.output_id,
                  trackedOutputName: output.display_name,
                  trackedZoneName: zone.display_name,
                })),
              ),
            ),
          );
        });
      },
      coreUnpaired: () => undefined,
    });

    services.status.set_status("Discovering Tracked Outputs", false);
    signal?.addEventListener("abort", handleAbort, { once: true });
    if (signal?.aborted) {
      handleAbort();
      return;
    }
    if (timeoutMilliseconds !== null) {
      timeout = setTimeout(() => {
        settleAfterCleanup(() =>
          reject(
            new Error(
              "Timed out waiting for Roon; confirm RoonScape is enabled in Settings → Extensions",
            ),
          ),
        );
      }, timeoutMilliseconds);
    }
    services.extension.start_discovery();
  });
}

function stopRoonServices(services: RoonServices): void {
  services.extension.stop_discovery();
  services.extension.disconnect_all();
}
