import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { authorizationFilePath } from "./authorization-store.js";
import { launchChildProcess } from "./child-process.js";
import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
} from "./display-configuration.js";
import { runRoonScapeCommand } from "./roonscape-command.js";
import { openRuntimeSession } from "./runtime-session.js";

const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));
const bridgeEntry = fileURLToPath(new URL("./index.js", import.meta.url));
const rendererExecutable = path.join(
  repositoryRoot,
  "target/debug/roonscape-renderer",
);
const packageMetadata = JSON.parse(
  readFileSync(path.join(repositoryRoot, "package.json"), "utf8"),
) as { version?: unknown };
if (typeof packageMetadata.version !== "string") {
  throw new Error("RoonScape package version is unavailable");
}
const getUserId = process.getuid;
if (getUserId === undefined) {
  throw new Error("RoonScape requires a Linux user identity");
}

const childEnvironment = (socketPath: string): NodeJS.ProcessEnv => ({
  ...process.env,
  ROONSCAPE_SOCKET: socketPath,
});

process.exitCode = await runRoonScapeCommand(process.argv.slice(2), {
  version: packageMetadata.version,
  environment: process.env,
  currentDirectory: process.cwd(),
  standardConfigurationFile: () => displayConfigurationFilePath(process.env),
  authorizationFile: () => authorizationFilePath(process.env),
  loadConfiguration: (configurationFile) =>
    new FileDisplayConfigurationStore(configurationFile).load(),
  openRuntime: async () =>
    openRuntimeSession({
      environment: process.env,
      processId: process.pid,
      userId: getUserId(),
    }),
  launchBridge: ({ authorizationFile, configurationFile, socketPath }) =>
    launchChildProcess(
      process.execPath,
      [
        bridgeEntry,
        "--config",
        configurationFile,
        "--authorization",
        authorizationFile,
      ],
      childEnvironment(socketPath),
    ),
  launchRenderer: ({ configurationFile, socketPath }) =>
    launchChildProcess(
      rendererExecutable,
      ["--config", configurationFile],
      childEnvironment(socketPath),
    ),
  subscribeToTermination: (handler) => {
    const handleInterrupt = (): void => handler("SIGINT");
    const handleTermination = (): void => handler("SIGTERM");
    process.on("SIGINT", handleInterrupt);
    process.on("SIGTERM", handleTermination);
    return () => {
      process.off("SIGINT", handleInterrupt);
      process.off("SIGTERM", handleTermination);
    };
  },
  delay: (milliseconds) =>
    new Promise((resolve) => {
      const timer = setTimeout(resolve, milliseconds);
      timer.unref();
    }),
  writeOutput: (line) => process.stdout.write(`${line}\n`),
  writeError: (line) => process.stderr.write(`RoonScape: ${line}\n`),
});
