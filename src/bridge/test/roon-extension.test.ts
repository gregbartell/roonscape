import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { createSocket, type RemoteInfo, type Socket } from "node:dgram";
import { once } from "node:events";
import test from "node:test";

import {
  startRoonBridge,
  type RoonCore,
  type RoonExtension,
  type RoonExtensionOptions,
} from "../src/roon-bridge.js";
import { discoverRoonServer } from "../src/roon-server-discovery.js";
import { connectRoonExtension } from "../src/roon-extension.js";
import {
  parseRoonServerHost,
  type RoonServerHost,
} from "../src/roon-server-host.js";
import { discoverTrackedOutputs } from "../src/tracked-output-discovery.js";

const roonServiceId = "00720724-5143-4a9b-abac-0e50cba674bb";
const loopbackRoonServerHost = requiredRoonServerHost("127.0.0.1");
const unavailableRoonServerHost = requiredRoonServerHost(
  "temporarily-unavailable.invalid",
);

test("a directed SOOD reply connects through the Roon JavaScript interface", async () => {
  await withLoopbackResponder(async (responder) => {
    let connectedEndpoint: { host: string; port: number } | undefined;
    let closedConnections = 0;
    const connected = Promise.withResolvers<void>();
    responder.on("message", (query, requester) => {
      const properties = parseSoodProperties(query, "Q");
      assert.equal(properties.query_service_id, roonServiceId);
      respond(
        responder,
        requester,
        soodPacket("R", {
          _tid: properties._tid ?? "missing",
          service_id: roonServiceId,
          http_port: "9330",
          _replyaddr: "192.0.2.10",
          optional_property: null,
        }),
      );
    });

    const connection = connectRoonExtension(
      extensionBoundary(({ host, port }) => {
        connectedEndpoint = { host, port };
        connected.resolve();
        return {
          transport: {
            close: () => {
              closedConnections += 1;
            },
          },
        };
      }),
      loopbackRoonServerHost,
    );

    await connected.promise;
    connection.stop();
    connection.stop();

    assert.deepEqual(connectedEndpoint, { host: "127.0.0.1", port: 9330 });
    assert.equal(closedConnections, 1);
  });
});

test("directed discovery supports Roon Authorization and Tracked Output setup", async () => {
  await withLoopbackResponder(async (responder) => {
    let extensionOptions:
      | Parameters<
          Parameters<typeof discoverTrackedOutputs>[0]["createRoonServices"]
        >[0]
      | undefined;
    let closedConnections = 0;
    responder.on("message", (query, requester) => {
      const transactionId = parseSoodProperties(query, "Q")._tid;
      assert.ok(transactionId);
      respond(responder, requester, validReply(transactionId, "9334"));
    });

    const outputs = await discoverTrackedOutputs({
      authorizationStore: { load: () => ({}), save: () => undefined },
      createRoonServices: (options) => {
        extensionOptions = options;
        return {
          extension: extensionBoundary(() => {
            queueMicrotask(() => {
              assert.ok(extensionOptions);
              extensionOptions.core_paired({
                core_id: "core-setup",
                services: {
                  RoonApiTransport: {
                    subscribe_zones: (listener) =>
                      listener("Subscribed", {
                        zones: [
                          {
                            zone_id: "zone-studio",
                            display_name: "Studio",
                            state: "paused",
                            outputs: [
                              {
                                output_id: "output-studio",
                                display_name: "Studio DAC",
                              },
                            ],
                          },
                        ],
                      }),
                  },
                },
              });
            });
            return {
              transport: {
                close: () => {
                  closedConnections += 1;
                },
              },
            };
          }),
          requiredServices: [
            { services: [{ name: "com.roonlabs.transport:2" }] },
          ],
          status: { services: [], set_status: () => undefined },
        };
      },
      roonServerHost: loopbackRoonServerHost,
      timeoutMilliseconds: 1_000,
    });

    assert.deepEqual(outputs, [
      {
        trackedOutputId: "output-studio",
        trackedOutputName: "Studio DAC",
        trackedZoneName: "Studio",
      },
    ]);
    assert.equal(closedConnections, 1);
  });
});

test("ordinary discovery retains its existing idempotent cleanup", () => {
  const events: string[] = [];
  const connection = connectRoonExtension({
    init_services: () => undefined,
    start_discovery: () => events.push("discovery started"),
    stop_discovery: () => events.push("discovery stopped"),
    disconnect_all: () => events.push("connections closed"),
  });

  connection.stop();
  connection.stop();

  assert.deepEqual(events, [
    "discovery started",
    "discovery stopped",
    "connections closed",
  ]);
});

test("directed discovery retries with a fresh transaction identifier", async (context) => {
  context.mock.timers.enable({ apis: ["setInterval"] });
  await withLoopbackResponder(async (responder) => {
    const transactionIds: string[] = [];
    const firstQuery = Promise.withResolvers<void>();
    const connected = Promise.withResolvers<void>();
    responder.on("message", (query, requester) => {
      const transactionId = parseSoodProperties(query, "Q")._tid;
      assert.ok(transactionId);
      transactionIds.push(transactionId);
      firstQuery.resolve();
      if (transactionIds.length === 2) {
        respond(responder, requester, validReply(transactionId, "9331"));
      }
    });
    const connection = connectRoonExtension(
      extensionBoundary(() => {
        connected.resolve();
        return { transport: { close: () => undefined } };
      }),
      loopbackRoonServerHost,
    );

    try {
      await firstQuery.promise;
      context.mock.timers.tick(1_100);
      await connected.promise;
      assert.equal(transactionIds.length, 2);
      assert.notEqual(transactionIds[0], transactionIds[1]);
    } finally {
      connection.stop();
    }
  });
});

test("directed discovery is cancellable before opening a socket", async () => {
  const controller = new AbortController();
  controller.abort();

  await assert.rejects(
    discoverRoonServer(loopbackRoonServerHost, controller.signal),
    { name: "AbortError" },
  );
});

test("a temporarily unresolvable Roon Server Host keeps waiting for cancellation", async () => {
  const controller = new AbortController();
  const discovery = discoverRoonServer(
    unavailableRoonServerHost,
    controller.signal,
  );
  setTimeout(() => controller.abort(), 50);

  await assert.rejects(discovery, { name: "AbortError" });
});

test("directed discovery cancellation closes an open socket idempotently", async (context) => {
  context.mock.timers.enable({ apis: ["setInterval"] });
  await withLoopbackResponder(async (responder) => {
    const receivedQuery = Promise.withResolvers<void>();
    let queries = 0;
    let connections = 0;
    responder.on("message", () => {
      queries += 1;
      receivedQuery.resolve();
    });
    const connection = connectRoonExtension(
      extensionBoundary(() => {
        connections += 1;
        return { transport: { close: () => undefined } };
      }),
      loopbackRoonServerHost,
    );

    await receivedQuery.promise;
    connection.stop();
    connection.stop();
    context.mock.timers.tick(1_100);

    assert.equal(connections, 0);
    assert.equal(queries, 1);
  });
});

test("unrelated and malformed SOOD replies cannot select the endpoint", async () => {
  await withLoopbackResponder(async (responder) => {
    const unexpectedSource = createSocket("udp4");
    await bind(unexpectedSource, 0, "127.0.0.2");
    let connections = 0;
    const connected = Promise.withResolvers<void>();
    responder.on("message", (query, requester) => {
      const transactionId = parseSoodProperties(query, "Q")._tid;
      assert.ok(transactionId);
      const invalidReplies = [
        Buffer.from("not SOOD"),
        validReply(randomUUID(), "9330"),
        soodPacket("R", {
          _tid: transactionId,
          service_id: "wrong-service",
          http_port: "9330",
        }),
        soodPacket("R", {
          _tid: transactionId,
          service_id: roonServiceId,
        }),
        validReply(transactionId, "nine-thirty"),
        validReply(transactionId, "0"),
        validReply(transactionId, "65536"),
      ];
      for (const reply of invalidReplies) {
        respond(responder, requester, reply);
      }
      respond(unexpectedSource, requester, validReply(transactionId, "9440"));
      setTimeout(
        () => respond(responder, requester, validReply(transactionId, "9332")),
        25,
      );
    });
    const connection = connectRoonExtension(
      extensionBoundary(() => {
        connections += 1;
        connected.resolve();
        return { transport: { close: () => undefined } };
      }),
      loopbackRoonServerHost,
    );

    try {
      await connected.promise;
      assert.equal(connections, 1);
    } finally {
      connection.stop();
      unexpectedSource.close();
      await once(unexpectedSource, "close");
    }
  });
});

test("a lost directed Live Mode connection stays Disconnected without rediscovery", async (context) => {
  context.mock.timers.enable({ apis: ["setInterval"] });
  await withLoopbackResponder(async (responder) => {
    let queries = 0;
    let connectionLost: (() => void) | undefined;
    let extensionOptions: RoonExtensionOptions | undefined;
    const connected = Promise.withResolvers<void>();
    const core: RoonCore = {
      core_id: "core-live",
      services: {
        RoonApiTransport: { subscribe_zones: () => undefined },
      },
    };
    responder.on("message", (query, requester) => {
      queries += 1;
      const transactionId = parseSoodProperties(query, "Q")._tid;
      assert.ok(transactionId);
      respond(responder, requester, validReply(transactionId, "9333"));
    });
    const extension = extensionBoundary((options) => {
      connectionLost = () => {
        options.onclose?.();
        extensionOptions?.core_unpaired(core);
      };
      extensionOptions?.core_paired(core);
      connected.resolve();
      return { transport: { close: () => undefined } };
    });
    const bridge = startRoonBridge({
      authorizationStore: {
        load: () => ({ tokens: { "core-live": "authorized" } }),
        save: () => undefined,
      },
      artworkFiles: {
        stage: async () => ({ revision: 0, path: "/unused" }),
        commit: async () => undefined,
        discard: async () => undefined,
        clear: async () => undefined,
      },
      displayConfigurationStore: {
        load: () => ({
          trackedOutputId: "output-studio",
          trackedOutputName: "Speaker System",
        }),
        save: () => undefined,
      },
      createRoonServices: (options) => {
        extensionOptions = options;
        return {
          extension,
          requiredServices: [],
          status: { services: [], set_status: () => undefined },
        };
      },
      roonServerHost: loopbackRoonServerHost,
      publish: () => undefined,
    });

    try {
      assert.equal(bridge.currentSnapshot().availability, "disconnected");
      await connected.promise;
      assert.equal(bridge.currentSnapshot().availability, "outputUnavailable");
      connectionLost?.();
      context.mock.timers.tick(1_100);
      assert.equal(queries, 1);
      assert.equal(bridge.currentSnapshot().availability, "disconnected");
    } finally {
      await bridge.stop();
    }
  });
});

function extensionBoundary(
  connect: NonNullable<RoonExtension["ws_connect"]>,
): RoonExtension {
  return {
    init_services: () => undefined,
    start_discovery: () => {
      throw new Error("ordinary discovery must not start");
    },
    stop_discovery: () => undefined,
    disconnect_all: () => undefined,
    ws_connect: connect,
  };
}

async function withLoopbackResponder(
  run: (responder: Socket) => Promise<void>,
): Promise<void> {
  const responder = createSocket("udp4");
  await bind(responder, 9003, "127.0.0.1");
  try {
    await run(responder);
  } finally {
    responder.close();
    await once(responder, "close");
  }
}

async function bind(socket: Socket, port: number, address: string) {
  socket.bind(port, address);
  await once(socket, "listening");
}

function respond(socket: Socket, requester: RemoteInfo, packet: Buffer): void {
  socket.send(packet, requester.port, requester.address);
}

function validReply(transactionId: string, port: string): Buffer {
  return soodPacket("R", {
    _tid: transactionId,
    service_id: roonServiceId,
    http_port: port,
  });
}

function soodPacket(
  type: "Q" | "R",
  properties: Record<string, string | null>,
): Buffer {
  const fields = Object.entries(properties).map(([name, value]) => {
    const nameBytes = Buffer.from(name);
    if (value === null) {
      const field = Buffer.alloc(1 + nameBytes.length + 2);
      field.writeUInt8(nameBytes.length, 0);
      nameBytes.copy(field, 1);
      field.writeUInt16BE(65_535, 1 + nameBytes.length);
      return field;
    }
    const valueBytes = Buffer.from(value);
    const field = Buffer.alloc(1 + nameBytes.length + 2 + valueBytes.length);
    field.writeUInt8(nameBytes.length, 0);
    nameBytes.copy(field, 1);
    field.writeUInt16BE(valueBytes.length, 1 + nameBytes.length);
    valueBytes.copy(field, 1 + nameBytes.length + 2);
    return field;
  });
  return Buffer.concat([Buffer.from(`SOOD\x02${type}`), ...fields]);
}

function parseSoodProperties(
  packet: Buffer,
  expectedType: "Q" | "R",
): Record<string, string> {
  assert.equal(packet.subarray(0, 6).toString(), `SOOD\x02${expectedType}`);
  const properties: Record<string, string> = {};
  let offset = 6;
  while (offset < packet.length) {
    const nameLength = packet.readUInt8(offset);
    offset += 1;
    const name = packet.subarray(offset, offset + nameLength).toString();
    offset += nameLength;
    const valueLength = packet.readUInt16BE(offset);
    offset += 2;
    properties[name] = packet.subarray(offset, offset + valueLength).toString();
    offset += valueLength;
  }
  return properties;
}

function requiredRoonServerHost(value: string): RoonServerHost {
  const roonServerHost = parseRoonServerHost(value);
  assert.ok(roonServerHost);
  return roonServerHost;
}
