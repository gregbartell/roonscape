import {
  FileAuthorizationStore,
  authorizationFilePath,
} from "./authorization-store.js";
import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
} from "./display-configuration.js";
import { startSnapshotPublisher } from "./fixture-publisher.js";
import { initialAvailabilitySnapshot, startRoonBridge } from "./roon-bridge.js";
import { createSupportedRoonServices } from "./roon-services.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

const authorizationStore = new FileAuthorizationStore(authorizationFilePath());
const displayConfigurationStore = new FileDisplayConfigurationStore(
  displayConfigurationFilePath(),
);
const publisher = await startSnapshotPublisher(
  initialAvailabilitySnapshot(authorizationStore),
  socketPath,
);
const bridge = startRoonBridge({
  authorizationStore,
  displayConfigurationStore,
  createRoonServices: createSupportedRoonServices,
  publish: (snapshot) => publisher.publish(snapshot),
});

process.stdout.write(`RoonScape bridge listening at ${socketPath}\n`);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.once(signal, () => {
    bridge.stop();
    void publisher.close().finally(() => process.exit(0));
  });
}
