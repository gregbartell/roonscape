import { randomUUID } from "node:crypto";
import type { EventEmitter } from "node:events";

import { redactDiagnosticData } from "./diagnostic-redaction.js";
import type { DiagnosticCapture } from "./diagnostic-capture.js";
import type { RoonConnectionOptions } from "./roon-bridge.js";

interface DecodedMessage {
  verb: string;
  name: string;
  request_id: string;
  content_type?: string;
  content_length?: number;
  body?: unknown;
  [key: string]: unknown;
}
interface SdkConnection {
  reqid: number;
  core?: { core_id: string };
  send_request(...arguments_: unknown[]): void;
  transport: {
    onmessage(message: DecodedMessage): void;
    onopen(): void;
    onclose(): void;
    onerror(...arguments_: unknown[]): void;
  };
}
interface ObservedSdk {
  ws_connect(options: RoonConnectionOptions): unknown;
  start_discovery?(): void;
  stop_discovery?(): void;
  _sood?: EventEmitter;
}

/**
 * Pinned SDK adapter: ws_connect returns before WebSocket open, and transport
 * onmessage receives exactly the successful Moo.parse result, before the SDK
 * deletes its body and handles registration, requests, or response callbacks.
 * No prototype/global patches and no second decoder or connection are needed.
 */
export function observeRoonSdk(
  extension: ObservedSdk,
  capture: DiagnosticCapture | undefined,
  role: "ordinary" | "lyricFeed",
): void {
  if (!capture) return;
  const connect = extension.ws_connect;
  extension.ws_connect = function (options) {
    const connection = connect.call(this, options) as SdkConnection;
    const connectionId = randomUUID();
    let coreId: string | undefined;
    const requests = new Map<string, unknown>();
    const registryRequests = new Set<string>();
    let incompleteArtworkCorrelation = false;
    const context = () => ({
      connectionId,
      role,
      endpoint: { host: options.host, port: options.port },
      coreId,
    });
    capture.record("connection", { ...context(), state: "connecting" });
    const send = connection.send_request;
    connection.send_request = function (...arguments_) {
      if (
        arguments_[0] === "com.roonlabs.registry:1/info" ||
        arguments_[0] === "com.roonlabs.registry:1/register"
      ) {
        registryRequests.add(String(this.reqid));
      }
      // Correlate only existing image requests; retain no private registry token.
      if (arguments_[0] === "com.roonlabs.image:1/get_image") {
        try {
          if (requests.size >= 128) throw new Error();
          requests.set(
            String(this.reqid),
            redactDiagnosticData(arguments_[1], 8192),
          );
        } catch {
          incompleteArtworkCorrelation = true;
          capture.record("gap", {
            reason: "artwork request correlation limit",
            connectionId,
          });
        }
      }
      return send.apply(this, arguments_);
    };
    const receive = connection.transport.onmessage;
    connection.transport.onmessage = function (message) {
      const body = message.body;
      if (
        message.verb !== "REQUEST" &&
        registryRequests.has(message.request_id) &&
        body &&
        typeof body === "object" &&
        "core_id" in body &&
        typeof body.core_id === "string"
      )
        coreId = body.core_id;
      const request =
        message.verb === "REQUEST"
          ? undefined
          : requests.get(message.request_id);
      const isArtwork =
        request !== undefined ||
        message.content_type?.startsWith("image/") === true ||
        (incompleteArtworkCorrelation && Buffer.isBuffer(body));
      capture.record("inbound", {
        ...context(),
        message:
          isArtwork && Buffer.isBuffer(body)
            ? { ...message, body: { artworkOmitted: true } }
            : message,
        ...(isArtwork
          ? {
              artwork: {
                request: request ?? null,
                coreId: coreId ?? connection.core?.core_id ?? null,
                status: message.name,
                contentType: message.content_type ?? null,
                byteCount: Buffer.isBuffer(body)
                  ? body.length
                  : (message.content_length ?? 0),
              },
            }
          : {}),
      });
      if (message.verb === "COMPLETE") {
        requests.delete(message.request_id);
        registryRequests.delete(message.request_id);
      }
      return receive.call(this, message);
    };
    for (const [callback, state] of [
      ["onopen", "open"],
      ["onclose", "closed"],
      ["onerror", "error"],
    ] as const) {
      const handle = connection.transport[callback];
      connection.transport[callback] = function (...arguments_: unknown[]) {
        capture.record("connection", { ...context(), state });
        if (state === "closed") {
          requests.clear();
          registryRequests.clear();
        }
        return handle.apply(this, arguments_);
      };
    }
    return connection;
  };
  const start = extension.start_discovery;
  if (start) {
    let observed: EventEmitter | undefined;
    extension.start_discovery = function () {
      capture.record("discovery", { role, state: "started" });
      const result = start.call(this);
      if (this._sood && this._sood !== observed) {
        observed = this._sood;
        observed.prependListener("message", (message: unknown) =>
          capture.record("discovery", { role, message }),
        );
      }
      return result;
    };
  }
  const stop = extension.stop_discovery;
  if (stop)
    extension.stop_discovery = function () {
      capture.record("discovery", { role, state: "stopped" });
      return stop.call(this);
    };
}
