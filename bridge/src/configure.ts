import {
  FileAuthorizationStore,
  authorizationFilePath,
} from "./authorization-store.js";
import { runDisplayConfigurationCommand } from "./display-configuration-command.js";
import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
} from "./display-configuration.js";
import { discoverTrackedOutputs } from "./tracked-output-discovery.js";
import { createSupportedRoonServices } from "./roon-services.js";

const authorizationStore = new FileAuthorizationStore(authorizationFilePath());
const configurationStore = new FileDisplayConfigurationStore(
  displayConfigurationFilePath(),
);

try {
  process.exitCode = await runDisplayConfigurationCommand(
    process.argv.slice(2),
    {
      configurationStore,
      discoverTrackedOutputs: () =>
        discoverTrackedOutputs({
          authorizationStore,
          createRoonServices: createSupportedRoonServices,
        }),
      writeLine: (line) => process.stdout.write(`${line}\n`),
    },
  );
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`RoonScape configuration: ${message}\n`);
  process.exitCode = 1;
}
