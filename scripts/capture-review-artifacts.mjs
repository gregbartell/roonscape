import {
  copyFile,
  mkdtemp,
  mkdir,
  readdir,
  rename,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";

import { runMonitoredProcess } from "./process-harness.mjs";

export function reviewImageFormat(lossless) {
  return lossless
    ? {
        extension: "png",
        description: "lossless PNG",
        encoderArguments: ["-c:v", "png", "-compression_level", "1"],
      }
    : {
        extension: "jpg",
        description: "high-quality lossy JPEG (4:4:4)",
        encoderArguments: [
          "-c:v",
          "mjpeg",
          "-q:v",
          "2",
          "-pix_fmt",
          "yuvj444p",
        ],
      };
}

const thumbnailFilter =
  "scale=384:216:force_original_aspect_ratio=decrease,pad=384:216:(ow-iw)/2:(oh-ih)/2:black";

const ffmpegArguments = [
  "-hide_banner",
  "-loglevel",
  "error",
  "-nostdin",
  "-filter_threads",
  "2",
  "-threads",
  "2",
];

// Captures have a constant frame rate. Like accurate input seeking, a request
// between ticks selects the following tick. Requests may share a tick or arrive
// out of order; encode each distinct tick once and retain the requested names.
export async function extractReviewFrames(
  videoPath,
  frames,
  { framesPerSecond = 20, lossless = false, thumbnailDirectory, signal } = {},
) {
  if (frames.length === 0) return;
  const byTick = new Map();
  for (const { atSeconds, outputPath } of frames) {
    if (!Number.isFinite(atSeconds) || atSeconds < 0) {
      throw new Error("review frame time must be finite and nonnegative");
    }
    const tick = Math.ceil(atSeconds * framesPerSecond - 1e-7);
    const paths = byTick.get(tick) ?? [];
    paths.push(outputPath);
    byTick.set(tick, paths);
  }
  const ticks = [...byTick.keys()].sort((left, right) => left - right);
  const firstTick = ticks[0];
  const format = reviewImageFormat(lossless);
  const work = await mkdtemp(
    path.join(path.dirname(frames[0].outputPath), ".frames."),
  );
  try {
    if (thumbnailDirectory)
      await mkdir(thumbnailDirectory, { recursive: true });
    const selection = `select='${frameSelection(ticks.map((tick) => tick - firstTick))}'`;
    await runMonitoredProcess(
      "ffmpeg",
      [
        ...ffmpegArguments,
        "-ss",
        (firstTick / framesPerSecond).toFixed(6),
        "-i",
        videoPath,
        ...(thumbnailDirectory
          ? [
              "-filter_complex_threads",
              "2",
              "-filter_complex",
              `${selection},split[full][small];[small]${thumbnailFilter}[thumbnail]`,
              "-map",
              "[full]",
            ]
          : ["-vf", selection]),
        "-vsync",
        "vfr",
        "-frames:v",
        String(ticks.length),
        ...format.encoderArguments,
        "-threads",
        "2",
        "-start_number",
        "0",
        "-y",
        path.join(work, `full-%06d.${format.extension}`),
        ...(thumbnailDirectory
          ? [
              "-map",
              "[thumbnail]",
              "-vsync",
              "vfr",
              "-frames:v",
              String(ticks.length),
              ...reviewImageFormat(true).encoderArguments,
              "-threads",
              "2",
              "-start_number",
              "0",
              "-y",
              path.join(work, "thumbnail-%06d.png"),
            ]
          : []),
      ],
      {
        description: "review frame extraction",
        timeoutMilliseconds: Math.ceil(ticks.length / 100) * 120_000,
        signal,
      },
    );
    const files = await readdir(work);
    if (files.length !== ticks.length * (thumbnailDirectory ? 2 : 1)) {
      throw new Error(
        "recording does not contain every requested review frame",
      );
    }
    for (const [index, tick] of ticks.entries()) {
      signal?.throwIfAborted();
      const [first, ...duplicates] = byTick.get(tick);
      const number = String(index).padStart(6, "0");
      await rename(
        path.join(work, `full-${number}.${format.extension}`),
        first,
      );
      for (const outputPath of duplicates) await copyFile(first, outputPath);
      if (thumbnailDirectory) {
        const thumbnail = path.join(
          thumbnailDirectory,
          `${path.basename(first)}.png`,
        );
        await rename(path.join(work, `thumbnail-${number}.png`), thumbnail);
        for (const outputPath of duplicates)
          await copyFile(
            thumbnail,
            path.join(thumbnailDirectory, `${path.basename(outputPath)}.png`),
          );
      }
    }
  } finally {
    await rm(work, { force: true, recursive: true });
  }
}

function frameSelection(ticks) {
  const ranges = [];
  for (const tick of ticks) {
    const previous = ranges.at(-1);
    if (previous && previous.last + 1 === tick) previous.last = tick;
    else ranges.push({ first: tick, last: tick });
  }
  const terms = ranges.map(({ first, last }) =>
    first === last ? `eq(n,${first})` : `between(n,${first},${last})`,
  );
  // A left-associative sum reaches FFmpeg's expression-depth limit on long
  // captures. Balance it even when sparse requests cannot collapse into ranges.
  return sumExpressions(terms);
}

function sumExpressions(terms) {
  if (terms.length === 1) return terms[0];
  const middle = Math.floor(terms.length / 2);
  return `(${sumExpressions(terms.slice(0, middle))}+${sumExpressions(terms.slice(middle))})`;
}

export async function createFullRateReviewSheets(
  videoPath,
  reviewDirectory,
  { durationSeconds, framesPerSecond, lossless = false },
  signal,
) {
  const framesPerPage = 100;
  const columns = 10;
  const frameCount = Math.max(1, Math.round(durationSeconds * framesPerSecond));
  const pageCount = Math.ceil(frameCount / framesPerPage);
  const format = reviewImageFormat(lossless);
  await runMonitoredProcess(
    "ffmpeg",
    [
      ...ffmpegArguments,
      "-i",
      videoPath,
      "-vf",
      `trim=end_frame=${frameCount},scale=192:108,drawtext=font=monospace:text='%{eif\\:mod(n,100)\\:d}':fontcolor=white:fontsize=16:box=1:boxcolor=black@0.78:boxborderw=3:x=4:y=h-th-4,tile=10x10:nb_frames=100:padding=1:margin=0:color=black`,
      "-vsync",
      "vfr",
      "-frames:v",
      String(pageCount),
      ...format.encoderArguments,
      "-threads",
      "2",
      "-start_number",
      "1",
      "-y",
      path.join(reviewDirectory, `full-rate-page-%03d.${format.extension}`),
    ],
    {
      description: "full-rate review sheets",
      timeoutMilliseconds: pageCount * 120_000,
      signal,
    },
  );
  const pages = [];
  for (
    let firstFrame = 0;
    firstFrame < frameCount;
    firstFrame += framesPerPage
  ) {
    const page = firstFrame / framesPerPage + 1;
    pages.push({
      file: `full-rate-page-${String(page).padStart(3, "0")}.${format.extension}`,
      firstFrame,
      count: Math.min(framesPerPage, frameCount - firstFrame),
      columns,
      startSeconds: Math.round((firstFrame / framesPerSecond) * 1000) / 1000,
    });
  }
  await writeFile(
    path.join(reviewDirectory, "review-index.json"),
    `${JSON.stringify({ framesPerSecond, pages }, null, 2)}\n`,
  );
  return pages.map(({ file }) => path.join(reviewDirectory, file));
}

export async function createReviewSheets(
  thumbnailPaths,
  outputPaths,
  {
    lossless = false,
    labels,
    columns = 5,
    framesPerPage = thumbnailPaths.length,
    signal,
  } = {},
) {
  if (
    thumbnailPaths.length === 0 ||
    !Number.isSafeInteger(framesPerPage) ||
    framesPerPage < 1 ||
    outputPaths.length !== Math.ceil(thumbnailPaths.length / framesPerPage) ||
    (labels && labels.length !== thumbnailPaths.length)
  ) {
    throw new Error(
      "contact sheet requires images and a label for every frame",
    );
  }
  const work = await mkdtemp(
    path.join(path.dirname(outputPaths[0]), ".overview."),
  );
  const format = reviewImageFormat(lossless);
  try {
    for (const [index, inputPath] of thumbnailPaths.entries()) {
      await symlink(
        path.resolve(inputPath),
        path.join(work, `${String(index).padStart(6, "0")}.png`),
      );
    }
    const render = async (annotate) => {
      const filters = [];
      if (annotate) {
        for (const [index, label] of labels.entries()) {
          filters.push(
            `metadata=mode=add:key=capture_label:value='${label}':enable='eq(n,${index})'`,
          );
        }
        // Load the font once; each frame carries its own timestamp metadata.
        filters.push(
          "drawtext=font=monospace:text='%{metadata\\:capture_label}':fontcolor=white:fontsize=22:box=1:boxcolor=black@0.78:boxborderw=6:x=8:y=h-th-8",
        );
      }
      filters.push(
        `tile=${columns}x${Math.ceil(framesPerPage / columns)}:nb_frames=${framesPerPage}:padding=2:margin=0:color=black`,
      );
      // A two-minute capture can exceed the OS argument limit with inline labels.
      const filterPath = path.join(work, "sheet-filter.txt");
      await writeFile(filterPath, filters.join(","));
      await runMonitoredProcess(
        "ffmpeg",
        [
          ...ffmpegArguments,
          "-framerate",
          "1",
          "-start_number",
          "0",
          "-i",
          path.join(work, "%06d.png"),
          "-filter_script:v",
          filterPath,
          "-vsync",
          "vfr",
          "-frames:v",
          String(outputPaths.length),
          ...format.encoderArguments,
          "-threads",
          "2",
          "-start_number",
          "0",
          "-y",
          path.join(work, `page-%06d.${format.extension}`),
        ],
        {
          description: "review overview",
          timeoutMilliseconds: outputPaths.length * 120_000,
          signal,
        },
      );
      for (const [index, outputPath] of outputPaths.entries()) {
        signal?.throwIfAborted();
        await rename(
          path.join(
            work,
            `page-${String(index).padStart(6, "0")}.${format.extension}`,
          ),
          outputPath,
        );
      }
    };
    try {
      await render(labels !== undefined);
      return { annotated: labels !== undefined };
    } catch (error) {
      signal?.throwIfAborted();
      if (labels === undefined) throw error;
      await render(false);
      return { annotated: false };
    }
  } finally {
    await rm(work, { force: true, recursive: true });
  }
}
