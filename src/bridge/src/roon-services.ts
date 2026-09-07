import RoonApi from "node-roon-api";
import RoonApiImage from "node-roon-api-image";
import RoonApiStatus from "node-roon-api-status";
import RoonApiTransport from "node-roon-api-transport";

import type { DiagnosticCapture } from "./diagnostic-capture.js";
import { observeRoonSdk } from "./roon-diagnostic-observer.js";

import type { RoonExtensionOptions, RoonServices } from "./roon-bridge.js";

export function createSupportedRoonServices(
  options: RoonExtensionOptions,
  diagnosticCapture?: DiagnosticCapture,
): RoonServices {
  const extension = new RoonApi(options);
  observeRoonSdk(extension, diagnosticCapture, "ordinary");
  const status = new RoonApiStatus(extension);
  return {
    extension,
    requiredServices: [RoonApiImage, RoonApiTransport],
    status,
  };
}
