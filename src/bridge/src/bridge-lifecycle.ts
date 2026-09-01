import { attemptAllCleanup } from "./cleanup.js";
import type { SnapshotPublisher } from "./fixture-publisher.js";
import {
  installProcessLifecycle,
  type ProcessLifecycleEnvironment,
} from "./process-lifecycle.js";
import type { RoonBridge } from "./roon-bridge.js";

interface BridgeLifecycleOptions extends ProcessLifecycleEnvironment {
  bridge: RoonBridge;
  publisher: SnapshotPublisher;
}

export function installBridgeLifecycle({
  bridge,
  publisher,
  once,
  reportError,
  exit,
}: BridgeLifecycleOptions): void {
  installProcessLifecycle({
    cleanup: () => shutdownBridge(bridge, publisher),
    failureMessage: "Could not stop RoonScape Bridge",
    once,
    reportError,
    exit,
  });
}

export async function shutdownBridge(
  bridge: RoonBridge,
  publisher: SnapshotPublisher,
): Promise<void> {
  return attemptAllCleanup("Could not stop RoonScape Bridge", [
    () => bridge.stop(),
    () => publisher.close(),
  ]);
}
