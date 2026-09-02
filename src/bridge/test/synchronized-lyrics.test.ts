import assert from "node:assert/strict";
import test from "node:test";

import {
  MAX_LYRIC_CUE_CODE_POINTS,
  MAX_LYRIC_CUES,
  MAX_LYRIC_TOTAL_CODE_POINTS,
  parseSynchronizedLyrics,
} from "../src/synchronized-lyrics.js";

test("parses, orders, and deduplicates synchronized LRC cues", () => {
  assert.deepEqual(
    parseSynchronizedLyrics(
      "[00:12.34]later\n[00:01.20]first\n[00:12.34]replacement\n[bad]ignored",
    ),
    {
      cues: [
        { atSeconds: 1.2, text: "first" },
        { atSeconds: 12.34, text: "replacement" },
      ],
    },
  );
});

test("preserves Unicode and intentional timed blank cues", () => {
  assert.deepEqual(parseSynchronizedLyrics("[00:01.00]月へ 🌙\n[00:04.50]"), {
    cues: [
      { atSeconds: 1, text: "月へ 🌙" },
      { atSeconds: 4.5, text: "" },
    ],
  });
});

test("treats null, malformed, invalid Unicode, and cue-free input as unavailable", () => {
  assert.equal(parseSynchronizedLyrics(null), null);
  assert.equal(parseSynchronizedLyrics("metadata only\n[1:02.00]wrong"), null);
  assert.equal(parseSynchronizedLyrics("[00:01.00]\ud800"), null);
});

test("rejects a timeline beyond any defensive bound", () => {
  const tooMany = Array.from(
    { length: MAX_LYRIC_CUES + 1 },
    (_, index) =>
      `[${String(Math.floor(index / 60)).padStart(2, "0")}:${String(index % 60).padStart(2, "0")}.00]cue`,
  ).join("\n");
  const oneLongCue = `[00:01.00]${"x".repeat(MAX_LYRIC_CUE_CODE_POINTS + 1)}`;
  const tooMuchText = Array.from(
    { length: MAX_LYRIC_CUES },
    (_, index) =>
      `[${String(Math.floor(index / 60)).padStart(2, "0")}:${String(index % 60).padStart(2, "0")}.00]${"x".repeat(Math.ceil(MAX_LYRIC_TOTAL_CODE_POINTS / MAX_LYRIC_CUES) + 1)}`,
  ).join("\n");

  assert.equal(parseSynchronizedLyrics(tooMany), null);
  assert.equal(parseSynchronizedLyrics(oneLongCue), null);
  assert.equal(parseSynchronizedLyrics(tooMuchText), null);
});
