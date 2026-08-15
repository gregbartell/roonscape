import RoonApi from "node-roon-api";
import RoonApiImage from "node-roon-api-image";
import RoonApiStatus from "node-roon-api-status";

import type {
  RoonExtensionOptions,
  RoonServices,
} from "./roon-availability.js";

export function createSupportedRoonServices(
  options: RoonExtensionOptions,
): RoonServices {
  const extension = new RoonApi(options);
  const status = new RoonApiStatus(extension);
  return { extension, requiredServices: [RoonApiImage], status };
}
