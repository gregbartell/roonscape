import { closeSync, openSync, writeSync } from "node:fs";
import { mkdir, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createNativeSession } from "./native-session.mjs";
import {
  processCancellation,
  startMonitoredProcess,
  stopProcess,
  waitForProcessExit,
} from "./process-harness.mjs";

const root = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const scratchRoot = "/var/tmp/codex/roonscape";

async function main() {
  const options = process.argv.slice(2);
  if (options.includes("--help")) {
    console.log(
      "Usage: npm run verify -- [--design | --presentation-ci]\nAlways runs repository checks; --design also runs the design suite; --presentation-ci also captures the maintained CI fallback scope.",
    );
    return;
  }
  if (
    options.some(
      (option) => !["--design", "--presentation-ci"].includes(option),
    )
  )
    throw new Error("Unknown verification option; use --help");
  await mkdir(scratchRoot, { recursive: true });
  const review = await mkdtemp(path.join(scratchRoot, "review."));
  console.log(`Review directory: ${review}`);
  const cancellation = processCancellation();
  const report = {
    startedAt: new Date().toISOString(),
    outcome: "incomplete",
    commands: [],
    designRequested:
      options.includes("--design") || options.includes("--presentation-ci"),
    presentationCiRequested: options.includes("--presentation-ci"),
    automatedOutcome: "incomplete",
    captureCompletion: "not assessed",
  };
  let session;
  let runtime;
  // Keep tool/cache locations across the native session's private HOME. Never
  // serialize the inherited environment or inspect authorization/config files.
  const environment = {
    ...process.env,
    CARGO_HOME: process.env.CARGO_HOME ?? path.join(os.homedir(), ".cargo"),
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? path.join(os.homedir(), ".rustup"),
    npm_config_cache:
      process.env.npm_config_cache ?? path.join(os.homedir(), ".npm"),
    CARGO_TARGET_DIR: path.join(root, "target"),
    CARGO_BUILD_TARGET_DIR: path.join(root, "target"),
    CARGO_BUILD_BUILD_DIR: path.join(root, "target"),
    RUSTUP_AUTO_INSTALL: "0",
  };
  try {
    await save();
    runtime = await mkdtemp(path.join(os.tmpdir(), "rs-v."));
    report.commandRuntimeDirectory = runtime;
    environment.TMPDIR = runtime;
    await command(
      ["run", "dev:diagnose", "--", "--evidence", review],
      environment,
    );
    session = await createNativeSession({
      width: 1600,
      height: 900,
      environment,
      signal: cancellation.signal,
    });
    report.runtimeDirectory = session.runtimeDirectory;
    await save();
    await command(["run", "check"], session.environment);
    if (report.designRequested)
      await command(["run", "test:design"], session.environment);
    report.automatedOutcome = "complete";
    report.outcome = "complete";
    if (report.presentationCiRequested) {
      report.captureCompletion = "incomplete";
      await save();
      await command(
        [
          "run",
          "review:presentations:built",
          "--",
          "--output",
          review,
          "--scope",
          "ci-fallback",
        ],
        environment,
      );
      report.captureCompletion = "complete";
    }
  } catch (error) {
    report.outcome = cancellation.signal.aborted ? "cancelled" : "failed";
    if (report.automatedOutcome !== "complete")
      report.automatedOutcome = report.outcome;
    if (report.captureCompletion === "incomplete")
      report.captureCompletion = report.outcome;
    report.error = error.message;
    console.error(error.message);
    process.exitCode = cancellation.signal.aborted ? 130 : 1;
  } finally {
    try {
      await session?.close();
      if (runtime) await rm(runtime, { recursive: true, force: true });
    } catch (error) {
      report.cleanupError = error.message;
      console.error(error.message);
      report.outcome = "incomplete";
      process.exitCode = 1;
    }
    report.finishedAt = new Date().toISOString();
    if (cancellation.signal.aborted) {
      report.outcome = "cancelled";
      process.exitCode = 130;
    }
    try {
      if (report.outcome === "complete") await rm(review, { recursive: true });
      else await save();
    } finally {
      cancellation.dispose();
    }
  }
  console.log(
    `Workflow: ${report.outcome}; automated checks: ${report.automatedOutcome}; captures: ${report.captureCompletion}${report.outcome === "complete" ? "" : `; diagnostics: ${review}`}`,
  );

  async function command(arguments_, commandEnvironment) {
    cancellation.signal.throwIfAborted();
    const number = report.commands.length + 1;
    const entry = {
      command: "npm",
      supervisor: "python3 scripts/verification-process.py",
      arguments: arguments_,
      startedAt: new Date().toISOString(),
      outcome: "incomplete",
      stdout: `${number}.stdout.log`,
      stderr: `${number}.stderr.log`,
    };
    report.commands.push(entry);
    await save();
    console.log(`Running npm ${arguments_.join(" ")}`);
    const stdout = openSync(path.join(review, entry.stdout), "wx", 0o600);
    const stderr = openSync(path.join(review, entry.stderr), "wx", 0o600);
    let child;
    const commandCancellation = new AbortController();
    const abort = () => commandCancellation.abort(cancellation.signal.reason);
    cancellation.signal.addEventListener("abort", abort, { once: true });
    if (cancellation.signal.aborted) abort();
    const capture = (descriptor, output) => (data) => {
      if (commandCancellation.signal.aborted) return;
      try {
        writeSync(descriptor, data);
        output.write(data);
      } catch (error) {
        commandCancellation.abort(error);
      }
    };
    try {
      child = startMonitoredProcess(
        "python3",
        [
          path.join(root, "scripts/verification-process.py"),
          "npm",
          ...arguments_,
        ],
        {
          cwd: root,
          environment: commandEnvironment,
        },
      );
      child.stdout.on("data", capture(stdout, process.stdout));
      child.stderr.on("data", capture(stderr, process.stderr));
      await child.spawned;
      const [exitCode, signal] = await waitForProcessExit(child, {
        signal: commandCancellation.signal,
      });
      entry.exitCode = exitCode;
      entry.signal = signal;
      entry.outcome = exitCode === 0 ? "complete" : "failed";
      if (exitCode !== 0)
        throw new Error(
          `npm ${arguments_.join(" ")} failed (${signal ?? exitCode}); see ${entry.stderr}`,
        );
    } catch (error) {
      entry.outcome = cancellation.signal.aborted ? "cancelled" : "failed";
      entry.error = error.message;
      throw error;
    } finally {
      try {
        await stopProcess(child);
        if (child) {
          entry.exitCode = child.exitCode;
          entry.signal = child.signalCode;
        }
      } finally {
        cancellation.signal.removeEventListener("abort", abort);
        child?.stdout.removeAllListeners("data");
        child?.stderr.removeAllListeners("data");
        closeSync(stdout);
        closeSync(stderr);
        entry.finishedAt = new Date().toISOString();
        await save();
      }
    }
  }

  async function save() {
    const file = path.join(review, "verification.json");
    await writeFile(`${file}.tmp`, `${JSON.stringify(report, null, 2)}\n`, {
      mode: 0o600,
    });
    await rename(`${file}.tmp`, file);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
