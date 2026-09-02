import { randomUUID } from "node:crypto";
import { createSocket, type RemoteInfo } from "node:dgram";
import { lookup } from "node:dns";

import type { RoonServerHost } from "./roon-server-host.js";

const roonDiscoveryPort = 9003;
const roonServiceId = "00720724-5143-4a9b-abac-0e50cba674bb";
const retryMilliseconds = 1_000;

export interface RoonServerEndpoint {
  host: string;
  port: number;
}

export async function discoverRoonServer(
  roonServerHost: RoonServerHost,
  signal: AbortSignal,
): Promise<RoonServerEndpoint> {
  while (true) {
    try {
      const addresses = await resolveIpv4Addresses(roonServerHost, signal);
      return await queryAddresses(addresses, signal);
    } catch (error) {
      if (isAbortError(error)) {
        throw error;
      }
      await waitForRetry(signal);
    }
  }
}

function waitForRetry(signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const finish = (settle: () => void): void => {
      clearTimeout(retry);
      signal.removeEventListener("abort", handleAbort);
      settle();
    };
    const handleAbort = (): void => finish(() => reject(discoveryCancelled()));
    const retry = setTimeout(() => finish(resolve), retryMilliseconds);
    signal.addEventListener("abort", handleAbort, { once: true });
    if (signal.aborted) {
      handleAbort();
    }
  });
}

function resolveIpv4Addresses(
  roonServerHost: RoonServerHost,
  signal: AbortSignal,
): Promise<ReadonlySet<string>> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (settle: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      signal.removeEventListener("abort", handleAbort);
      settle();
    };
    const handleAbort = (): void => finish(() => reject(discoveryCancelled()));

    signal.addEventListener("abort", handleAbort, { once: true });
    if (signal.aborted) {
      handleAbort();
      return;
    }

    lookup(
      roonServerHost,
      { all: true, family: 4 },
      (error, resolvedAddresses) => {
        if (error !== null) {
          finish(() => reject(error));
          return;
        }
        const addresses = new Set(
          resolvedAddresses.map(({ address }) => address),
        );
        if (addresses.size === 0) {
          finish(() =>
            reject(
              new Error(
                `Roon Server Host did not resolve to an IPv4 address: ${roonServerHost}`,
              ),
            ),
          );
          return;
        }
        finish(() => resolve(addresses));
      },
    );
  });
}

function queryAddresses(
  addresses: ReadonlySet<string>,
  signal: AbortSignal,
): Promise<RoonServerEndpoint> {
  return new Promise((resolve, reject) => {
    const socket = createSocket("udp4");
    let activeTransactionId = "";
    let retry: NodeJS.Timeout | undefined;
    let settled = false;

    const cleanup = (): void => {
      if (retry !== undefined) {
        clearInterval(retry);
      }
      signal.removeEventListener("abort", handleAbort);
      socket.removeAllListeners();
      try {
        socket.close();
      } catch (error) {
        if (!isSocketNotRunning(error)) {
          throw error;
        }
      }
    };
    const finish = (settle: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      try {
        cleanup();
      } catch (error) {
        reject(error);
        return;
      }
      settle();
    };
    const handleAbort = (): void => finish(() => reject(discoveryCancelled()));
    const handleError = (error: Error): void => finish(() => reject(error));
    const handleMessage = (packet: Buffer, source: RemoteInfo): void => {
      const properties = parseReply(packet);
      if (
        properties === null ||
        properties._tid !== activeTransactionId ||
        properties.service_id !== roonServiceId ||
        !addresses.has(source.address)
      ) {
        return;
      }
      const advertisedPort = properties.http_port;
      if (typeof advertisedPort !== "string" || !/^\d+$/.test(advertisedPort)) {
        return;
      }
      const port = Number(advertisedPort);
      if (!Number.isInteger(port) || port < 1 || port > 65_535) {
        return;
      }
      finish(() => resolve({ host: source.address, port }));
    };
    const sendQuery = (): void => {
      activeTransactionId = randomUUID();
      const packet = encodeQuery({
        _tid: activeTransactionId,
        query_service_id: roonServiceId,
      });
      for (const address of addresses) {
        socket.send(packet, roonDiscoveryPort, address);
      }
    };

    signal.addEventListener("abort", handleAbort, { once: true });
    if (signal.aborted) {
      handleAbort();
      return;
    }
    socket.on("error", handleError);
    socket.on("message", handleMessage);
    socket.bind(0, () => {
      if (settled) {
        return;
      }
      sendQuery();
      retry = setInterval(sendQuery, retryMilliseconds);
      retry.unref();
    });
  });
}

function encodeQuery(properties: Readonly<Record<string, string>>): Buffer {
  const fields = Object.entries(properties).map(([name, value]) => {
    const nameBytes = Buffer.from(name);
    const valueBytes = Buffer.from(value);
    const field = Buffer.alloc(1 + nameBytes.length + 2 + valueBytes.length);
    field.writeUInt8(nameBytes.length, 0);
    nameBytes.copy(field, 1);
    field.writeUInt16BE(valueBytes.length, 1 + nameBytes.length);
    valueBytes.copy(field, 1 + nameBytes.length + 2);
    return field;
  });
  return Buffer.concat([Buffer.from("SOOD\x02Q"), ...fields]);
}

function parseReply(packet: Buffer): Record<string, string | null> | null {
  if (packet.length < 6 || packet.subarray(0, 6).toString() !== "SOOD\x02R") {
    return null;
  }

  try {
    const properties: Record<string, string | null> = {};
    let offset = 6;
    while (offset < packet.length) {
      const nameLength = packet.readUInt8(offset);
      offset += 1;
      if (nameLength === 0 || offset + nameLength + 2 > packet.length) {
        return null;
      }
      const name = packet.subarray(offset, offset + nameLength).toString();
      offset += nameLength;
      const valueLength = packet.readUInt16BE(offset);
      offset += 2;
      if (valueLength === 65_535) {
        properties[name] = null;
        continue;
      }
      if (offset + valueLength > packet.length) {
        return null;
      }
      properties[name] = packet
        .subarray(offset, offset + valueLength)
        .toString();
      offset += valueLength;
    }
    return properties;
  } catch {
    return null;
  }
}

function discoveryCancelled(): DOMException {
  return new DOMException("Roon discovery cancelled", "AbortError");
}

function isSocketNotRunning(error: unknown): boolean {
  return (
    error instanceof Error &&
    "code" in error &&
    (error as NodeJS.ErrnoException).code === "ERR_SOCKET_DGRAM_NOT_RUNNING"
  );
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError";
}
