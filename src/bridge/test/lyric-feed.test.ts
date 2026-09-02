import assert from "node:assert/strict";
import test from "node:test";

import {
  startLyricFeed,
  type LyricFeedConnection,
  type LyricFeedConnectionFactory,
  type PrivateLyricEvent,
} from "../src/lyric-feed.js";

function createBoundary() {
  const connections: Array<{
    emit(
      event: PrivateLyricEvent,
      observedNowPlayingIdentity?: string | null,
    ): void;
    disconnect(): void;
    reports: string[];
    stopped: boolean;
  }> = [];
  const schedules: Array<{ cancelled: boolean; reconnect(): void }> = [];
  const endpoints: Array<{ host: string; port: number; coreId: string }> = [];
  const factory: LyricFeedConnectionFactory = ({
    endpoint,
    expectedCoreId,
    onEvent,
    onDisconnect,
  }) => {
    endpoints.push({ ...endpoint, coreId: expectedCoreId });
    const boundary = {
      emit: onEvent,
      disconnect: onDisconnect,
      reports: [] as string[],
      stopped: false,
    };
    connections.push(boundary);
    const connection: LyricFeedConnection = {
      reportViewed: (key) => boundary.reports.push(key),
      stop: () => {
        boundary.stopped = true;
      },
    };
    return connection;
  };
  const timelines: unknown[] = [];
  const feed = startLyricFeed({
    endpoint: { host: "roon.local", port: 9330 },
    expectedCoreId: "core-1",
    connect: factory,
    onTimeline: (timeline) => timelines.push(timeline),
    scheduleReconnect: (reconnect) => {
      const scheduled = { cancelled: false, reconnect };
      schedules.push(scheduled);
      return () => {
        scheduled.cancelled = true;
      };
    },
  });
  return { connections, endpoints, feed, schedules, timelines };
}

test("targets only the ordinary connection's exact Core endpoint", () => {
  const boundary = createBoundary();

  assert.deepEqual(boundary.endpoints, [
    { host: "roon.local", port: 9330, coreId: "core-1" },
  ]);
});

test("publishes only well-formed synchronized lyrics for the tracked Now Playing", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  boundary.connections[0]?.emit({
    zone_id: "zone-b",
    key: "wrong-zone-key",
    lrc: "[00:01.00]wrong zone",
  });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "untimed-key",
    lrc: null,
  });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]First\n[00:04.00]Second",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  assert.deepEqual(boundary.timelines, [
    {
      cues: [
        { atSeconds: 1, text: "First" },
        { atSeconds: 4, text: "Second" },
      ],
    },
  ]);
});

test("retains the latest initial lyric event by zone until Now Playing is known", () => {
  const boundary = createBoundary();
  boundary.connections[0]?.emit({
    zone_id: "zone-b",
    key: "zone-b-key",
    lrc: "[00:01.00]Wrong zone",
  });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]Initial cue",
  });

  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  assert.deepEqual(boundary.timelines, [
    { cues: [{ atSeconds: 1, text: "Initial cue" }] },
  ]);
});

test("waits for ordinary Now Playing to confirm an unknown key during a track transition", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  boundary.connections[0]?.emit(
    {
      zone_id: "zone-a",
      key: "track-b-key",
      lrc: "[00:01.00]Track B cue",
    },
    "track-b",
  );

  assert.deepEqual(boundary.timelines, []);

  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-b" });

  assert.deepEqual(boundary.timelines, [
    { cues: [{ atSeconds: 1, text: "Track B cue" }] },
  ]);
});

test("merges another zone's event while a different zone is tracked", () => {
  const boundary = createBoundary();
  const zoneA = { zoneId: "zone-a", nowPlayingIdentity: "track-a" };
  const zoneB = { zoneId: "zone-b", nowPlayingIdentity: "track-b" };
  boundary.feed.track(zoneA, [zoneA, zoneB]);
  boundary.connections[0]?.emit({
    zone_id: "zone-b",
    key: "zone-b-key",
    lrc: "[00:02.00]Grouped-zone cue",
  });

  boundary.feed.track(zoneB, [zoneA, zoneB]);

  assert.deepEqual(boundary.timelines, [
    { cues: [{ atSeconds: 2, text: "Grouped-zone cue" }] },
  ]);
});

test("does not resurrect a pending zone event after track change or disconnect", () => {
  const boundary = createBoundary();
  const zoneA = { zoneId: "zone-a", nowPlayingIdentity: "track-a" };
  const oldZoneB = { zoneId: "zone-b", nowPlayingIdentity: "track-b-old" };
  const newZoneB = { zoneId: "zone-b", nowPlayingIdentity: "track-b-new" };
  boundary.feed.track(zoneA, [zoneA, oldZoneB]);
  boundary.connections[0]?.emit({
    zone_id: "zone-b",
    key: "old-zone-b-key",
    lrc: "[00:02.00]Stale cue",
  });

  boundary.feed.track(newZoneB, [zoneA, newZoneB]);
  assert.deepEqual(boundary.timelines, []);

  boundary.feed.track(zoneA, [zoneA, oldZoneB]);
  boundary.connections[0]?.emit({
    zone_id: "zone-b",
    key: "old-zone-b-key",
    lrc: "[00:02.00]Disconnected cue",
  });
  boundary.connections[0]?.disconnect();
  boundary.feed.track(oldZoneB, [zoneA, oldZoneB]);
  assert.deepEqual(boundary.timelines, []);
});

test("clears immediately and rejects late keys after track and zone changes", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]First",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-b" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:02.00]Late",
  });
  boundary.feed.track({ zoneId: "zone-b", nowPlayingIdentity: "track-b" });

  assert.deepEqual(boundary.timelines, [
    { cues: [{ atSeconds: 1, text: "First" }] },
    null,
  ]);
});

test("accepts a known key again only when its original track returns", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]First",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-b" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:02.00]Late",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:03.00]Returned",
  });

  assert.deepEqual(boundary.timelines, [
    { cues: [{ atSeconds: 1, text: "First" }] },
    null,
    { cues: [{ atSeconds: 3, text: "Returned" }] },
  ]);
});

test("reports a non-null lyric key once only after visibility", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]First",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  boundary.feed.markVisible();
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]First",
  });
  boundary.feed.markVisible();

  assert.deepEqual(boundary.connections[0]?.reports, ["track-a-key"]);
});

test("forgets key correlation when the lyric connection disconnects", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "reused-key",
    lrc: "[00:01.00]First track",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.disconnect();
  boundary.schedules[0]?.reconnect();

  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-b" });
  boundary.connections[1]?.emit({
    zone_id: "zone-a",
    key: "reused-key",
    lrc: "[00:02.00]Second track",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-b" });

  assert.deepEqual(boundary.timelines, [
    { cues: [{ atSeconds: 1, text: "First track" }] },
    null,
    { cues: [{ atSeconds: 2, text: "Second track" }] },
  ]);
});

test("view-report failure remains optional capability loss", () => {
  const factory: LyricFeedConnectionFactory = ({ onEvent }) => {
    queueMicrotask(() =>
      onEvent({
        zone_id: "zone-a",
        key: "track-a-key",
        lrc: "[00:01.00]First",
      }),
    );
    return {
      reportViewed: () => {
        throw new Error("private report failed");
      },
      stop: () => undefined,
    };
  };
  const feed = startLyricFeed({
    endpoint: { host: "roon.local", port: 9330 },
    expectedCoreId: "core-1",
    connect: factory,
    onTimeline: () => undefined,
  });
  feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  return new Promise<void>((resolve) => {
    setImmediate(() => {
      assert.doesNotThrow(() => feed.markVisible());
      feed.stop();
      resolve();
    });
  });
});

test("disconnect is optional capability loss and reconnects without retaining authorization", () => {
  const boundary = createBoundary();
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });
  boundary.connections[0]?.emit({
    zone_id: "zone-a",
    key: "track-a-key",
    lrc: "[00:01.00]First",
  });
  boundary.feed.track({ zoneId: "zone-a", nowPlayingIdentity: "track-a" });

  boundary.connections[0]?.disconnect();
  assert.equal(boundary.timelines.at(-1), null);
  boundary.schedules[0]?.reconnect();
  assert.equal(boundary.connections.length, 2);

  boundary.feed.stop();
  assert.equal(boundary.connections[1]?.stopped, true);
});

test("a failed reconnect remains optional capability loss and retries again", () => {
  const schedules: Array<{ cancelled: boolean; reconnect(): void }> = [];
  let connectionAttempts = 0;
  let disconnect: (() => void) | undefined;
  const feed = startLyricFeed({
    endpoint: { host: "roon.local", port: 9330 },
    expectedCoreId: "core-1",
    connect: ({ onDisconnect }) => {
      connectionAttempts += 1;
      if (connectionAttempts === 2) {
        throw new Error("private reconnect failed");
      }
      disconnect = onDisconnect;
      return {
        reportViewed: () => undefined,
        stop: () => undefined,
      };
    },
    onTimeline: () => undefined,
    scheduleReconnect: (reconnect) => {
      const scheduled = { cancelled: false, reconnect };
      schedules.push(scheduled);
      return () => {
        scheduled.cancelled = true;
      };
    },
  });

  disconnect?.();
  assert.doesNotThrow(() => schedules[0]?.reconnect());
  assert.equal(connectionAttempts, 2);
  assert.equal(schedules.length, 2);

  schedules[1]?.reconnect();
  assert.equal(connectionAttempts, 3);
  feed.stop();
});
