#!/usr/bin/env node

import { spawn } from "node:child_process";
import { constants as fsConstants } from "node:fs";
import {
  access,
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { homedir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcess,
} from "../../../../scripts/process-harness.mjs";

const repositoryRoot = fileURLToPath(new URL("../../../../", import.meta.url));
const scratchRoot = "/var/tmp/codex/roonscape";
const framesPerSecond = 20;
const frameSeconds = 1 / framesPerSecond;
const maximumRecordingSeconds = 120;
const defaultResolution = { width: 1280, height: 720 };
const maximumDimension = 32_767;

const usage = `Usage:
  live-capture-session.mjs record --event DESCRIPTION [--resolution WIDTHxHEIGHT] [--fullscreen] [--duration SECONDS] [--config PATH]
  live-capture-session.mjs snapshot --session DIRECTORY
  live-capture-session.mjs stop --session DIRECTORY
  live-capture-session.mjs review --session DIRECTORY
  live-capture-session.mjs inspect --session DIRECTORY --at SECONDS
  live-capture-session.mjs publish --session DIRECTORY --selection FILE
  live-capture-session.mjs retract --session DIRECTORY
  live-capture-session.mjs finalize --session DIRECTORY
  live-capture-session.mjs discard --session DIRECTORY`;

export function parseResolution(value) {
  const match = /^(\d+)x(\d+)$/.exec(value);
  if (match === null) {
    throw new Error("--resolution must use WIDTHxHEIGHT");
  }
  const width = Number(match[1]);
  const height = Number(match[2]);
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width < 1280 ||
    height < 720
  ) {
    throw new Error("--resolution must be at least 1280x720");
  }
  if (width <= height) {
    throw new Error("--resolution must be landscape");
  }
  if (width > maximumDimension || height > maximumDimension) {
    throw new Error(
      `--resolution exceeds the supported maximum of ${maximumDimension}`,
    );
  }
  return { width, height };
}

export function buildLiveEnvironment(
  environment,
  display,
  resolution,
  fullscreen,
) {
  const liveEnvironment = {
    ...environment,
    DISPLAY: display,
    GDK_BACKEND: "x11",
    NO_AT_BRIDGE: "1",
  };
  for (const name of [
    "ROONSCAPE_CAPTURE_CONTROL",
    "ROONSCAPE_CAPTURE_TYPOGRAPHY",
    "ROONSCAPE_FIXTURE",
    "ROONSCAPE_FIXTURE_AUTO_CLOSE_MS",
    "ROONSCAPE_FIXTURE_CONTROL",
    "ROONSCAPE_SOCKET",
    "ROONSCAPE_STATIC_FIXTURE",
  ]) {
    delete liveEnvironment[name];
  }
  if (fullscreen) {
    delete liveEnvironment.ROONSCAPE_CAPTURE_VIEWPORT;
    delete liveEnvironment.ROONSCAPE_WINDOWED;
  } else {
    liveEnvironment.ROONSCAPE_CAPTURE_VIEWPORT = resolutionText(resolution);
    liveEnvironment.ROONSCAPE_WINDOWED = "1";
  }
  return liveEnvironment;
}

export function formatRelativeTimestamp(seconds, prefix = "T") {
  if (!Number.isFinite(seconds)) {
    throw new Error("timestamp must be finite");
  }
  const sign = seconds < 0 ? "−" : "+";
  const absolute = Math.abs(seconds);
  const whole = Math.floor(absolute);
  const fraction = Math.round((absolute - whole) * 100);
  const normalizedWhole = fraction === 100 ? whole + 1 : whole;
  const normalizedFraction = fraction === 100 ? 0 : fraction;
  return `${prefix}${sign}${String(normalizedWhole).padStart(3, "0")}.${String(normalizedFraction).padStart(2, "0")}s`;
}

export function validateSelection(selection, state) {
  if (!isPlainObject(selection)) {
    throw new Error("selection must be a JSON object");
  }
  requiredNonemptyString(selection.title, "selection title");
  requiredNonemptyString(selection.summary, "selection summary");
  if (typeof selection.complete !== "boolean") {
    throw new Error("selection complete must be true or false");
  }
  if (!Array.isArray(selection.frames)) {
    throw new Error("selection frames must be an array");
  }
  if (selection.complete && selection.frames.length < 2) {
    throw new Error(
      "a complete selection requires pre-event and concluding frames",
    );
  }
  if (!selection.complete) {
    requiredNonemptyString(
      selection.incompleteReason,
      "incomplete selection reason",
    );
  }
  if (selection.preserveDiagnostics === true && selection.complete) {
    throw new Error(
      "diagnostics may be preserved only for an incomplete session",
    );
  }

  let previous = -1;
  const names = new Set();
  for (const [index, frame] of selection.frames.entries()) {
    if (!isPlainObject(frame)) {
      throw new Error(`selection frame ${index} must be an object`);
    }
    if (!Number.isFinite(frame.at) || frame.at < 0) {
      throw new Error(
        `selection frame ${index} at must be nonnegative seconds`,
      );
    }
    if (frame.at <= previous) {
      throw new Error("selection frames must use strictly increasing times");
    }
    if (frame.at > state.durationSeconds + frameSeconds) {
      throw new Error(`selection frame ${index} is beyond the recording`);
    }
    previous = frame.at;
    const name = semanticSlug(frame.name);
    if (names.has(name)) {
      throw new Error(`duplicate selection frame name: ${name}`);
    }
    names.add(name);
    requiredNonemptyString(
      frame.observation,
      `selection frame ${index} observation`,
    );
  }

  if (selection.acceptance !== undefined) {
    if (!isPlainObject(selection.acceptance)) {
      throw new Error("selection acceptance must be an object");
    }
    requiredNonemptyString(
      selection.acceptance.criteria,
      "acceptance criteria",
    );
    if (
      !["pass", "fail", "inconclusive"].includes(selection.acceptance.verdict)
    ) {
      throw new Error("acceptance verdict must be pass, fail, or inconclusive");
    }
    requiredNonemptyString(
      selection.acceptance.rationale,
      "acceptance rationale",
    );
  }
  return selection;
}

export function renderReadme(selection, state, frames, annotationWarning) {
  const status = selection.complete ? "" : " — Incomplete";
  const mode = state.fullscreen ? "fullscreen" : "windowed";
  const origin = frames[0]?.at;
  const lines = [
    `# ${selection.title}${status}`,
    "",
    `Captured from RoonScape Live Mode in ${mode} mode at ${resolutionText(state.resolution)} on ${state.date}.`,
  ];
  if (origin !== undefined) {
    lines.push(
      "Times below are relative to the final stable frame immediately before the event.",
      "",
      "| File | Time | Observed presentation state |",
      "| --- | ---: | --- |",
      ...frames.map(
        (frame) =>
          `| \`${frame.fileName}\` | ${formatRelativeTimestamp(frame.at - origin)} | ${tableText(frame.observation)} |`,
      ),
    );
  }
  if (!selection.complete) {
    lines.push("", `**Incomplete:** ${selection.incompleteReason}`);
  }
  if (annotationWarning !== undefined) {
    lines.push("", `> ${annotationWarning}`);
  }
  lines.push("", selection.summary);
  if (selection.acceptance !== undefined) {
    lines.push(
      "",
      "## Acceptance",
      "",
      `- **Criterion:** ${selection.acceptance.criteria}`,
      `- **Verdict:** ${selection.acceptance.verdict}`,
      `- **Rationale:** ${selection.acceptance.rationale}`,
    );
  }
  return `${lines.join("\n")}\n`;
}

async function main(arguments_) {
  const [command, ...rest] = arguments_;
  if (command === undefined || command === "--help" || command === "-h") {
    process.stdout.write(`${usage}\n`);
    return;
  }
  switch (command) {
    case "record":
      await recordSession(parseRecordOptions(rest));
      return;
    case "snapshot":
      await snapshotSession(requiredOption(rest, "--session"));
      return;
    case "stop":
      await requestStop(requiredOption(rest, "--session"));
      return;
    case "review":
      await reviewSession(requiredOption(rest, "--session"));
      return;
    case "inspect": {
      const options = namedOptions(rest, ["--session", "--at"]);
      await inspectRecordedFrame(options["--session"], options["--at"]);
      return;
    }
    case "publish":
      {
        const options = namedOptions(rest, ["--session", "--selection"]);
        await publishSession(options["--session"], options["--selection"]);
      }
      return;
    case "retract":
      await retractPublication(requiredOption(rest, "--session"));
      return;
    case "finalize":
      await finalizeSession(requiredOption(rest, "--session"));
      return;
    case "discard":
      await discardSession(requiredOption(rest, "--session"));
      return;
    default:
      throw new Error(`unknown command: ${command}\n${usage}`);
  }
}

export function parseRecordOptions(arguments_) {
  const options = {
    event: undefined,
    resolution: defaultResolution,
    fullscreen: false,
    durationSeconds: undefined,
    configurationFile: undefined,
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    switch (argument) {
      case "--event":
        options.event = optionValue(arguments_, ++index, argument);
        break;
      case "--resolution":
        options.resolution = parseResolution(
          optionValue(arguments_, ++index, argument),
        );
        break;
      case "--fullscreen":
        options.fullscreen = true;
        break;
      case "--duration": {
        const value = Number(optionValue(arguments_, ++index, argument));
        if (
          !Number.isFinite(value) ||
          value <= 0 ||
          value > maximumRecordingSeconds
        ) {
          throw new Error(
            `--duration must be greater than zero and at most ${maximumRecordingSeconds} seconds`,
          );
        }
        options.durationSeconds = value;
        break;
      }
      case "--config":
        options.configurationFile = path.resolve(
          optionValue(arguments_, ++index, argument),
        );
        break;
      default:
        throw new Error(`unknown record option: ${argument}`);
    }
  }
  requiredNonemptyString(options.event, "--event");
  return options;
}

async function recordSession(options) {
  await preflight(options);
  await mkdir(scratchRoot, { recursive: true });
  const sessionDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const statePath = path.join(sessionDirectory, "session.json");
  const rawVideoPath = path.join(sessionDirectory, "capture.mkv");
  const stopRequestPath = path.join(sessionDirectory, "stop-requested");
  let state = {
    version: 1,
    status: "preparing",
    event: options.event,
    eventSlug: semanticSlug(options.event),
    date: localDate(),
    resolution: options.resolution,
    fullscreen: options.fullscreen,
    framesPerSecond,
    createdAt: new Date().toISOString(),
  };
  await writeJson(statePath, state);
  emit({ type: "session", path: sessionDirectory });

  let xvfb;
  let roonscape;
  let recorder;
  let failure;
  const cleanupErrors = [];
  try {
    await runMonitoredProcess("npm", ["run", "build"], {
      cwd: repositoryRoot,
      description: "RoonScape Bridge build",
      timeoutMilliseconds: 600_000,
    });
    await runMonitoredProcess(
      "cargo",
      ["build", "--locked", "--release", "--package", "roonscape-renderer"],
      {
        cwd: repositoryRoot,
        description: "RoonScape Renderer build",
        timeoutMilliseconds: 600_000,
      },
    );

    const displaySession = await startXvfbDisplay({
      ...options.resolution,
      cwd: repositoryRoot,
      description: "Live Capture Session display",
    });
    xvfb = displaySession.xvfb;
    const environment = buildLiveEnvironment(
      process.env,
      displaySession.display,
      options.resolution,
      options.fullscreen,
    );
    const launcherArguments = [];
    if (options.configurationFile) {
      launcherArguments.push("--config", options.configurationFile);
    }
    roonscape = startMonitoredProcess(
      path.join(repositoryRoot, "src/launcher/roonscape"),
      launcherArguments,
      { cwd: repositoryRoot, environment },
    );
    await roonscape.spawned;
    await waitForWindow(roonscape, environment, options.resolution);
    const recordingLimitSeconds =
      options.durationSeconds ?? maximumRecordingSeconds;
    const recorderStartedAt = Date.now();
    recorder = startRecorder(
      displaySession.display,
      options.resolution,
      rawVideoPath,
      environment,
      recordingLimitSeconds,
    );
    await recorder.spawned;
    await delay(250);
    assertProcessRunning(recorder, "Live Capture Session recorder");

    state = {
      ...state,
      status: "recording",
      display: displaySession.display,
      recordingStartedAt: new Date(recorderStartedAt).toISOString(),
    };
    await writeJson(statePath, state);
    emit({
      type: "runtime-ready",
      path: sessionDirectory,
      mode: options.fullscreen ? "fullscreen" : "windowed",
      resolution: resolutionText(options.resolution),
    });

    const outcome = await waitForRecordingOutcome({
      roonscape,
      recorder,
      xvfb,
      stopRequestPath,
      recordingDeadlineMilliseconds:
        recorderStartedAt + recordingLimitSeconds * 1000,
      limitOutcome:
        options.durationSeconds === undefined ? "safety-timeout" : "duration",
    });
    await stopProcess(recorder, {
      description: "Live Capture Session recorder",
    });
    recorder = undefined;
    const video = await probeVideo(rawVideoPath);
    state = {
      ...state,
      status: "processing",
      stopReason: outcome,
      durationSeconds: video.durationSeconds,
      recordingStoppedAt: new Date().toISOString(),
    };
    await writeJson(statePath, state);
    const candidates = await extractCandidates(sessionDirectory, state);
    state = { ...state, status: "recorded", candidateCount: candidates.length };
    delete state.display;
    await writeJson(statePath, state);
    emit({
      type: "recorded",
      path: sessionDirectory,
      stopReason: outcome,
      durationSeconds: state.durationSeconds,
      candidates: candidates.length,
    });
  } catch (error) {
    failure = error;
    state = { ...state, status: "failed", failure: errorMessage(error) };
    delete state.display;
    await writeJson(statePath, state).catch(() => {});
    try {
      await writeDiagnostics(sessionDirectory, {
        roonscape,
        recorder,
        xvfb,
        error,
      });
    } catch (diagnosticError) {
      cleanupErrors.push(diagnosticError);
    }
  } finally {
    for (const [child, description] of [
      [recorder, "Live Capture Session recorder"],
      [roonscape, "RoonScape Live Mode"],
      [xvfb, "Live Capture Session display"],
    ]) {
      try {
        await stopProcess(child, { description });
      } catch (error) {
        cleanupErrors.push(error);
      }
    }
  }
  if (failure !== undefined) {
    if (cleanupErrors.length > 0) {
      throw new AggregateError(
        [failure, ...cleanupErrors],
        errorMessage(failure),
        { cause: failure },
      );
    }
    throw failure;
  }
  if (cleanupErrors.length > 0) {
    throw new AggregateError(
      cleanupErrors,
      "Live Capture Session cleanup failed",
      { cause: cleanupErrors[0] },
    );
  }
}

async function preflight(options) {
  const missing = [];
  for (const command of ["Xvfb", "ffmpeg", "ffprobe", "xwininfo"]) {
    if (!(await executableOnPath(command))) {
      missing.push(command);
    }
  }
  if (missing.length > 0) {
    throw new Error(
      `required executable is unavailable: ${missing.join(", ")}`,
    );
  }
  const configurationFile =
    options.configurationFile ??
    path.join(
      process.env.XDG_CONFIG_HOME ?? path.join(homedir(), ".config"),
      "roonscape",
      "display.json",
    );
  const configuration = await readJsonFile(
    configurationFile,
    "Display Configuration",
  );
  if (
    !isPlainObject(configuration) ||
    typeof configuration.trackedOutputId !== "string" ||
    configuration.trackedOutputId.length === 0
  ) {
    throw new Error(
      `Display Configuration is missing or invalid: ${configurationFile}`,
    );
  }
  const authorizationFile = process.env.ROONSCAPE_AUTHORIZATION_FILE
    ? path.resolve(process.env.ROONSCAPE_AUTHORIZATION_FILE)
    : path.join(
        process.env.XDG_STATE_HOME ?? path.join(homedir(), ".local", "state"),
        "roonscape",
        "authorization.json",
      );
  const authorization = await readJsonFile(
    authorizationFile,
    "Roon Authorization",
  );
  if (
    !isPlainObject(authorization) ||
    Object.keys(authorization).length === 0
  ) {
    throw new Error(
      `Roon Authorization is missing or empty: ${authorizationFile}`,
    );
  }
  await rejectRunningRoonScape(process.env);
}

async function rejectRunningRoonScape(environment) {
  const userId = process.getuid?.();
  if (userId === undefined) {
    throw new Error("Live Capture Session requires a Linux user identity");
  }
  const runtimeRoot = environment.XDG_RUNTIME_DIR ?? `/run/user/${userId}`;
  const ownerPath = path.join(
    runtimeRoot,
    "roonscape",
    "owner",
    "session.json",
  );
  let owner;
  try {
    owner = JSON.parse(await readFile(ownerPath, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT") {
      return;
    }
    throw new Error(`cannot verify existing RoonScape runtime: ${ownerPath}`, {
      cause: error,
    });
  }
  if (
    !isPlainObject(owner) ||
    !Number.isSafeInteger(owner.processId) ||
    typeof owner.processStartTimeTicks !== "string"
  ) {
    throw new Error(
      `cannot verify existing RoonScape runtime ownership: ${ownerPath}`,
    );
  }
  try {
    const processStat = await readFile(`/proc/${owner.processId}/stat`, "utf8");
    if (linuxProcessStartTime(processStat) === owner.processStartTimeTicks) {
      throw new Error(
        `RoonScape is already running for this user (process ${owner.processId}); close it before capture`,
      );
    }
  } catch (error) {
    if (error?.code !== "ENOENT") {
      throw error;
    }
  }
}

function linuxProcessStartTime(processStat) {
  const commandEnd = processStat.lastIndexOf(")");
  if (commandEnd < 0) {
    throw new Error("cannot parse existing RoonScape process identity");
  }
  const fieldsFromState = processStat
    .slice(commandEnd + 2)
    .trim()
    .split(/\s+/);
  const startTime = fieldsFromState[19];
  if (startTime === undefined) {
    throw new Error("cannot parse existing RoonScape process start time");
  }
  return startTime;
}

async function waitForWindow(roonscape, environment, resolution) {
  let lastError;
  for (let attempt = 0; attempt < 300; attempt += 1) {
    assertProcessRunning(roonscape, "RoonScape Live Mode");
    try {
      const output = await runMonitoredProcess(
        "xwininfo",
        ["-name", "RoonScape", "-int"],
        {
          cwd: repositoryRoot,
          environment,
          description: "RoonScape Live Mode window query",
          timeoutMilliseconds: 1_000,
        },
      );
      const width = Number(output.match(/^\s*Width:\s+(\d+)/m)?.[1]);
      const height = Number(output.match(/^\s*Height:\s+(\d+)/m)?.[1]);
      if (width === resolution.width && height === resolution.height) {
        return;
      }
      lastError = new Error(
        `RoonScape window is ${width}x${height}; expected ${resolutionText(resolution)}`,
      );
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw new Error("timed out waiting for the RoonScape Live Mode window", {
    cause: lastError,
  });
}

function startRecorder(
  display,
  resolution,
  rawVideoPath,
  environment,
  recordingLimitSeconds,
) {
  return startMonitoredProcess(
    "ffmpeg",
    [
      "-hide_banner",
      "-loglevel",
      "warning",
      "-nostdin",
      "-f",
      "x11grab",
      "-draw_mouse",
      "0",
      "-framerate",
      String(framesPerSecond),
      "-video_size",
      resolutionText(resolution),
      "-i",
      `${display}.0+0,0`,
      "-vf",
      "setpts=PTS-STARTPTS",
      "-c:v",
      "ffv1",
      "-level",
      "3",
      "-g",
      "1",
      "-t",
      String(recordingLimitSeconds),
      "-y",
      rawVideoPath,
    ],
    { cwd: repositoryRoot, environment },
  );
}

async function waitForRecordingOutcome({
  roonscape,
  recorder,
  xvfb,
  stopRequestPath,
  recordingDeadlineMilliseconds,
  limitOutcome,
}) {
  for (;;) {
    assertProcessRunning(roonscape, "RoonScape Live Mode");
    assertProcessRunning(xvfb, "Live Capture Session display");
    try {
      await access(stopRequestPath);
      return "requested";
    } catch (error) {
      if (error?.code !== "ENOENT") {
        throw error;
      }
    }
    if (Date.now() >= recordingDeadlineMilliseconds) {
      return limitOutcome;
    }
    assertProcessRunning(recorder, "Live Capture Session recorder");
    await delay(100);
  }
}

async function probeVideo(videoPath) {
  const output = await runMonitoredProcess(
    "ffprobe",
    [
      "-v",
      "error",
      "-show_entries",
      "format=duration:stream=width,height",
      "-of",
      "json",
      videoPath,
    ],
    {
      cwd: repositoryRoot,
      description: "Live Capture Session recording validation",
      timeoutMilliseconds: 30_000,
    },
  );
  const report = JSON.parse(output);
  const durationSeconds = Number(report.format?.duration);
  const stream = report.streams?.[0];
  if (
    !Number.isFinite(durationSeconds) ||
    durationSeconds <= 0 ||
    !Number.isSafeInteger(stream?.width) ||
    !Number.isSafeInteger(stream?.height)
  ) {
    throw new Error("Live Capture Session recording is invalid");
  }
  return { durationSeconds, width: stream.width, height: stream.height };
}

export async function extractCandidates(sessionDirectory, state) {
  const candidateDirectory = path.join(sessionDirectory, "candidates");
  await mkdir(candidateDirectory);
  const timestamps = [];
  await runStreamingProcess(
    "ffmpeg",
    [
      "-hide_banner",
      "-loglevel",
      "info",
      "-i",
      path.join(sessionDirectory, "capture.mkv"),
      "-vf",
      "mpdecimate,showinfo",
      "-fps_mode",
      "vfr",
      "-start_number",
      "0",
      "-y",
      path.join(candidateDirectory, "%06d.png"),
    ],
    (line) => {
      const value = line.match(/showinfo.*pts_time:\s*([0-9.]+)/)?.[1];
      if (value !== undefined) {
        timestamps.push(Number(value));
      }
    },
  );
  const names = (await readdir(candidateDirectory))
    .filter((name) => /^\d{6}\.png$/.test(name))
    .sort();
  if (names.length !== timestamps.length || names.length === 0) {
    throw new Error(
      "candidate extraction did not produce a timestamped frame set",
    );
  }
  const candidates = names.map((file, index) => ({
    file,
    capturedSeconds: roundedSeconds(timestamps[index]),
  }));
  for (const { capturedSeconds } of [...candidates]) {
    await addCandidateAt(
      candidates,
      candidateDirectory,
      path.join(sessionDirectory, "capture.mkv"),
      Math.max(0, capturedSeconds - frameSeconds),
    );
  }
  const finalTime = Math.max(0, state.durationSeconds - frameSeconds);
  await addCandidateAt(
    candidates,
    candidateDirectory,
    path.join(sessionDirectory, "capture.mkv"),
    finalTime,
  );
  candidates.sort(
    (left, right) => left.capturedSeconds - right.capturedSeconds,
  );
  await writeJson(path.join(sessionDirectory, "candidates.json"), candidates);
  return candidates;
}

async function addCandidateAt(candidates, directory, videoPath, seconds) {
  const capturedSeconds = roundedSeconds(seconds);
  if (
    candidates.some(
      (candidate) =>
        Math.abs(candidate.capturedSeconds - capturedSeconds) <
        frameSeconds / 2,
    )
  ) {
    return;
  }
  const file = `${String(candidates.length).padStart(6, "0")}.png`;
  await extractFrameAt(videoPath, capturedSeconds, path.join(directory, file));
  candidates.push({ file, capturedSeconds });
}

async function snapshotSession(sessionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (state.status !== "recording") {
    throw new Error(`session is not recording: ${state.status}`);
  }
  const outputPath = path.join(sessionDirectory, "observation.png");
  await runMonitoredProcess(
    "ffmpeg",
    [
      "-hide_banner",
      "-loglevel",
      "error",
      "-f",
      "x11grab",
      "-draw_mouse",
      "0",
      "-video_size",
      resolutionText(state.resolution),
      "-i",
      `${state.display}.0+0,0`,
      "-frames:v",
      "1",
      "-update",
      "1",
      "-y",
      outputPath,
    ],
    {
      cwd: repositoryRoot,
      description: "Live Capture Session observation frame",
      timeoutMilliseconds: 30_000,
    },
  );
  process.stdout.write(`${outputPath}\n`);
}

async function requestStop(sessionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (state.status !== "recording") {
    throw new Error(`session is not recording: ${state.status}`);
  }
  await writeFile(path.join(sessionDirectory, "stop-requested"), "stop\n", {
    flag: "wx",
  });
  emit({ type: "stop-requested", path: sessionDirectory });
}

export async function reviewSession(sessionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (state.status !== "recorded") {
    throw new Error(`session is not ready for review: ${state.status}`);
  }
  const candidates = JSON.parse(
    await readFile(path.join(sessionDirectory, "candidates.json"), "utf8"),
  );
  const reviewDirectory = path.join(sessionDirectory, "review");
  await rm(reviewDirectory, { force: true, recursive: true });
  await mkdir(reviewDirectory);
  const fullRatePages = await createFullRateReviewSheets(
    sessionDirectory,
    state,
    reviewDirectory,
  );
  const candidatePages = [];
  for (let offset = 0; offset < candidates.length; offset += 25) {
    const pageCandidates = candidates.slice(offset, offset + 25);
    const outputPath = path.join(
      reviewDirectory,
      `candidate-page-${String(candidatePages.length + 1).padStart(3, "0")}.png`,
    );
    await createContactSheet(
      pageCandidates.map((candidate) =>
        path.join(sessionDirectory, "candidates", candidate.file),
      ),
      pageCandidates.map((candidate) =>
        formatRelativeTimestamp(candidate.capturedSeconds, "R"),
      ),
      outputPath,
    );
    candidatePages.push(outputPath);
  }
  for (const page of [...fullRatePages, ...candidatePages]) {
    process.stdout.write(`${page}\n`);
  }
  return { fullRatePages, candidatePages };
}

export async function inspectRecordedFrame(sessionPath, secondsText) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (!["recorded", "published"].includes(state.status)) {
    throw new Error(`session is not ready for inspection: ${state.status}`);
  }
  const seconds = Number(secondsText);
  if (
    !Number.isFinite(seconds) ||
    seconds < 0 ||
    seconds > state.durationSeconds
  ) {
    throw new Error(
      `--at must be between zero and ${state.durationSeconds} seconds`,
    );
  }
  const outputPath = path.join(
    sessionDirectory,
    `inspection-${String(Math.round(seconds * 1000)).padStart(6, "0")}.png`,
  );
  await extractFrameAt(
    path.join(sessionDirectory, "capture.mkv"),
    seconds,
    outputPath,
  );
  await assertImageDimensions(outputPath, state.resolution);
  process.stdout.write(`${outputPath}\n`);
  return outputPath;
}

async function createFullRateReviewSheets(
  sessionDirectory,
  state,
  reviewDirectory,
) {
  const framesPerPage = 100;
  const columns = 10;
  const frameCount = Math.max(
    1,
    Math.round(state.durationSeconds * state.framesPerSecond),
  );
  const pages = [];
  const index = { framesPerSecond: state.framesPerSecond, pages: [] };
  for (
    let firstFrame = 0;
    firstFrame < frameCount;
    firstFrame += framesPerPage
  ) {
    const count = Math.min(framesPerPage, frameCount - firstFrame);
    const rows = Math.ceil(count / columns);
    const file = `full-rate-page-${String(pages.length + 1).padStart(3, "0")}.png`;
    const outputPath = path.join(reviewDirectory, file);
    await runMonitoredProcess(
      "ffmpeg",
      [
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        (firstFrame / state.framesPerSecond).toFixed(3),
        "-i",
        path.join(sessionDirectory, "capture.mkv"),
        "-vf",
        `scale=192:108,drawtext=font=monospace:text='%{n}':fontcolor=white:fontsize=16:box=1:boxcolor=black@0.78:boxborderw=3:x=4:y=h-th-4,tile=${columns}x${rows}:nb_frames=${count}:padding=1:margin=0:color=black`,
        "-frames:v",
        "1",
        "-y",
        outputPath,
      ],
      {
        cwd: repositoryRoot,
        description: "full-rate Live Capture Session review sheet",
        timeoutMilliseconds: 120_000,
      },
    );
    pages.push(outputPath);
    index.pages.push({
      file,
      firstFrame,
      count,
      columns,
      startSeconds: roundedSeconds(firstFrame / state.framesPerSecond),
    });
  }
  await writeJson(path.join(reviewDirectory, "review-index.json"), index);
  return pages;
}

export async function publishSession(sessionPath, selectionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (!["recorded", "failed"].includes(state.status)) {
    throw new Error(`session is not publishable: ${state.status}`);
  }
  const selection = validateSelection(
    JSON.parse(await readFile(path.resolve(selectionPath), "utf8")),
    state,
  );
  const outputBase = `${state.eventSlug}-${state.date}${selection.complete ? "" : "-incomplete"}`;
  const outputDirectory = await createCollisionSafeDirectory(outputBase);
  const publishedFrames = [];
  let annotationWarning;
  try {
    for (const [index, frame] of selection.frames.entries()) {
      const fileName = `${String(index).padStart(2, "0")}-${semanticSlug(frame.name)}.png`;
      const outputPath = path.join(outputDirectory, fileName);
      await extractFrameAt(
        path.join(sessionDirectory, "capture.mkv"),
        frame.at,
        outputPath,
      );
      await assertImageDimensions(outputPath, state.resolution);
      publishedFrames.push({ ...frame, fileName, outputPath });
    }
    if (publishedFrames.length > 0) {
      const origin = publishedFrames[0].at;
      const overviewResult = await createContactSheet(
        publishedFrames.map((frame) => frame.outputPath),
        publishedFrames.map((frame) =>
          formatRelativeTimestamp(frame.at - origin),
        ),
        path.join(outputDirectory, "overview.png"),
      );
      if (!overviewResult.annotated) {
        annotationWarning =
          "Overview timestamp annotation was unavailable; consult this timeline for authoritative relative times.";
      }
    }
    await writeFile(
      path.join(outputDirectory, "README.md"),
      renderReadme(selection, state, publishedFrames, annotationWarning),
      { flag: "wx" },
    );
    if (selection.preserveDiagnostics === true) {
      await copyIfPresent(
        path.join(sessionDirectory, "capture.mkv"),
        path.join(outputDirectory, "raw-capture.mkv"),
      );
      await copyIfPresent(
        path.join(sessionDirectory, "diagnostics.log"),
        path.join(outputDirectory, "diagnostics.log"),
      );
      await copyIfPresent(
        path.join(sessionDirectory, "candidates.json"),
        path.join(outputDirectory, "candidates.json"),
      );
    }
    await validatePublishedOutput(outputDirectory, publishedFrames);
    await writeJson(path.join(sessionDirectory, "session.json"), {
      ...state,
      status: "published",
      publication: {
        sourceStatus: state.status,
        outputDirectory,
        frameFiles: publishedFrames.map(({ fileName }) => fileName),
        hasOverview: publishedFrames.length > 0,
      },
    });
  } catch (error) {
    await rm(outputDirectory, { force: true, recursive: true });
    throw error;
  }
  emit({ type: "published", path: outputDirectory });
  return outputDirectory;
}

export async function retractPublication(sessionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (state.status !== "published") {
    throw new Error(`session has no pending publication: ${state.status}`);
  }
  const outputDirectory = await validatedPublishedDirectory(state);
  await rm(outputDirectory, { recursive: true });
  const { publication, ...restored } = state;
  await writeJson(path.join(sessionDirectory, "session.json"), {
    ...restored,
    status: publication.sourceStatus,
  });
  emit({ type: "retracted", path: outputDirectory });
}

export async function finalizeSession(sessionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (state.status !== "published") {
    throw new Error(`session is not ready to finalize: ${state.status}`);
  }
  const outputDirectory = await validatedPublishedDirectory(state);
  await access(path.join(outputDirectory, "README.md"));
  if (state.publication.hasOverview) {
    await access(path.join(outputDirectory, "overview.png"));
  }
  for (const fileName of state.publication.frameFiles) {
    await access(path.join(outputDirectory, fileName));
  }
  await rm(sessionDirectory, { recursive: true });
  emit({ type: "finalized", path: outputDirectory });
  return outputDirectory;
}

async function discardSession(sessionPath) {
  const sessionDirectory = await validatedSessionDirectory(sessionPath);
  const state = await readState(sessionDirectory);
  if (state.status === "published") {
    throw new Error(
      "published session must be finalized or retracted before discard",
    );
  }
  await rm(sessionDirectory, { recursive: true });
  emit({ type: "discarded", path: sessionDirectory });
}

async function createContactSheet(inputPaths, labels, outputPath) {
  if (inputPaths.length !== labels.length || inputPaths.length === 0) {
    throw new Error("contact sheet requires a label for every frame");
  }
  const parent = path.dirname(outputPath);
  const work = await mkdtemp(path.join(parent, ".contact-sheet."));
  let annotated = true;
  try {
    try {
      await createThumbnails(inputPaths, labels, work, true);
    } catch {
      annotated = false;
      await rm(work, { force: true, recursive: true });
      await mkdir(work);
      await createThumbnails(inputPaths, labels, work, false);
    }
    const rows = Math.ceil(inputPaths.length / 5);
    await runMonitoredProcess(
      "ffmpeg",
      [
        "-hide_banner",
        "-loglevel",
        "error",
        "-framerate",
        "1",
        "-start_number",
        "0",
        "-i",
        path.join(work, "%03d.png"),
        "-vf",
        `tile=5x${rows}:nb_frames=${inputPaths.length}:padding=2:margin=0:color=black`,
        "-frames:v",
        "1",
        "-y",
        outputPath,
      ],
      {
        cwd: repositoryRoot,
        description: "Live Capture Session overview",
        timeoutMilliseconds: 120_000,
      },
    );
  } finally {
    await rm(work, { force: true, recursive: true });
  }
  return { annotated };
}

async function createThumbnails(inputPaths, labels, directory, annotate) {
  for (const [index, inputPath] of inputPaths.entries()) {
    const filters = [
      "scale=384:216:force_original_aspect_ratio=decrease",
      "pad=384:216:(ow-iw)/2:(oh-ih)/2:black",
    ];
    if (annotate) {
      filters.push(
        `drawtext=font=monospace:text='${labels[index]}':fontcolor=white:fontsize=22:box=1:boxcolor=black@0.78:boxborderw=6:x=8:y=h-th-8`,
      );
    }
    await runMonitoredProcess(
      "ffmpeg",
      [
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        inputPath,
        "-vf",
        filters.join(","),
        "-frames:v",
        "1",
        "-y",
        path.join(directory, `${String(index).padStart(3, "0")}.png`),
      ],
      {
        cwd: repositoryRoot,
        description: "Live Capture Session overview thumbnail",
        timeoutMilliseconds: 30_000,
      },
    );
  }
}

async function extractFrameAt(videoPath, seconds, outputPath) {
  await runMonitoredProcess(
    "ffmpeg",
    [
      "-hide_banner",
      "-loglevel",
      "error",
      "-ss",
      seconds.toFixed(3),
      "-i",
      videoPath,
      "-frames:v",
      "1",
      "-y",
      outputPath,
    ],
    {
      cwd: repositoryRoot,
      description: "Live Capture Frame extraction",
      timeoutMilliseconds: 30_000,
    },
  );
}

async function assertImageDimensions(imagePath, expected) {
  const output = await runMonitoredProcess(
    "ffprobe",
    [
      "-v",
      "error",
      "-select_streams",
      "v:0",
      "-show_entries",
      "stream=width,height",
      "-of",
      "csv=p=0:s=x",
      imagePath,
    ],
    {
      cwd: repositoryRoot,
      description: "Live Capture Frame validation",
      timeoutMilliseconds: 30_000,
    },
  );
  if (output.trim() !== resolutionText(expected)) {
    throw new Error(
      `Live Capture Frame is ${output.trim()}; expected ${resolutionText(expected)}`,
    );
  }
}

async function validatePublishedOutput(outputDirectory, frames) {
  await access(path.join(outputDirectory, "README.md"));
  if (frames.length > 0) {
    await access(path.join(outputDirectory, "overview.png"));
  }
  for (const frame of frames) {
    await access(frame.outputPath);
  }
}

async function createCollisionSafeDirectory(baseName) {
  for (let index = 1; ; index += 1) {
    const suffix = index === 1 ? "" : `-${String(index).padStart(2, "0")}`;
    const candidate = path.join(scratchRoot, `${baseName}${suffix}`);
    try {
      await mkdir(candidate);
      return candidate;
    } catch (error) {
      if (error?.code !== "EEXIST") {
        throw error;
      }
    }
  }
}

async function validatedSessionDirectory(sessionPath) {
  const resolved = path.resolve(sessionPath);
  if (
    path.dirname(resolved) !== scratchRoot ||
    !path.basename(resolved).startsWith("task.")
  ) {
    throw new Error(`refusing non-session directory: ${resolved}`);
  }
  const metadata = await lstat(resolved);
  if (!metadata.isDirectory()) {
    throw new Error(`session path is not a directory: ${resolved}`);
  }
  await access(path.join(resolved, "session.json"));
  return resolved;
}

async function validatedPublishedDirectory(state) {
  const candidate = state.publication?.outputDirectory;
  if (typeof candidate !== "string") {
    throw new Error("session publication has no output directory");
  }
  const resolved = path.resolve(candidate);
  const expectedPrefix = `${state.eventSlug}-${state.date}`;
  const outputName = path.basename(resolved);
  if (
    path.dirname(resolved) !== scratchRoot ||
    (outputName !== expectedPrefix &&
      !outputName.startsWith(`${expectedPrefix}-`)) ||
    outputName.startsWith("task.")
  ) {
    throw new Error(`refusing non-publication directory: ${resolved}`);
  }
  const metadata = await lstat(resolved);
  if (!metadata.isDirectory()) {
    throw new Error(`publication path is not a directory: ${resolved}`);
  }
  return resolved;
}

async function readState(sessionDirectory) {
  return JSON.parse(
    await readFile(path.join(sessionDirectory, "session.json"), "utf8"),
  );
}

async function readJsonFile(filePath, description) {
  try {
    return JSON.parse(await readFile(filePath, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT" || error instanceof SyntaxError) {
      throw new Error(`${description} is missing or invalid: ${filePath}`, {
        cause: error,
      });
    }
    throw error;
  }
}

async function executableOnPath(command) {
  for (const directory of (process.env.PATH ?? "").split(path.delimiter)) {
    if (directory.length === 0) {
      continue;
    }
    try {
      await access(path.join(directory, command), fsConstants.X_OK);
      return true;
    } catch {
      // Continue searching PATH.
    }
  }
  return false;
}

async function runStreamingProcess(command, arguments_, onErrorLine) {
  const child = spawn(command, arguments_, {
    cwd: repositoryRoot,
    env: process.env,
    stdio: ["ignore", "ignore", "pipe"],
  });
  const completed = new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("close", (code, childSignal) => resolve([code, childSignal]));
  });
  const recent = [];
  const lines = createInterface({ input: child.stderr });
  for await (const line of lines) {
    onErrorLine(line);
    recent.push(line);
    if (recent.length > 20) {
      recent.shift();
    }
  }
  const [exitCode, signal] = await completed;
  if (exitCode !== 0) {
    throw new Error(
      `${command} exited with ${signal ?? exitCode ?? "unknown status"}\n${recent.join("\n")}`,
    );
  }
}

async function writeDiagnostics(sessionDirectory, details) {
  const sections = [`failure:\n${errorMessage(details.error)}`];
  for (const [name, child] of Object.entries(details)) {
    if (name === "error" || child === undefined) {
      continue;
    }
    const output = child.capturedStandardOutput?.trim();
    const error = child.capturedStandardError?.trim();
    if (output || error) {
      sections.push(`${name}:\n${[output, error].filter(Boolean).join("\n")}`);
    }
  }
  await writeFile(
    path.join(sessionDirectory, "diagnostics.log"),
    `${sections.join("\n\n")}\n`,
  );
}

async function copyIfPresent(source, destination) {
  try {
    await copyFile(source, destination, fsConstants.COPYFILE_EXCL);
  } catch (error) {
    if (error?.code !== "ENOENT") {
      throw error;
    }
  }
}

function requiredOption(arguments_, option) {
  const index = arguments_.indexOf(option);
  if (index < 0) {
    throw new Error(`${option} is required`);
  }
  const value = optionValue(arguments_, index + 1, option);
  if (arguments_.length !== 2) {
    throw new Error(`unexpected options for command: ${arguments_.join(" ")}`);
  }
  return value;
}

function namedOptions(arguments_, names) {
  if (arguments_.length !== names.length * 2) {
    throw new Error(`expected options: ${names.join(", ")}`);
  }
  const parsed = {};
  for (let index = 0; index < arguments_.length; index += 2) {
    const name = arguments_[index];
    if (!names.includes(name) || parsed[name] !== undefined) {
      throw new Error(`unexpected or duplicate option: ${name}`);
    }
    parsed[name] = optionValue(arguments_, index + 1, name);
  }
  for (const name of names) {
    if (parsed[name] === undefined) {
      throw new Error(`${name} is required`);
    }
  }
  return parsed;
}

function optionValue(arguments_, index, option) {
  const value = arguments_[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}

function semanticSlug(value) {
  requiredNonemptyString(value, "semantic name");
  const slug = value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80)
    .replace(/-+$/g, "");
  if (slug.length === 0) {
    throw new Error(`semantic name cannot form a filename: ${value}`);
  }
  return slug;
}

function requiredNonemptyString(value, description) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${description} must be a nonempty string`);
  }
}

function resolutionText({ width, height }) {
  return `${width}x${height}`;
}

function roundedSeconds(value) {
  return Math.round(value * 1000) / 1000;
}

function tableText(value) {
  return value.replace(/\s+/g, " ").replace(/\|/g, "\\|").trim();
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function localDate(now = new Date()) {
  return [now.getFullYear(), now.getMonth() + 1, now.getDate()]
    .map((part, index) => String(part).padStart(index === 0 ? 4 : 2, "0"))
    .join("-");
}

async function writeJson(filePath, value) {
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function emit(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`Live Capture Session: ${errorMessage(error)}\n`);
    process.exitCode = 1;
  });
}
