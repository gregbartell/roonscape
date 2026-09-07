import path from "node:path";

import {
  DiagnosticCapture,
  defaultCaptureBudgetBytes,
  type DiagnosticCaptureOptions,
} from "./diagnostic-capture.js";
import { parseLaunchOptions } from "./roonscape-command.js";
import { ArtworkFileStore } from "./artwork-file-store.js";
import {
  FileAuthorizationStore,
  authorizationFilePath,
} from "./authorization-store.js";
import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
} from "./display-configuration.js";
import { attemptAllCleanup } from "./cleanup.js";
import { installProcessLifecycle } from "./process-lifecycle.js";
import { startSnapshotPublisher } from "./fixture-publisher.js";
import { initialAvailabilitySnapshot, startRoonBridge } from "./roon-bridge.js";
import { createSupportedRoonServices } from "./roon-services.js";
import type { RoonServerHost } from "./roon-server-host.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

const bridgeOptions = parseBridgeOptions(process.argv.slice(2));
const diagnosticCapture =
  bridgeOptions.diagnosticCapture === undefined
    ? undefined
    : new DiagnosticCapture(bridgeOptions.diagnosticCapture);

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
  diagnosticCapture,
  authorizationStore,
  artworkFiles,
  displayConfigurationStore,
  createRoonServices: createSupportedRoonServices,
  roonServerHost: bridgeOptions.roonServerHost,
  publish: (snapshot) => publisher.publish(snapshot),
});
bridgeOwner.current = bridge;

process.stdout.write(`RoonScape Bridge listening at ${socketPath}\n`);

installProcessLifecycle({
  cleanup: async () => {
    try {
      await attemptAllCleanup("Could not stop RoonScape Bridge", [
        () => bridge.stop(),
        () => publisher.close(),
      ]);
    } finally {
      await diagnosticCapture?.close();
    }
  },
  failureMessage: "Could not stop RoonScape Bridge",
});

function parseBridgeOptions(arguments_: string[]): {
  authorizationFile?: string;
  configurationFile?: string;
  roonServerHost?: RoonServerHost;
  diagnosticCapture?: DiagnosticCaptureOptions;
} {
  if (arguments_.length === 0) return {};
  if (
    arguments_[0] !== "--config" ||
    !arguments_[1] ||
    arguments_[2] !== "--authorization" ||
    !arguments_[3]
  ) {
    throw new Error("Invalid launcher-provided bridge file options");
  }
  const options = parseLaunchOptions([
    arguments_[0],
    arguments_[1],
    ...arguments_.slice(4),
  ]);
  if (options === null || options.setupRequested)
    throw new Error("Invalid launcher-provided bridge options");
  return {
    authorizationFile: path.resolve(arguments_[3]),
    configurationFile: path.resolve(arguments_[1]),
    roonServerHost: options.roonServerHost,
    diagnosticCapture:
      options.captureDirectory === undefined
        ? undefined
        : {
            directory: path.resolve(options.captureDirectory),
            budgetBytes:
              options.captureBudgetBytes ?? defaultCaptureBudgetBytes,
          },
  };
}
