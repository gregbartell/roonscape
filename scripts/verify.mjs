import { closeSync, openSync, writeSync } from "node:fs";
import {
  mkdir,
  mkdtemp,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createNativeSession } from "./native-session.mjs";
import {
  processCancellation,
  runMonitoredProcess,
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
    source: { root },
    environment: {
      platform: os.platform(),
      release: os.release(),
      architecture: os.arch(),
      node: process.version,
    },
    commands: [],
    designRequested:
      options.includes("--design") || options.includes("--presentation-ci"),
    presentationCiRequested: options.includes("--presentation-ci"),
    automatedOutcome: "incomplete",
    captureCompletion: "not assessed",
    visualAcceptance: "not assessed",
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
    report.source.revision = (
      await runMonitoredProcess("git", ["rev-parse", "HEAD"], {
        cwd: root,
        signal: cancellation.signal,
      })
    ).trim();
    report.source.workingTree = await runMonitoredProcess(
      "git",
      ["status", "--porcelain=v1", "--untracked-files=all"],
      { cwd: root, signal: cancellation.signal },
    );
    await command(
      ["run", "dev:diagnose", "--", "--evidence", review],
      environment,
    );
    report.environment.tools = {};
    for (const [name, args] of [
      ["npm", ["--version"]],
      ["rustc", ["--version"]],
      ["cargo", ["--version"]],
      ["ffmpeg", ["-version"]],
      ["ffprobe", ["-version"]],
      ["pkg-config", ["--modversion", "gtk4"]],
    ]) {
      report.environment.tools[name] = (
        await runMonitoredProcess(name, args, {
          cwd: root,
          environment,
          signal: cancellation.signal,
        })
      ).split("\n")[0];
    }

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
          "--review",
          review,
          "--scope",
          "ci-fallback",
          "--rationale",
          "CI representative Now Playing, Full-field, long metadata, and light palette coverage using packaged fallback fonts.",
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
    process.exitCode = cancellation.signal.aborted ? 130 : 1;
  } finally {
    try {
      await session?.close();
      if (runtime) await rm(runtime, { recursive: true, force: true });
    } catch (error) {
      report.cleanupError = error.message;
      report.outcome = "incomplete";
      process.exitCode = 1;
    }
    report.finishedAt = new Date().toISOString();
    if (cancellation.signal.aborted) {
      report.outcome = "cancelled";
      process.exitCode = 130;
    }
    await save();
    cancellation.dispose();
  }
  console.log(
    `Workflow: ${report.outcome}; automated checks: ${report.automatedOutcome}; captures: ${report.captureCompletion}; visual acceptance: ${report.visualAcceptance}. See ${path.join(review, "README.md")}`,
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
    const capture = (descriptor) => (data) => {
      if (commandCancellation.signal.aborted) return;
      try {
        writeSync(descriptor, data);
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
      child.stdout.on("data", capture(stdout));
      child.stderr.on("data", capture(stderr));
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
    const presentations = (
      await readdir(review, { withFileTypes: true })
    ).filter(
      (entry) => entry.isDirectory() && entry.name.startsWith("presentation."),
    );
    const presentationLinks = presentations
      .map(({ name }) => `- [Presentation review: ${name}](${name}/index.html)`)
      .join("\n");
    const index = `# Verification review\n\nWorkflow outcome: **${report.outcome}**\n\nAutomated outcome: **${report.automatedOutcome}**\nCapture completion: **${report.captureCompletion}**\nVisual acceptance: **${report.visualAcceptance}**\n\nSource: ${report.source.root}\nRevision: ${report.source.revision ?? "not recorded"}\nStarted: ${report.startedAt}\nFinished: ${report.finishedAt ?? "pending"}\n\nWorking-tree state (porcelain):\n\n\`\`\`\n${report.source.workingTree || "clean or not yet recorded\n"}\`\`\`\n\nEnvironment: ${JSON.stringify(report.environment)}\n\n${report.commands.map((entry) => `- \`npm ${entry.arguments.join(" ")}\`: ${entry.outcome}; exit ${entry.exitCode ?? "not available"}, signal ${entry.signal ?? "none"}; [stdout](${entry.stdout}), [stderr](${entry.stderr})`).join("\n")}\n\n${report.error ?? ""}\n${report.cleanupError ?? ""}\n\nAutomated completion does not establish capture completion or visual acceptance.\nLive Capture Session helper tests use deterministic logic and synthetic media; they do not verify an actual Live Capture Session.\n${presentationLinks}\n\nSee [structured outcomes](verification.json).\n`;
    for (const [name, contents] of [
      ["verification.json", `${JSON.stringify(report, null, 2)}\n`],
      ["README.md", index],
    ]) {
      await writeFile(path.join(review, `${name}.tmp`), contents, {
        mode: 0o600,
      });
      await rename(path.join(review, `${name}.tmp`), path.join(review, name));
    }
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
