import { copyFile, mkdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { digest } from "./renderer-comparison-sources.mjs";

export const coverage = [
  "progress",
  "lyrics",
  "lyric-transitions",
  "replacements",
  "activity",
  "static",
  "reduced-animation",
  "fresh-content",
  "reused-content",
  "superseded-content",
];
export const profiles = {
  default: { repeats: 2, warmupMs: 1000, measurementMs: 8000, intervalMs: 100 },
  thorough: {
    repeats: 5,
    warmupMs: 5000,
    measurementMs: 30000,
    intervalMs: 100,
  },
  smoke: {
    repeats: 1,
    warmupMs: 100,
    measurementMs: 400,
    intervalMs: 50,
    drainMs: 200,
  },
};
const root = fileURLToPath(new URL("..", import.meta.url));
export async function workloads(selected = coverage) {
  if (
    !selected.length ||
    new Set(selected).size !== selected.length ||
    selected.some((name) => !coverage.includes(name))
  )
    throw new Error(`workloads must select from: ${coverage.join(", ")}`);
  const fixtures = {};
  for (const name of [
    "playing",
    "paused",
    "loading",
    "stopped",
    "light-artwork",
    "dark-teal",
    "lyrics-reel-lift-tour",
  ]) {
    fixtures[name] = JSON.parse(
      await readFile(
        path.join(root, `src/shared/fixtures/${name}.json`),
        "utf8",
      ),
    );
  }
  // A long, continuously available timeline exercises settled composition and adjacent handoffs.
  const lyrics = structuredClone(fixtures["lyrics-reel-lift-tour"]);
  lyrics.lyrics.cues = Array.from({ length: 180 }, (_, index) => ({
    atSeconds: index,
    text: `Synthetic lyric cue ${index + 1}`,
  }));
  lyrics.timing.position.seconds = 10;
  lyrics.timing.durationSeconds = 240;
  const schedules = {
    "fresh-content": [
      [0, fixtures.playing],
      [100, fixtures["light-artwork"], true],
      [1100, fixtures["dark-teal"], true],
      [2100, fixtures.playing, true],
    ],
    "reused-content": [
      [0, fixtures.playing],
      [100, fixtures["light-artwork"]],
      [500, fixtures.playing],
      [900, fixtures["light-artwork"]],
      [1300, fixtures.playing],
    ],
    "superseded-content": [
      [0, fixtures.playing],
      [100, fixtures["light-artwork"], true],
      [110, fixtures["dark-teal"], true],
      [120, fixtures.playing, true],
      [1100, fixtures["dark-teal"], true],
    ],
    progress: [[0, fixtures.playing]],
    lyrics: [[0, lyrics]],
    "lyric-transitions": [
      [0, fixtures.playing],
      [1000, fixtures["lyrics-reel-lift-tour"]],
      [6000, fixtures.playing],
    ],
    replacements: [
      [0, fixtures.playing],
      [1000, fixtures["light-artwork"]],
      [1100, fixtures.paused],
      [1200, fixtures["light-artwork"]],
      [4000, fixtures.playing],
    ],
    activity: [
      [0, fixtures.loading],
      [2000, fixtures.stopped],
      [6000, fixtures.playing],
    ],
    static: [[0, fixtures.playing]],
    "reduced-animation": [
      [0, fixtures.playing],
      [1000, lyrics],
      [3000, fixtures["light-artwork"]],
      [3100, fixtures.paused],
      [6000, fixtures.loading],
    ],
  };
  const result = [];
  for (const name of selected) {
    const schedule = schedules[name].map(([atMs, snapshot, fresh = false]) => ({
      fresh,
      atMs,
      snapshot: structuredClone(snapshot),
    }));
    const artwork = {};
    for (const entry of schedule) {
      const source = entry.snapshot.artwork?.path;
      if (source)
        artwork[source] = digest(await readFile(path.join(root, source)));
    }
    result.push({
      name,
      cycleMs:
        name === "fresh-content"
          ? 3200
          : name.endsWith("-content")
            ? 1600
            : 8000,
      diagnostic: name.endsWith("-content"),
      warmupSchedule: name.endsWith("-content")
        ? [{ atMs: 0, snapshot: structuredClone(fixtures.playing) }]
        : schedule,
      static: name === "static",
      reducedAnimation: name === "reduced-animation",
      schedule,
      artwork,
      digest: digest(JSON.stringify({ schedule, artwork })),
    });
  }
  return result;
}
export const workloadRoot = root;

// Materialize all synthetic inputs before any clean measurement. New paths and
// artwork revisions make fresh publications miss the Renderer's prepared cache;
// the bytes themselves are identical for both sides, with OS cache effects left
// explicit rather than claiming cold physical storage.
export async function prepareWorkloadContent(selected, profile, output) {
  await mkdir(path.join(output, "content"));
  for (const workload of selected) {
    workload.phases = {};
    for (const [phase, durationMs] of [
      ["warmup", profile.warmupMs],
      ["measurement", profile.measurementMs],
    ]) {
      const source =
        phase === "warmup" ? workload.warmupSchedule : workload.schedule;
      const entries = [];
      for (let cycle = 0; cycle * workload.cycleMs < durationMs; cycle++) {
        for (const [index, entry] of source.entries()) {
          const atMs = cycle * workload.cycleMs + entry.atMs;
          if (atMs >= durationMs) continue;
          const snapshot = structuredClone(entry.snapshot);
          let asset;
          if (snapshot.artwork?.path) {
            const sourcePath = snapshot.artwork.path;
            const relativePath = entry.fresh
              ? `${workload.name}/${phase}-${cycle}-${index}${path.extname(sourcePath)}`
              : path.basename(sourcePath);
            asset = {
              path: path.join(output, "content", relativePath),
              relativePath,
              sha256: workload.artwork[sourcePath],
              fresh: entry.fresh ?? false,
            };
            await mkdir(path.dirname(asset.path), { recursive: true });
            await copyFile(path.join(root, sourcePath), asset.path);
            snapshot.artwork.path = asset.path;
            if (entry.fresh) {
              snapshot.artwork.revision = 100 + cycle * source.length + index;
              snapshot.nowPlaying.title += ` ${cycle}-${index}`;
            }
          }
          const contentId = digest(
            JSON.stringify({
              nowPlaying: snapshot.nowPlaying,
              lyrics: snapshot.lyrics,
              artwork: asset
                ? {
                    path: asset.relativePath,
                    revision: snapshot.artwork.revision,
                    sha256: asset.sha256,
                  }
                : null,
            }),
          );
          entries.push({ atMs, snapshot, asset, contentId });
        }
      }
      workload.phases[phase] = { durationMs, entries };
    }
  }
}
