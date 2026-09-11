import { readFile } from "node:fs/promises";
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
];
export const profiles = {
  default: { repeats: 3, warmupMs: 2000, measurementMs: 8000, intervalMs: 100 },
  thorough: {
    repeats: 5,
    warmupMs: 5000,
    measurementMs: 30000,
    intervalMs: 100,
  },
  smoke: { repeats: 1, warmupMs: 100, measurementMs: 400, intervalMs: 50 },
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
    const schedule = schedules[name].map(([atMs, snapshot]) => ({
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
      cycleMs: 8000,
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
