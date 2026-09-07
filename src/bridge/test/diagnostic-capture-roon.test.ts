import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";

import { DiagnosticCapture } from "../src/diagnostic-capture.js";
import { startRoonBridge } from "../src/roon-bridge.js";
import { createSupportedRoonServices } from "../src/roon-services.js";
import type { PresentationSnapshot } from "../src/snapshot.js";
import { withTaskDirectory } from "./support.js";

interface PeerSocket extends EventEmitter {
  send(data: Buffer): void;
  terminate(): void;
}
interface PeerServer extends EventEmitter {
  address(): { port: number };
  clients: Set<PeerSocket>;
  close(callback: () => void): void;
}
const { WebSocketServer } = createRequire(import.meta.url)("ws") as {
  WebSocketServer: new (options: { host: string; port: number }) => PeerServer;
};

function message(
  verb: string,
  name: string,
  id: string,
  body?: unknown,
): Buffer {
  const data =
    body === undefined
      ? undefined
      : Buffer.isBuffer(body)
        ? body
        : Buffer.from(JSON.stringify(body));
  return Buffer.concat([
    Buffer.from(
      `MOO/1 ${verb} ${name}\nRequest-Id: ${id}\n${data ? `Content-Length: ${data.length}\nContent-Type: ${Buffer.isBuffer(body) ? "image/jpeg" : "application/json"}\n` : ""}\n`,
    ),
    data ?? Buffer.alloc(0),
  ]);
}

async function observeSyntheticRoonSession(
  directory?: string,
  {
    reconnect = false,
    lyricsEnabled = true,
    ending,
    pressure = false,
  }: {
    reconnect?: boolean;
    lyricsEnabled?: boolean;
    ending?: "unknown" | "malformed";
    pressure?: boolean;
  } = {},
) {
  const server = new WebSocketServer({ host: "127.0.0.1", port: 0 });
  await once(server, "listening");
  const traffic: string[] = [];
  const snapshots: PresentationSnapshot[] = [];
  const logs: string[] = [];
  const capture =
    directory === undefined
      ? undefined
      : new DiagnosticCapture({ directory }, (line) => logs.push(line));
  let connections = 0;
  const subscribed = new Set<string>();
  const peers = new Map<string, PeerSocket>();
  const zone = {
    zone_id: "zone-1",
    display_name: "Studio",
    state: "playing",
    outputs: [{ output_id: "speaker", display_name: "Speaker" }],
    now_playing: {
      image_key: "image-1",
      three_line: { line1: "Title", line2: "Artist", line3: "Album" },
      length: 180,
      seek_position: 12,
    },
  };
  server.on("connection", (socket: PeerSocket) => {
    connections += 1;
    let role = "unknown";
    socket.on("message", (buffer: Buffer) => {
      const text = buffer.toString();
      const first = text.split("\n")[0]!;
      const id = /Request-Id: (\d+)/.exec(text)![1]!;
      const body = text.slice(text.indexOf("\n\n") + 2);
      if (first.includes("/register")) {
        role = JSON.parse(body).extension_id;
        peers.set(role, socket);
      }
      // Connection scheduling may interleave; compare per-message content as a multiset.
      traffic.push(text);
      if (first.endsWith("registry:1/info")) {
        socket.send(
          message("COMPLETE", "Success", id, {
            core_id: "core-1",
            display_name: "Synthetic server",
          }),
        );
      } else if (first.endsWith("registry:1/register")) {
        socket.send(
          message("CONTINUE", "Registered", id, {
            core_id: "core-1",
            token: "synthetic-private-token",
            provided_services: [
              "com.roonlabs.image:1",
              "com.roonlabs.transport:2",
            ],
          }),
        );
        socket.send(
          message("REQUEST", "com.roonlabs.pairing:1/get_pairing", "800"),
        );
        socket.send(
          message("REQUEST", "unknown.service:1/rejected", "801", {
            nested: { credentials: { password: "synthetic-password" } },
            image_key: "nonsecret-key",
          }),
        );
      } else if (first.endsWith("transport:2/subscribe_zones")) {
        socket.send(
          message("CONTINUE", "Subscribed", id, {
            zones: [zone, { ...zone, zone_id: "ignored-zone", outputs: [] }],
          }),
        );
        socket.send(
          message("CONTINUE", "Changed", id, { zones_changed: [zone] }),
        );
        socket.send(
          message("CONTINUE", "Ignored", id, {
            marker: "ignored-decoded-input",
            ...(pressure ? { oversized: "x".repeat(2 * 1024 * 1024) } : {}),
          }),
        );
        if (role === "com.roonlabs.display_zone")
          socket.send(
            message("CONTINUE", "LyricsChanged", id, {
              zone_id: "stale-zone",
              marker: "stale-lyrics",
              lrc: "[00:01]Stale",
            }),
          );
        subscribed.add(role);
      } else if (first.endsWith("image:1/get_image")) {
        socket.send(
          message(
            "COMPLETE",
            "Success",
            id,
            Buffer.from("synthetic-artwork-bytes"),
          ),
        );
      }
    });
  });
  const sockets: Array<{ transport: { close(): void } }> = [];
  let connectOrdinary = () => undefined as void;
  const bridge = startRoonBridge({
    diagnosticCapture: capture,
    authorizationStore: { load: () => ({}), save: () => undefined },
    displayConfigurationStore: {
      load: () => ({
        trackedOutputId: "speaker",
        trackedOutputName: "Speaker",
        lyricsEnabled,
      }),
      save: () => undefined,
    },
    artworkFiles: {
      stage: async (revision) => ({ revision, path: "artwork/fixed.jpg" }),
      commit: async () => undefined,
      discard: async () => undefined,
      clear: async () => undefined,
    },
    createRoonServices: (options, diagnosticCapture) => {
      const services = createSupportedRoonServices(options, diagnosticCapture);
      services.extension.start_discovery = () => {
        sockets.push(
          services.extension.ws_connect!({
            host: "127.0.0.1",
            port: server.address().port,
          }),
        );
      };
      connectOrdinary = services.extension.start_discovery;
      services.extension.stop_discovery = () => undefined;
      services.extension.disconnect_all = () =>
        sockets.forEach((socket) => socket.transport.close());
      return services;
    },
    now: () => new Date("2026-01-01T00:00:00Z"),
    publish: (snapshot) => snapshots.push(structuredClone(snapshot)),
  });
  try {
    for (
      let attempt = 0;
      attempt < 200 &&
      (subscribed.size < (lyricsEnabled ? 2 : 1) ||
        !snapshots.some((snapshot) => snapshot.artwork !== null));
      attempt += 1
    )
      await delay(10);
    assert.equal(subscribed.size, lyricsEnabled ? 2 : 1);
    await delay(30);
    if (reconnect) {
      sockets[0]!.transport.close();
      await delay(10);
      connectOrdinary();
      for (
        let attempt = 0;
        attempt < 200 && connections < (lyricsEnabled ? 4 : 2);
        attempt += 1
      )
        await delay(10);
      await delay(50);
    }
    if (ending) {
      const peer = peers.get("io.roonscape.bridge")!;
      peer.send(
        ending === "unknown"
          ? message("COMPLETE", "LateRejected", "99999", {
              marker: "unknown-request-id",
              core_id: "unrelated-core",
            })
          : Buffer.from("malformed-network-traffic"),
      );
      await delay(30);
    }
  } finally {
    await bridge.stop();
    for (const socket of server.clients) socket.terminate();
    await new Promise<void>((resolve) => server.close(resolve));
    await capture?.close();
  }
  return { traffic: traffic.sort(), snapshots, logs, connections };
}

test(
  "capture preserves Roon traffic and snapshots across both SDK connection paths",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      const disabled = await observeSyntheticRoonSession();
      const destination = path.join(directory, "capture");
      const enabled = await observeSyntheticRoonSession(destination);
      assert.deepEqual(enabled.traffic, disabled.traffic);
      assert.deepEqual(enabled.snapshots, disabled.snapshots);
      assert.equal(enabled.connections, 2);
      assert.deepEqual(enabled.logs, []);
      const records = await readRecords(destination);
      assert.doesNotMatch(
        JSON.stringify(records),
        /synthetic-private-token|synthetic-password|synthetic-artwork-bytes/,
      );
      const inbound = records.filter((record) => record.type === "inbound");
      assert.equal(
        inbound.filter((record) => record.data.message.name === "Registered")
          .length,
        2,
      );
      assert.equal(
        inbound.filter((record) => record.data.message.name === "get_pairing")
          .length,
        2,
      );
      assert.equal(
        inbound.filter((record) => record.data.message.name === "rejected")
          .length,
        2,
      );
      assert.equal(
        inbound.filter((record) => record.data.message.name === "Ignored")
          .length,
        2,
      );
      assert.equal(
        inbound.filter((record) => record.data.message.name === "LyricsChanged")
          .length,
        1,
      );
      assert.equal(
        new Set(inbound.map((record) => record.data.connectionId)).size,
        2,
      );
      assert.deepEqual(
        records
          .filter((record) => record.type === "snapshot")
          .map((record) => record.data),
        enabled.snapshots,
      );
      const image = inbound.find((record) => record.data.artwork);
      assert.deepEqual(image.data.message.body, { artworkOmitted: true });
      assert.deepEqual(image.data.artwork, {
        request: {
          image_key: "image-1",
          scale: "fit",
          width: 1600,
          height: 1600,
          format: "image/jpeg",
        },
        coreId: "core-1",
        status: "Success",
        contentType: "image/jpeg",
        byteCount: 23,
      });
      assert.ok(
        inbound
          .find((record) => record.data.message.name === "Subscribed")
          .data.message.body.zones.some(
            (zone: { zone_id: string }) => zone.zone_id === "ignored-zone",
          ),
      );
      const badDestination = path.join(directory, "synthetic-secret-path");
      await writeFile(badDestination, "not a directory");
      const failing = await observeSyntheticRoonSession(badDestination);
      assert.deepEqual(failing.traffic, disabled.traffic);
      assert.deepEqual(failing.snapshots, disabled.snapshots);
      assert.equal(failing.connections, 2);
      assert.ok(failing.logs.length > 0);
      assert.doesNotMatch(failing.logs.join("\n"), /synthetic-/);
    });
  },
);

test(
  "reconnects get distinct identities without enabling a disabled Lyric Feed",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      const disabled = await observeSyntheticRoonSession(undefined, {
        reconnect: true,
        lyricsEnabled: false,
      });
      const enabled = await observeSyntheticRoonSession(directory, {
        reconnect: true,
        lyricsEnabled: false,
      });
      assert.deepEqual(enabled.traffic, disabled.traffic);
      assert.deepEqual(enabled.snapshots, disabled.snapshots);
      assert.equal(enabled.connections, 2);
      const records = await readRecords(directory);
      const inbound = records.filter((record) => record.type === "inbound");
      assert.equal(
        new Set(inbound.map((record) => record.data.connectionId)).size,
        2,
      );
      assert.ok(inbound.every((record) => record.data.role === "ordinary"));
      assert.equal(
        inbound.filter((record) => record.data.message?.name === "Registered")
          .length,
        2,
      );
      assert.equal(
        records.filter(
          (record) =>
            record.type === "connection" && record.data.state === "closed",
        ).length,
        2,
      );
    });
  },
);

for (const ending of ["unknown", "malformed"] as const) {
  test(
    `records decoded rejected responses but excludes malformed traffic: ${ending}`,
    { timeout: 10000 },
    async () => {
      await withTaskDirectory(async (directory) => {
        const disabled = await observeSyntheticRoonSession(undefined, {
          lyricsEnabled: false,
          ending,
        });
        const enabled = await observeSyntheticRoonSession(directory, {
          lyricsEnabled: false,
          ending,
        });
        assert.deepEqual(enabled.traffic, disabled.traffic);
        assert.deepEqual(enabled.snapshots, disabled.snapshots);
        const records = await readRecords(directory);
        assert.doesNotMatch(
          JSON.stringify(records),
          /malformed-network-traffic/,
        );
        if (ending === "unknown")
          assert.equal(
            records.find(
              (record) => record.data.message?.name === "LateRejected",
            )?.data.coreId,
            "core-1",
          );
        assert.equal(
          records.filter(
            (record) =>
              record.type === "inbound" &&
              record.data.message?.name === "LateRejected",
          ).length,
          ending === "unknown" ? 1 : 0,
        );
      });
    },
  );
}

async function readRecords(directory: string) {
  const contents = (
    await Promise.all(
      (await readdir(directory))
        .filter((file) => file.endsWith(".jsonl"))
        .sort()
        .map((file) => readFile(path.join(directory, file), "utf8")),
    )
  ).join("");
  return contents
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
}

test(
  "oversized recording omissions preserve traffic and Presentation Snapshots",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      const disabled = await observeSyntheticRoonSession(undefined, {
        pressure: true,
      });
      const enabled = await observeSyntheticRoonSession(directory, {
        pressure: true,
      });
      assert.deepEqual(enabled.traffic, disabled.traffic);
      assert.deepEqual(enabled.snapshots, disabled.snapshots);
      assert.ok(enabled.logs.some((line) => line.includes("records omitted")));
      assert.ok(
        (await readRecords(directory)).some((record) => record.type === "gap"),
      );
    });
  },
);
