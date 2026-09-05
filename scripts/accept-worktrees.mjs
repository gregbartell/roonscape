import assert from "node:assert/strict";
import { createWriteStream } from "node:fs";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import {
  assertProcessRunning,
  processCancellation,
  runMonitoredProcess,
  startMonitoredProcess,
  stopProcesses,
  waitFor,
  waitForProcessExit,
} from "./process-harness.mjs";

// This is a deliberately opt-in acceptance exercise, not part of verify (which
// it invokes). Worktree/branch creation remains the caller's responsibility.
async function main() {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--help") {
    console.log(
      "Usage: npm run accept:worktrees -- /absolute/worktree-a /absolute/worktree-b\nRequires two fresh, existing, clean worktrees on a provisioned Linux host. Retains evidence under /var/tmp/codex/roonscape.",
    );
    return;
  }
  assert.equal(
    args.length,
    2,
    "Provide two fresh existing worktree paths; use --help",
  );
  const roots = await Promise.all(args.map((root) => realpath(root)));
  assert.notEqual(roots[0], roots[1], "Worktrees must be distinct");
  for (const root of roots) {
    assert.equal(
      (
        await runMonitoredProcess("git", ["rev-parse", "--show-toplevel"], {
          cwd: root,
        })
      ).trim(),
      root,
      "Provide a worktree root",
    );
    assert.equal(
      (
        await runMonitoredProcess(
          "git",
          ["status", "--porcelain=v1", "--untracked-files=all"],
          { cwd: root },
        )
      ).trim(),
      "",
      "Worktrees must start clean",
    );
    for (const generated of ["node_modules", "target", "src/bridge/dist"]) {
      await assert.rejects(
        access(path.join(root, generated)),
        { code: "ENOENT" },
        `Fresh worktree already contains ${generated}`,
      );
    }
  }
  const scratch = "/var/tmp/codex/roonscape";
  await mkdir(scratch, { recursive: true });
  const evidence = await mkdtemp(path.join(scratch, "acceptance."));
  // Short runtime path for Unix sockets; retained evidence never lives here.
  const runtime = await mkdtemp("/tmp/rs-a.");
  const cancellation = processCancellation();
  const children = [];
  const streams = [];
  const report = {
    outcome: "incomplete",
    startedAt: new Date().toISOString(),
    roots,
    commands: [],
    observations: [],
    visualAcceptance: "unreviewed",
    liveObservation: "not performed",
  };
  console.log(`Acceptance evidence: ${evidence}`);
  let sentinel;
  let sentinelEnvironment;
  let sentinelWindow;
  try {
    await save();
    const environments = [];
    for (const [index, root] of roots.entries()) {
      const temporary = path.join(runtime, String(index));
      await mkdir(temporary);
      // npm may enable Node's compile cache under TMPDIR. Disable that
      // disposable optimization so the runtime-removal assertion measures
      // owned native resources rather than unrelated bytecode cache files.
      const environment = {
        ...process.env,
        TMPDIR: temporary,
        NODE_DISABLE_COMPILE_CACHE: "1",
      };
      for (const name of [
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET_DIR",
        "CARGO_BUILD_BUILD_DIR",
      ])
        delete environment[name];
      environments.push(environment);
      await complete(
        start(root, environment, `diagnose-${index}`, "npm", [
          "run",
          "dev:diagnose",
        ]),
      );
      await complete(
        start(root, environment, `prepare-${index}`, "npm", [
          "run",
          "dev:prepare",
        ]),
      );
    }
    // Build only after demonstrating fresh dependency preparation. The public
    // fixture command builds and starts the controlled neighboring session.
    sentinel = start(roots[0], environments[0], "sentinel", "npm", [
      "run",
      "fixture",
      "--",
      "--headless",
      "--static",
      "--scenario",
      "idle",
      "--resolution",
      "1280x720",
    ]);
    const ready = await waitFor(
      () => {
        const match = sentinel.child.capturedStandardOutput.match(
          /Headless Fixture Mode ready.*DISPLAY=([^;]+); runtime=(\S+)/,
        );
        assert.ok(match, "sentinel is not ready");
        return match;
      },
      sentinel.child,
      "sentinel startup including fresh build",
      { timeoutMilliseconds: 20 * 60_000, signal: cancellation.signal },
    );
    sentinelEnvironment = {
      ...process.env,
      DISPLAY: ready[1],
      XAUTHORITY: "/dev/null",
    };
    report.sentinel = { display: ready[1], runtimeDirectory: ready[2] };
    await probeSentinel("before verification");

    const verification = roots.map((root, index) =>
      start(root, environments[index], `verify-${index}`, "npm", [
        "run",
        "verify",
        "--",
        "--presentation-ci",
      ]),
    );
    const reviewPaths = await Promise.all(
      verification.map(async (run) =>
        waitFor(
          () => {
            const match = run.child.capturedStandardOutput.match(
              /Review directory: (\S+)/,
            );
            assert.ok(match);
            return match[1];
          },
          run.child,
          "verification evidence directory",
          { timeoutMilliseconds: 30_000, signal: cancellation.signal },
        ),
      ),
    );
    assert.notEqual(...reviewPaths);
    report.reviews = reviewPaths;
    await save();
    // Observe both native sessions at once, rather than infer concurrency from
    // adjacent command timestamps. Their private configuration must differ.
    const nativeReports = await waitFor(
      async () => {
        const reports = await Promise.all(
          reviewPaths.map((directory) =>
            json(path.join(directory, "verification.json")),
          ),
        );
        for (const item of reports) {
          assert.ok(item.runtimeDirectory);
          assert.equal(item.finishedAt, undefined);
          await access(path.join(item.runtimeDirectory, "display.json"));
        }
        return reports;
      },
      verification[0].child,
      "overlapping native verification sessions",
      { timeoutMilliseconds: 60_000, signal: cancellation.signal },
    );
    assert.notEqual(
      nativeReports[0].runtimeDirectory,
      nativeReports[1].runtimeDirectory,
    );
    assert.notEqual(
      nativeReports[0].runtimeDirectory,
      report.sentinel.runtimeDirectory,
    );
    assert.notEqual(
      nativeReports[1].runtimeDirectory,
      report.sentinel.runtimeDirectory,
    );
    report.observations.push({
      phase: "concurrent verification",
      nativeRuntimeDirectories: nativeReports.map(
        (item) => item.runtimeDirectory,
      ),
    });
    await probeSentinel("during verification");
    await Promise.all(verification.map((run) => complete(run)));
    for (const [index, directory] of reviewPaths.entries()) {
      const item = await json(path.join(directory, "verification.json"));
      assert.equal(item.outcome, "complete");
      assert.equal(item.automatedOutcome, "complete");
      assert.equal(item.captureCompletion, "complete");
      assert.equal(item.visualAcceptance, "not assessed");
      assert.equal(await realpath(item.source.root), roots[index]);
      assert.equal(
        item.source.revision,
        (
          await runMonitoredProcess("git", ["rev-parse", "HEAD"], {
            cwd: roots[index],
          })
        ).trim(),
      );
      assert.equal(item.source.workingTree, "");
      for (const entry of item.commands) {
        assert.equal(entry.exitCode, 0);
        await access(path.join(directory, entry.stdout));
        await access(path.join(directory, entry.stderr));
      }
      for (const owned of [item.runtimeDirectory, item.commandRuntimeDirectory])
        await assert.rejects(access(owned), { code: "ENOENT" });
      const presentation = await findPresentation(directory);
      await inspectCaptures(presentation, "complete", "ci-fallback");
    }
    await probeSentinel("after verification");

    // Both commands use the maintained focused scope at all seven viewports.
    // More scenarios in A keep useful native work running while B is cancelled.
    const reviewArgs = (index, scenarios) => [
      "run",
      "review:presentations:built",
      "--",
      "--review",
      reviewPaths[index],
      "--scope",
      "focused",
      ...scenarios.flatMap((scenario) => ["--scenario", scenario]),
      "--rationale",
      "Acceptance of concurrent capture publication and cancellation: Now Playing and Full-field representatives at all maintained viewports; no presentation design changes.",
    ];
    const survivor = start(
      roots[0],
      environments[0],
      "capture-survivor",
      "npm",
      reviewArgs(0, ["playing", "idle", "long-metadata", "light-artwork"]),
    );
    const survivorDirectory = await presentationDirectory(survivor);
    const victim = start(
      roots[1],
      environments[1],
      "capture-cancelled",
      "npm",
      reviewArgs(1, ["playing", "idle"]),
    );
    const victimDirectory = await presentationDirectory(victim);
    const { owned, surviving } = await waitFor(
      async () => {
        const item = await json(path.join(victimDirectory, "captures.json"));
        assert.ok(
          item.completed.length > 0 &&
            item.completed.length < item.requested.length,
        );
        assert.equal(item.outcome, "incomplete");
        assertProcessRunning(survivor.child, "surviving capture command");
        const [owned, surviving] = await Promise.all([
          descendants(victim.child.pid),
          descendants(survivor.child.pid),
        ]);
        for (const processes of [owned, surviving]) {
          assert.ok(
            processes.some((entry) => entry.name.startsWith("roonscape")),
            "both reviews must have active native Renderers",
          );
        }
        return { owned, surviving };
      },
      victim.child,
      "published capture and overlapping native review work",
      {
        retryMilliseconds: 10,
        timeoutMilliseconds: 60_000,
        signal: cancellation.signal,
      },
    );
    const started = Date.now();
    // Signal the delivered JS CLI, not npm's shell: npm does not reliably
    // forward SIGTERM. The process is identified solely among owned descendants.
    const renderer = owned.find((entry) => entry.name.startsWith("roonscape"));
    // The existing native seam spawns the Renderer directly from the review
    // CLI. Node's comm can be MainThread, and npm need not insert a shell.
    const cliPid = renderer.parent;
    assert.ok(
      cliPid === victim.child.pid ||
        owned.some((entry) => entry.pid === cliPid),
      "presentation review CLI belongs to this command",
    );
    process.kill(cliPid, "SIGTERM");
    await complete(victim, 130, 5_000);
    report.cancellation = {
      elapsedMilliseconds: Date.now() - started,
      processes: owned,
      survivingProcesses: surviving,
      directory: victimDirectory,
    };
    await inspectCaptures(victimDirectory, "cancelled", "focused");
    for (const entry of owned)
      assert.throws(() => process.kill(entry.pid, 0), { code: "ESRCH" });
    assert.deepEqual(
      await readdir(environments[1].TMPDIR),
      [],
      "cancelled command removed owned runtime resources",
    );
    assertProcessRunning(survivor.child, "other capture survives cancellation");
    await probeSentinel("after neighboring cancellation");
    await complete(survivor);
    await inspectCaptures(survivorDirectory, "complete", "focused");
    report.focusedReviews = {
      survivor: survivorDirectory,
      cancelled: victimDirectory,
    };
    await probeSentinel("after surviving capture completion");
    report.outcome = "complete";
  } catch (error) {
    report.outcome = cancellation.signal.aborted ? "cancelled" : "failed";
    report.error = error.stack;
    process.exitCode = 1;
  } finally {
    try {
      await stopProcesses(children, { graceMilliseconds: 5_000 });
      for (const [index, child] of children.entries()) {
        const entry = report.commands[index];
        if (entry.outcome !== "incomplete") continue;
        const [exitCode, signal] = await waitForProcessExit(child, {
          timeoutMilliseconds: 5_000,
        });
        Object.assign(entry, {
          exitCode,
          signal,
          finishedAt: new Date().toISOString(),
          outcome:
            entry.label === "sentinel" && report.outcome === "complete"
              ? "stopped after observation"
              : "stopped during cleanup",
        });
      }
      if (report.sentinel)
        await assert.rejects(access(report.sentinel.runtimeDirectory), {
          code: "ENOENT",
        });
      // Check before deleting our containing runtime, so leaked child resources
      // cannot be hidden by the exercise's own cleanup.
      for (const entry of await readdir(runtime))
        assert.deepEqual(await readdir(path.join(runtime, entry)), []);
      report.cleanup = "complete";
      await rm(runtime, { recursive: true });
    } catch (error) {
      report.cleanup = error.message;
      report.outcome = "failed";
      process.exitCode = 1;
    }
    for (const stream of streams)
      await new Promise((resolve) => stream.end(resolve));
    cancellation.dispose();
    report.finishedAt = new Date().toISOString();
    await save();
  }
  console.log(
    `Acceptance: ${report.outcome}; visual acceptance: unreviewed. See ${evidence}/README.md`,
  );

  function start(cwd, environment, label, command, arguments_) {
    const child = startMonitoredProcess(command, arguments_, {
      cwd,
      environment,
    });
    children.push(child);
    const entry = {
      label,
      cwd,
      command,
      arguments: arguments_,
      startedAt: new Date().toISOString(),
      outcome: "incomplete",
      stdout: `${label}.stdout.log`,
      stderr: `${label}.stderr.log`,
    };
    report.commands.push(entry);
    for (const [source, name] of [
      [child.stdout, entry.stdout],
      [child.stderr, entry.stderr],
    ]) {
      const stream = createWriteStream(path.join(evidence, name), {
        flags: "wx",
        mode: 0o600,
      });
      streams.push(stream);
      source.pipe(stream, { end: false });
    }
    console.log(`${label}: ${command} ${arguments_.join(" ")}`);
    return { child, entry };
  }

  async function complete(
    run,
    expected = 0,
    timeoutMilliseconds = 40 * 60_000,
  ) {
    const [code, signal] = await waitForProcessExit(run.child, {
      signal: cancellation.signal,
      timeoutMilliseconds,
    });
    Object.assign(run.entry, {
      exitCode: code,
      signal,
      finishedAt: new Date().toISOString(),
      outcome: code === expected ? "expected" : "failed",
    });
    await save();
    assert.equal(
      code,
      expected,
      `${run.entry.label}: ${run.child.capturedStandardError}`,
    );
  }

  async function probeSentinel(phase) {
    assertProcessRunning(sentinel.child, "neighboring Fixture Mode");
    const window = await runMonitoredProcess(
      "xwininfo",
      ["-name", "RoonScape", "-int"],
      { environment: sentinelEnvironment, signal: cancellation.signal },
    );
    assert.match(window, /Map State: IsViewable/);
    assert.match(window, /Width: 1280\b/);
    assert.match(window, /Height: 720\b/);
    const identity = window.match(/Window id: (\d+)/)?.[1];
    assert.ok(identity);
    if (sentinelWindow) assert.equal(identity, sentinelWindow);
    sentinelWindow = identity;
    report.observations.push({
      phase,
      window: identity,
      at: new Date().toISOString(),
    });
    await save();
  }

  async function save() {
    await writeFile(
      path.join(evidence, "acceptance.json"),
      JSON.stringify(report, null, 2) + "\n",
    );
    await writeFile(
      path.join(evidence, "README.md"),
      `# Two-worktree acceptance\n\nOutcome: **${report.outcome}**. Visual acceptance: **unreviewed**. Live observation: **not performed**.\n\n[Structured observations and commands](acceptance.json)\n\n${report.commands.map((entry) => `- ${entry.label}: ${entry.outcome}; [stdout](${entry.stdout}), [stderr](${entry.stderr})`).join("\n")}\n\n${(report.reviews ?? []).map((directory) => `- [Verification evidence](${path.relative(evidence, directory)}/README.md)`).join("\n")}\n\n${report.error ?? ""}\n\nInspect every completed capture and record agent-authored verdicts separately. This exercise does not establish complete typography, physical-display quality, or a real Live Capture Session.\n`,
    );
  }

  async function inspectCaptures(directory, outcome, scope) {
    const item = await json(path.join(directory, "captures.json"));
    assert.equal(item.outcome, outcome);
    assert.equal(item.scope, scope);
    assert.equal(item.visualAcceptance, "unreviewed");
    assert.equal(item.verdict, undefined);
    assert.equal(
      await realpath(item.source.root),
      await realpath(item.verificationSource.root),
    );
    assert.equal(item.source.revision, item.verificationSource.revision);
    assert.equal(item.source.workingTree, "");
    if (scope === "ci-fallback") {
      assert.equal(item.typography, "packaged fallback only");
      assert.equal(item.requested.length, 4);
      assert.ok(
        item.requested.every((entry) => entry.typography === "fallback"),
      );
    }
    assert.ok(item.rationale);
    const index = await readFile(path.join(directory, "index.html"), "utf8");
    assert.equal(new Set(item.completed).size, item.completed.length);
    if (outcome === "complete")
      assert.equal(item.completed.length, item.requested.length);
    else
      assert.ok(
        item.completed.length > 0 &&
          item.completed.length < item.requested.length,
      );
    for (const name of item.completed) {
      assert.ok(item.requested.some((entry) => entry.fileName === name));
      assert.ok(index.includes(`href="${name}"`));
      const png = await readFile(path.join(directory, name));
      assert.equal(png.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
    }
    await access(path.join(directory, "capture.log"));
  }

  async function presentationDirectory(run) {
    return waitFor(
      () => {
        const match = run.child.capturedStandardOutput.match(
          /Presentation review: (\S+)/,
        );
        assert.ok(match);
        return match[1];
      },
      run.child,
      "presentation review directory",
      { timeoutMilliseconds: 60_000, signal: cancellation.signal },
    );
  }
}

async function json(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function findPresentation(directory) {
  const names = (await readdir(directory)).filter((name) =>
    name.startsWith("presentation."),
  );
  assert.equal(names.length, 1);
  return path.join(directory, names[0]);
}

// Read only PID, parent, and executable name; never collect environments or
// command lines, which can contain personal configuration or credentials.
async function descendants(parent) {
  const rows = (await runMonitoredProcess("ps", ["-eo", "pid=,ppid=,comm="]))
    .trim()
    .split("\n")
    .map((line) => {
      const [pid, ppid, name] = line.trim().split(/\s+/);
      return { pid: Number(pid), parent: Number(ppid), name };
    });
  const owned = new Set([parent]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const row of rows)
      if (owned.has(row.parent) && !owned.has(row.pid)) {
        owned.add(row.pid);
        changed = true;
      }
  }
  return rows.filter((row) => row.pid !== parent && owned.has(row.pid));
}

await main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
