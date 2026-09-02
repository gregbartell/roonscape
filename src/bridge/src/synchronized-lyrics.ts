import { hasUnpairedSurrogate } from "./unicode.js";

export const MAX_LYRIC_CUES = 256;
export const MAX_LYRIC_CUE_CODE_POINTS = 512;
export const MAX_LYRIC_TOTAL_CODE_POINTS = 16_384;
const MAX_LRC_BYTES = 64 * 1024;
const MAX_LYRIC_TIMESTAMP_SECONDS = 24 * 60 * 60;
const TIMED_LINE = /^\[(\d{2,}):([0-5]\d)\.(\d{2})\](.*)$/u;

export interface LyricCue {
  atSeconds: number;
  text: string;
}

export interface SynchronizedLyrics {
  cues: LyricCue[];
}

export function parseSynchronizedLyrics(
  lrc: string | null,
): SynchronizedLyrics | null {
  if (lrc === null || Buffer.byteLength(lrc, "utf8") > MAX_LRC_BYTES) {
    return null;
  }

  const cuesByTimestamp = new Map<number, LyricCue>();
  let totalCodePoints = 0;
  for (const line of lrc.split(/\r?\n/u)) {
    const match = TIMED_LINE.exec(line);
    if (match === null) {
      continue;
    }

    const [, minutesText, secondsText, centisecondsText, text] = match;
    if (
      minutesText === undefined ||
      secondsText === undefined ||
      centisecondsText === undefined ||
      text === undefined ||
      hasUnpairedSurrogate(text)
    ) {
      return null;
    }
    const textCodePoints = [...text].length;
    if (textCodePoints > MAX_LYRIC_CUE_CODE_POINTS) {
      return null;
    }

    const atSeconds =
      Number(minutesText) * 60 +
      Number(secondsText) +
      Number(centisecondsText) / 100;
    if (
      !Number.isSafeInteger(Number(minutesText)) ||
      atSeconds > MAX_LYRIC_TIMESTAMP_SECONDS
    ) {
      return null;
    }

    const previous = cuesByTimestamp.get(atSeconds);
    totalCodePoints -= previous === undefined ? 0 : [...previous.text].length;
    totalCodePoints += textCodePoints;
    if (totalCodePoints > MAX_LYRIC_TOTAL_CODE_POINTS) {
      return null;
    }
    cuesByTimestamp.set(atSeconds, { atSeconds, text });
    if (cuesByTimestamp.size > MAX_LYRIC_CUES) {
      return null;
    }
  }

  if (cuesByTimestamp.size === 0) {
    return null;
  }
  return {
    cues: [...cuesByTimestamp.values()].sort(
      (left, right) => left.atSeconds - right.atSeconds,
    ),
  };
}
