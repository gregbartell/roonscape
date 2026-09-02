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
import {
  parseRoonServerHost,
  type RoonServerHost,
} from "./roon-server-host.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

rejectRemovedDisplayConfigurationOverride(process.env);

const bridgeOptions = parseBridgeOptions(process.argv.slice(2));

const authorizationStore = new FileAuthorizationStore(
  bridgeOptions.authorizationFile ?? authorizationFilePath(),
);
const displayConfigurationStore = new FileDisplayConfigurationStore(
  bridgeOptions.configurationFile ?? displayConfigurationFilePath(),
);
const artworkFiles = await ArtworkFileStore.open(
  path.join(path.dirname(socketPath), "artwork"),
);
const bridgeOwner: { current?: ReturnType<typeof startRoonBridge> } = {};
const publisher = await startSnapshotPublisher(
  initialAvailabilitySnapshot(authorizationStore),
  socketPath,
  {
    onLyricsVisible: (revision) => bridgeOwner.current?.lyricsVisible(revision),
  },
);
const bridge = startRoonBridge({
  authorizationStore,
  artworkFiles,
  displayConfigurationStore,
  createRoonServices: createSupportedRoonServices,
  roonServerHost: bridgeOptions.roonServerHost,
  publish: (snapshot) => publisher.publish(snapshot),
});
bridgeOwner.current = bridge;

process.stdout.write(`RoonScape Bridge listening at ${socketPath}\n`);

installBridgeLifecycle({ bridge, publisher });

function parseBridgeOptions(arguments_: string[]): {
  authorizationFile?: string;
  configurationFile?: string;
  roonServerHost?: RoonServerHost;
} {
  if (arguments_.length === 0) {
    return {};
  }
  if (arguments_.length !== 4 && arguments_.length !== 6) {
    throw new Error(
      "RoonScape Bridge accepts only launcher-provided --config, --authorization, and --roon-server options",
    );
  }
  if (
    arguments_[0] !== "--config" ||
    !arguments_[1] ||
    arguments_[2] !== "--authorization" ||
    !arguments_[3]
  ) {
    throw new Error("Invalid launcher-provided bridge file options");
  }

  let roonServerHost: RoonServerHost | undefined;
  if (arguments_.length === 6) {
    roonServerHost =
      arguments_[4] === "--roon-server"
        ? (parseRoonServerHost(arguments_[5] ?? "") ?? undefined)
        : undefined;
    if (roonServerHost === undefined) {
      throw new Error("Invalid launcher-provided Roon Server Host");
    }
  }

  return {
    authorizationFile: path.resolve(arguments_[3]),
    configurationFile: path.resolve(arguments_[1]),
    ...(roonServerHost === undefined ? {} : { roonServerHost }),
  };
}
