import { createHash } from "node:crypto";

import {
  parseSynchronizedLyrics,
  type SynchronizedLyrics,
} from "./synchronized-lyrics.js";

const MAX_CORRELATED_KEYS = 64;
const MAX_PENDING_ZONES = 64;

export interface LyricFeedEndpoint {
  host: string;
  port: number;
}

export interface PrivateLyricEvent {
  zone_id?: unknown;
  key?: unknown;
  lrc?: unknown;
}

interface ValidPrivateLyricEvent {
  zone_id: string;
  key: string | null;
  lrc: string | null;
}

interface PendingPrivateLyricEvent {
  event: ValidPrivateLyricEvent;
  nowPlayingIdentity: string | null;
}

export interface LyricFeedConnection {
  reportViewed(key: string): void;
  stop(): void;
}

interface ConnectLyricFeedOptions {
  endpoint: LyricFeedEndpoint;
  expectedCoreId: string;
  onEvent(
    event: PrivateLyricEvent,
    observedNowPlayingIdentity?: string | null,
  ): void;
  onDisconnect(): void;
}

export type LyricFeedConnectionFactory = (
  options: ConnectLyricFeedOptions,
) => LyricFeedConnection;

export interface TrackedNowPlaying {
  zoneId: string;
  nowPlayingIdentity: string;
}

export interface LyricNowPlayingZone {
  zone_id: string;
  state: string;
  now_playing?: {
    image_key?: string;
    length?: number;
    three_line?: {
      line1?: string;
      line2?: string;
      line3?: string;
    };
  };
}

export interface LyricFeed {
  track(
    nowPlaying: TrackedNowPlaying | null,
    knownNowPlaying?: readonly TrackedNowPlaying[],
  ): void;
  markVisible(): void;
  stop(): void;
}

interface StartLyricFeedOptions {
  endpoint: LyricFeedEndpoint;
  expectedCoreId: string;
  connect: LyricFeedConnectionFactory;
  onTimeline(timeline: SynchronizedLyrics | null): void;
  scheduleReconnect?: (reconnect: () => void) => () => void;
}

export function startLyricFeed({
  endpoint,
  expectedCoreId,
  connect,
  onTimeline,
  scheduleReconnect = scheduleReconnectWithTimeout,
}: StartLyricFeedOptions): LyricFeed {
  let tracked: TrackedNowPlaying | null = null;
  let connection: LyricFeedConnection | undefined;
  let cancelReconnect: (() => void) | undefined;
  let stopped = false;
  let timelineVisible = false;
  let acceptedKey: string | null = null;
  let acceptedTimeline: SynchronizedLyrics | null = null;
  let hasTimeline = false;
  const keyTrackIdentities = new Map<string, string>();
  const knownTrackIdentitiesByZone = new Map<string, string>();
  const pendingEventsByZone = new Map<string, PendingPrivateLyricEvent>();

  const retainEvent = (
    event: ValidPrivateLyricEvent,
    nowPlayingIdentity = knownTrackIdentitiesByZone.get(event.zone_id) ?? null,
  ): void => {
    pendingEventsByZone.delete(event.zone_id);
    pendingEventsByZone.set(event.zone_id, {
      event,
      nowPlayingIdentity,
    });
    if (pendingEventsByZone.size > MAX_PENDING_ZONES) {
      const oldestZone = pendingEventsByZone.keys().next().value;
      if (oldestZone !== undefined) {
        pendingEventsByZone.delete(oldestZone);
      }
    }
  };

  const clear = (): void => {
    acceptedKey = null;
    acceptedTimeline = null;
    timelineVisible = false;
    if (hasTimeline) {
      hasTimeline = false;
      onTimeline(null);
    }
  };

  const acceptEvent = (event: ValidPrivateLyricEvent): void => {
    if (tracked === null || event.zone_id !== tracked.zoneId) {
      return;
    }
    const { key } = event;
    const keyFingerprint = key === null ? null : fingerprintKey(key);
    if (keyFingerprint !== null) {
      const knownTrackIdentity = keyTrackIdentities.get(keyFingerprint);
      if (
        knownTrackIdentity !== undefined &&
        knownTrackIdentity !== tracked.nowPlayingIdentity
      ) {
        return;
      }
    }
    const timeline = parseSynchronizedLyrics(event.lrc);
    if (timeline === null) {
      clear();
      return;
    }
    if (key === acceptedKey && sameTimeline(timeline, acceptedTimeline)) {
      return;
    }
    acceptedKey = key;
    acceptedTimeline = timeline;
    if (keyFingerprint !== null) {
      keyTrackIdentities.delete(keyFingerprint);
      keyTrackIdentities.set(keyFingerprint, tracked.nowPlayingIdentity);
      if (keyTrackIdentities.size > MAX_CORRELATED_KEYS) {
        const oldestKey = keyTrackIdentities.keys().next().value;
        if (oldestKey !== undefined) {
          keyTrackIdentities.delete(oldestKey);
        }
      }
    }
    timelineVisible = false;
    hasTimeline = true;
    onTimeline(timeline);
  };

  const scheduleOpen = (): void => {
    cancelReconnect?.();
    cancelReconnect = scheduleReconnect(() => {
      cancelReconnect = undefined;
      open();
    });
  };

  const loseConnection = (): void => {
    if (stopped) {
      return;
    }
    connection = undefined;
    clear();
    keyTrackIdentities.clear();
    pendingEventsByZone.clear();
    knownTrackIdentitiesByZone.clear();
    scheduleOpen();
  };

  const open = (): void => {
    if (stopped) {
      return;
    }
    connection = undefined;
    try {
      connection = connect({
        endpoint,
        expectedCoreId,
        onEvent: (event, observedNowPlayingIdentity) => {
          if (
            stopped ||
            typeof event.zone_id !== "string" ||
            (event.key !== null && typeof event.key !== "string") ||
            (event.lrc !== null && typeof event.lrc !== "string")
          ) {
            return;
          }
          const validEvent: ValidPrivateLyricEvent = {
            zone_id: event.zone_id,
            key: event.key,
            lrc: event.lrc,
          };
          if (observedNowPlayingIdentity !== undefined) {
            if (
              tracked !== null &&
              validEvent.zone_id === tracked.zoneId &&
              observedNowPlayingIdentity === tracked.nowPlayingIdentity
            ) {
              pendingEventsByZone.delete(validEvent.zone_id);
              acceptEvent(validEvent);
            } else {
              retainEvent(validEvent, observedNowPlayingIdentity);
            }
            return;
          }
          if (tracked === null || validEvent.zone_id !== tracked.zoneId) {
            retainEvent(validEvent);
            return;
          }
          const keyFingerprint =
            validEvent.key === null ? null : fingerprintKey(validEvent.key);
          if (
            keyFingerprint === null ||
            !keyTrackIdentities.has(keyFingerprint)
          ) {
            retainEvent(validEvent, null);
            return;
          }
          pendingEventsByZone.delete(validEvent.zone_id);
          acceptEvent(validEvent);
        },
        onDisconnect: loseConnection,
      });
    } catch {
      loseConnection();
    }
  };
  open();

  return {
    track: (next, knownNowPlaying = next === null ? [] : [next]) => {
      knownTrackIdentitiesByZone.clear();
      for (const known of knownNowPlaying) {
        knownTrackIdentitiesByZone.set(known.zoneId, known.nowPlayingIdentity);
      }
      const sameNowPlaying =
        tracked?.zoneId === next?.zoneId &&
        tracked?.nowPlayingIdentity === next?.nowPlayingIdentity;
      if (!sameNowPlaying) {
        clear();
        tracked = next;
      }
      if (next !== null) {
        const pendingEvent = pendingEventsByZone.get(next.zoneId);
        pendingEventsByZone.delete(next.zoneId);
        if (
          pendingEvent !== undefined &&
          (pendingEvent.nowPlayingIdentity === null ||
            pendingEvent.nowPlayingIdentity === next.nowPlayingIdentity)
        ) {
          acceptEvent(pendingEvent.event);
        }
      }
    },
    markVisible: () => {
      if (timelineVisible || acceptedKey === null) {
        return;
      }
      timelineVisible = true;
      try {
        connection?.reportViewed(acceptedKey);
      } catch {
        // View reporting is best-effort telemetry, never a presentation dependency.
      }
    },
    stop: () => {
      if (stopped) {
        return;
      }
      stopped = true;
      cancelReconnect?.();
      cancelReconnect = undefined;
      connection?.stop();
      connection = undefined;
      clear();
      keyTrackIdentities.clear();
      knownTrackIdentitiesByZone.clear();
      pendingEventsByZone.clear();
    },
  };
}

export function trackedNowPlaying(
  zone: LyricNowPlayingZone,
): TrackedNowPlaying | null {
  if (zone.state === "stopped" || zone.now_playing === undefined) {
    return null;
  }
  return {
    zoneId: zone.zone_id,
    nowPlayingIdentity: JSON.stringify([
      zone.now_playing.image_key ?? null,
      zone.now_playing.length ?? null,
      zone.now_playing.three_line ?? null,
    ]),
  };
}

function scheduleReconnectWithTimeout(reconnect: () => void): () => void {
  const timeout = setTimeout(reconnect, 10_000);
  return () => clearTimeout(timeout);
}

function fingerprintKey(key: string): string {
  return createHash("sha256").update(key, "utf8").digest("hex");
}

function sameTimeline(
  left: SynchronizedLyrics,
  right: SynchronizedLyrics | null,
): boolean {
  return (
    right !== null &&
    left.cues.length === right.cues.length &&
    left.cues.every(
      (cue, index) =>
        cue.atSeconds === right.cues[index]?.atSeconds &&
        cue.text === right.cues[index]?.text,
    )
  );
}
