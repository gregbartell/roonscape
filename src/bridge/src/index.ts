import path from "node:path";

import { ArtworkFileStore } from "./artwork-file-store.js";
import {
  FileAuthorizationStore,
  authorizationFilePath,
} from "./authorization-store.js";
import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
  rejectRemovedDisplayConfigurationOverride,
} from "./display-configuration.js";
import { installBridgeLifecycle } from "./bridge-lifecycle.js";
import { startSnapshotPublisher } from "./fixture-publisher.js";
import { initialAvailabilitySnapshot, startRoonBridge } from "./roon-bridge.js";
import { createSupportedRoonServices } from "./roon-services.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

rejectRemovedDisplayConfigurationOverride(process.env);

const bridgeOptions = bridgeFileOptions(process.argv.slice(2));

const authorizationStore = new FileAuthorizationStore(
  bridgeOptions.authorizationFile ?? authorizationFilePath(),
);
const displayConfigurationStore = new FileDisplayConfigurationStore(
  bridgeOptions.configurationFile ?? displayConfigurationFilePath(),
);
const artworkFiles = await ArtworkFileStore.open(
  path.join(path.dirname(socketPath), "artwork"),
);
const publisher = await startSnapshotPublisher(
  initialAvailabilitySnapshot(authorizationStore),
  socketPath,
);
const bridge = startRoonBridge({
  authorizationStore,
  artworkFiles,
  displayConfigurationStore,
  createRoonServices: createSupportedRoonServices,
  publish: (snapshot) => publisher.publish(snapshot),
});

process.stdout.write(`RoonScape bridge listening at ${socketPath}\n`);

installBridgeLifecycle({ bridge, publisher });

function bridgeFileOptions(arguments_: string[]): {
  authorizationFile?: string;
  configurationFile?: string;
} {
  if (arguments_.length === 0) {
    return {};
  }
  if (
    arguments_.length === 4 &&
    arguments_[0] === "--config" &&
    arguments_[1] &&
    arguments_[2] === "--authorization" &&
    arguments_[3]
  ) {
    return {
      authorizationFile: path.resolve(arguments_[3]),
      configurationFile: path.resolve(arguments_[1]),
    };
  }
  throw new Error(
    "RoonScape bridge accepts only launcher-provided --config and --authorization paths",
  );
}
