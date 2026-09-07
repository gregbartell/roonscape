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
    provenance = false,
    rotate = false,
    emptyLyrics = false,
    largeLyrics = false,
  }: {
    reconnect?: boolean;
    lyricsEnabled?: boolean;
    ending?: "unknown" | "malformed";
    pressure?: boolean;
    provenance?: boolean;
    rotate?: boolean;
    emptyLyrics?: boolean;
    largeLyrics?: boolean;
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
      : new DiagnosticCapture(
          { directory, budgetBytes: provenance ? 1024 * 1024 : undefined },
          (line) => logs.push(line),
        );
  let connections = 0;
  const subscribed = new Set<string>();
  const subscriptions = new Map<string, string>();
  const images: Array<{ socket: PeerSocket; id: string }> = [];
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
        subscriptions.set(role, id);
      } else if (first.endsWith("image:1/get_image")) {
        if (provenance) {
          images.push({ socket, id });
          return;
        }
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
        (provenance
          ? images.length === 0
          : !snapshots.some((snapshot) => snapshot.artwork !== null)));
      attempt += 1
    )
      await delay(10);
    assert.equal(subscribed.size, lyricsEnabled ? 2 : 1);
    await delay(30);
    if (provenance) {
      const ordinary = peers.get("io.roonscape.bridge")!;
      const lyrics = peers.get("com.roonlabs.display_zone")!;
      lyrics.send(
        message(
          "CONTINUE",
          "LyricsChanged",
          subscriptions.get("com.roonlabs.display_zone")!,
          {
            zone_id: "zone-1",
            key: emptyLyrics ? null : "lyrics-1",
            lrc: emptyLyrics
              ? null
              : largeLyrics
                ? Array.from(
                    { length: 256 },
                    (_, index) =>
                      `[${String(Math.floor(index / 60)).padStart(2, "0")}:${String(index % 60).padStart(2, "0")}.00]${"🎵".repeat(60)}`,
                  ).join("\n")
                : "[00:01.00]Accepted lyrics",
          },
        ),
      );
      await delay(20);
      ordinary.send(
        message(
          "CONTINUE",
          "Changed",
          subscriptions.get("io.roonscape.bridge")!,
          {
            zones_seek_changed: [{ zone_id: "zone-1", seek_position: 24 }],
          },
        ),
      );
      lyrics.send(
        message(
          "CONTINUE",
          "LyricsChanged",
          subscriptions.get("com.roonlabs.display_zone")!,
          {
            zone_id: "stale-zone",
            key: "stale-key",
            lrc: "[00:01.00]Rejected lyrics",
          },
        ),
      );
      await delay(20);
      images[0]!.socket.send(
        message(
          "COMPLETE",
          "Success",
          images[0]!.id,
          Buffer.from("synthetic-artwork-bytes"),
        ),
      );
      await delay(30);
      assert.equal(snapshots.at(-1)?.timing?.position?.seconds, 24);
      assert.equal(
        snapshots.at(-1)?.lyrics?.cues[0]?.text,
        emptyLyrics || largeLyrics ? undefined : "Accepted lyrics",
      );
      if (rotate && capture) {
        for (let index = 0; index < 1200; index += 1) {
          capture.record("inbound", {
            marker: "rotation filler",
            text: "x".repeat(1500),
          });
          await delay(1);
        }
      }
      for (const imageKey of ["image-2", "image-3"]) {
        ordinary.send(
          message(
            "CONTINUE",
            "Changed",
            subscriptions.get("io.roonscape.bridge")!,
            {
              zones_changed: [
                {
                  ...zone,
                  now_playing: { ...zone.now_playing, image_key: imageKey },
                },
              ],
            },
          ),
        );
        await delay(20);
      }
      assert.equal(images.length, 3);
      images[1]!.socket.send(
        message(
          "COMPLETE",
          "Success",
          images[1]!.id,
          Buffer.from("stale-artwork"),
        ),
      );
      images[2]!.socket.send(
        message(
          "COMPLETE",
          "Success",
          images[2]!.id,
          Buffer.from("accepted-artwork"),
        ),
      );
      await delay(30);
    }
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
          .map((record) => record.data.snapshot),
        enabled.snapshots,
      );
      const published = records.find(
        (record) => record.type === "snapshot" && record.data.snapshot.artwork,
      );
      assert.equal(
        published.data.contributors.zone.context.message.name,
        "Changed",
      );
      assert.equal(
        published.data.contributors.artwork.context.artwork.request.image_key,
        "image-1",
      );
      assert.equal(
        published.data.contributors.artwork.sequence,
        inbound.find((record) => record.data.artwork).sequence,
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
      const artworkPublications = records.filter(
        (record) => record.type === "snapshot" && record.data.snapshot.artwork,
      );
      assert.equal(artworkPublications.length, 2);
      assert.notEqual(
        artworkPublications[0].data.contributors.zone.context.connectionId,
        artworkPublications[1].data.contributors.zone.context.connectionId,
      );
      for (const publication of artworkPublications) {
        assert.equal(
          publication.data.contributors.zone.context.connectionId,
          publication.data.contributors.artwork.context.connectionId,
        );
      }
      for (const disconnected of records.filter(
        (record) =>
          record.type === "snapshot" &&
          record.data.snapshot.availability === "disconnected",
      )) {
        assert.equal(
          disconnected.data.contributors.availability.type,
          "connection",
        );
        assert.equal(disconnected.data.contributors.zone, undefined);
      }
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

test(
  "pending snapshots retain zone, lyric, seek and accepted artwork contributors",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      const disabled = await observeSyntheticRoonSession(undefined, {
        provenance: true,
      });
      const enabled = await observeSyntheticRoonSession(directory, {
        provenance: true,
      });
      assert.deepEqual(enabled.traffic, disabled.traffic);
      assert.deepEqual(enabled.snapshots, disabled.snapshots);
      const records = await readRecords(directory);
      const combined = records.find(
        (record) =>
          record.type === "snapshot" && record.data.snapshot.lyrics !== null,
      );
      assert.ok(combined);
      const { zone, timing, lyrics, artwork } = combined.data.contributors;
      assert.equal(
        zone.context.message.body.zones_changed[0].now_playing.three_line.line1,
        "Title",
      );
      assert.equal(
        timing.context.message.body.zones_seek_changed[0].seek_position,
        24,
      );
      assert.equal(
        lyrics.context.message.body.lrc,
        "[00:01.00]Accepted lyrics",
      );
      assert.equal(artwork.context.artwork.request.image_key, "image-1");
      assert.ok(
        zone.sequence < lyrics.sequence &&
          lyrics.sequence < timing.sequence &&
          timing.sequence < artwork.sequence,
      );
      const publications = records.filter(
        (record) => record.type === "snapshot",
      );
      assert.ok(
        publications.some(
          (record) =>
            record.data.contributors.artwork?.context.artwork.request
              .image_key === "image-3",
        ),
      );
      assert.ok(
        publications.every(
          (record) =>
            record.data.contributors.artwork?.context.artwork.request
              .image_key !== "image-2",
        ),
      );
      assert.ok(
        records.some(
          (record) =>
            record.type === "inbound" &&
            record.data.artwork?.request.image_key === "image-2",
        ),
      );
      const destination = path.join(directory, "failure");
      await writeFile(destination, "unavailable");
      const failing = await observeSyntheticRoonSession(destination, {
        provenance: true,
      });
      assert.deepEqual(failing.traffic, disabled.traffic);
      assert.deepEqual(failing.snapshots, disabled.snapshots);
    });
  },
);

test(
  "rotation checkpoints preserve accumulated state and deleted contributing inputs",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      await observeSyntheticRoonSession(directory, {
        provenance: true,
        rotate: true,
      });
      const records = await readRecords(directory);
      const checkpoint = records.find(
        (record) =>
          record.type === "checkpoint" &&
          record.data.snapshot?.snapshot?.lyrics !== null,
      );
      assert.ok(checkpoint?.data.snapshot?.snapshot?.lyrics);
      assert.equal(checkpoint.data.kind, "retained state");
      assert.equal(checkpoint.data.roonState.status, "observed");
      assert.equal(
        checkpoint.data.roonState.zones.find(
          (retained: { zone: { zone_id: string } }) =>
            retained.zone.zone_id === "zone-1",
        ).zone.now_playing.seek_position,
        24,
      );
      assert.equal(
        checkpoint.data.snapshot.snapshot.timing.position.seconds,
        24,
      );
      assert.equal(
        checkpoint.data.snapshot.contributors.lyrics.context.message.body.lrc,
        "[00:01.00]Accepted lyrics",
      );
      assert.equal(
        checkpoint.data.snapshot.contributors.artwork.context.artwork.request
          .image_key,
        "image-1",
      );
      for (const source of Object.values(
        checkpoint.data.snapshot.contributors,
      ) as Array<{ sequence: number }>) {
        assert.ok(
          !records.some(
            (record) =>
              record.type === "inbound" && record.sequence === source.sequence,
          ),
        );
      }
      assert.doesNotMatch(
        JSON.stringify(records),
        /synthetic-private-token|synthetic-artwork-bytes/,
      );
      for (const file of (await readdir(directory)).filter((name) =>
        name.endsWith(".jsonl"),
      )) {
        assert.equal(
          JSON.parse(
            (await readFile(path.join(directory, file), "utf8")).split(
              "\n",
            )[0]!,
          ).type,
          "checkpoint",
        );
      }
    });
  },
);

test(
  "retained Lyric Feed state distinguishes an observed empty response",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      await observeSyntheticRoonSession(directory, {
        provenance: true,
        rotate: true,
        emptyLyrics: true,
      });
      const records = await readRecords(directory);
      const checkpoint = records.find(
        (record) =>
          record.type === "checkpoint" &&
          record.data.lyricFeed?.state?.observation?.status ===
            "observed empty",
      );
      assert.ok(checkpoint);
      assert.equal(
        checkpoint.data.lyricFeed.state.observation.source.context.message.body
          .lrc,
        null,
      );
      assert.ok(
        !records.some(
          (record) =>
            record.type === "inbound" &&
            record.sequence ===
              checkpoint.data.lyricFeed.state.observation.source.sequence,
        ),
      );
    });
  },
);

test(
  "capture identifies local oversized-lyric recovery while artwork is pending",
  { timeout: 10000 },
  async () => {
    await withTaskDirectory(async (directory) => {
      const disabled = await observeSyntheticRoonSession(undefined, {
        provenance: true,
        largeLyrics: true,
      });
      const enabled = await observeSyntheticRoonSession(directory, {
        provenance: true,
        largeLyrics: true,
      });
      assert.deepEqual(enabled.traffic, disabled.traffic);
      assert.deepEqual(enabled.snapshots, disabled.snapshots);
      const publication = (await readRecords(directory)).find(
        (record) => record.type === "snapshot" && record.data.snapshot.artwork,
      );
      assert.match(publication.data.trigger, /lyrics omitted.*size/);
      assert.equal(publication.data.snapshot.lyrics, null);
    });
  },
);
