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
} from "node:fs/promises";
import path from "node:path";
import { createAcceptanceBudget } from "./acceptance-budget.mjs";
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
      "Usage: npm run accept:worktrees -- /absolute/worktree-a /absolute/worktree-b\nRequires two fresh, existing, clean worktrees on a provisioned Linux host. Leaves failure diagnostics under /var/tmp/codex/roonscape.",
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
  // Short runtime path for Unix sockets.
  const runtime = await mkdtemp("/tmp/rs-a.");
  const cancellation = processCancellation();
  const budget = createAcceptanceBudget({ signal: cancellation.signal });
  const children = [];
  const streams = [];
  let outcome = "incomplete";
  let sentinelRuntime;
  console.log(`Acceptance diagnostics: ${evidence}`);
  let sentinel;
  let sentinelEnvironment;
  let sentinelWindow;
  try {
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
      budget.waitOptions(20 * 60_000),
    );
    sentinelEnvironment = {
      ...process.env,
      DISPLAY: ready[1],
      XAUTHORITY: "/dev/null",
    };
    sentinelRuntime = ready[2];
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
          budget.waitOptions(30_000),
        ),
      ),
    );
    assert.notEqual(...reviewPaths);
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
      budget.waitOptions(60_000),
    );
    assert.notEqual(
      nativeReports[0].runtimeDirectory,
      nativeReports[1].runtimeDirectory,
    );
    assert.notEqual(nativeReports[0].runtimeDirectory, sentinelRuntime);
    assert.notEqual(nativeReports[1].runtimeDirectory, sentinelRuntime);
    await probeSentinel("during verification");
    await Promise.all(verification.map((run) => complete(run)));
    for (const [index, directory] of reviewPaths.entries()) {
      await assert.rejects(access(directory), { code: "ENOENT" });
      const item = nativeReports[index];
      for (const owned of [item.runtimeDirectory, item.commandRuntimeDirectory])
        await assert.rejects(access(owned), { code: "ENOENT" });
      assert.equal(
        (
          await runMonitoredProcess(
            "git",
            ["status", "--porcelain=v1", "--untracked-files=all"],
            {
              cwd: roots[index],
              ...budget.waitOptions(5_000),
            },
          )
        ).trim(),
        "",
        "verification leaves the source worktree clean",
      );
    }
    await probeSentinel("after verification");

    // Both commands use the maintained focused scope at all seven viewports.
    // More scenarios in A keep useful native work running while B is cancelled.
    const reviewArgs = (scenarios) => [
      "run",
      "review:presentations:built",
      "--",
      "--output",
      evidence,
      "--scope",
      "focused",
      ...scenarios.flatMap((scenario) => ["--scenario", scenario]),
    ];
    const survivor = start(
      roots[0],
      environments[0],
      "capture-survivor",
      "npm",
      reviewArgs(["playing", "idle", "long-metadata", "light-artwork"]),
    );
    const survivorDirectory = await presentationDirectory(survivor);
    const victim = start(
      roots[1],
      environments[1],
      "capture-cancelled",
      "npm",
      reviewArgs(["playing", "idle"]),
    );
    const victimDirectory = await presentationDirectory(victim);
    const owned = await waitFor(
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
        return owned;
      },
      victim.child,
      "published capture and overlapping native review work",
      {
        retryMilliseconds: 10,
        ...budget.waitOptions(60_000),
      },
    );
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
    await probeSentinel("after surviving capture completion");
    budget.waitOptions();
    outcome = "complete";
  } catch (error) {
    outcome = cancellation.signal.aborted ? "cancelled" : "failed";
    console.error(error.stack);
    process.exitCode = 1;
  } finally {
    budget.dispose();
    try {
      try {
        if (sentinelRuntime) {
          const owned = await descendants(sentinel.child.pid);
          const renderer = owned.find((entry) =>
            entry.name.startsWith("roonscape"),
          );
          if (renderer) {
            assert.ok(owned.some((entry) => entry.pid === renderer.parent));
            // Let the Fixture CLI remove its runtime before the process monitor
            // stops npm's group; npm can exit before its children finish.
            process.kill(renderer.parent, "SIGTERM");
            await waitForProcessExit(sentinel.child, {
              timeoutMilliseconds: 5_000,
            });
          }
        }
      } finally {
        await stopProcesses(children, { graceMilliseconds: 5_000 });
      }
      if (sentinelRuntime)
        await assert.rejects(access(sentinelRuntime), {
          code: "ENOENT",
        });
      // Check before deleting our containing runtime, so leaked child resources
      // cannot be hidden by the exercise's own cleanup.
      for (const entry of await readdir(runtime))
        assert.deepEqual(await readdir(path.join(runtime, entry)), []);
      await rm(runtime, { recursive: true });
    } catch (error) {
      console.error(error.stack);
      outcome = "failed";
      process.exitCode = 1;
    }
    for (const stream of streams)
      await new Promise((resolve) => stream.end(resolve));
    if (cancellation.signal.aborted) {
      outcome = "cancelled";
      process.exitCode = 130;
    }
    cancellation.dispose();
    if (outcome === "complete") await rm(evidence, { recursive: true });
  }
  console.log(
    `Acceptance: ${outcome}${outcome === "complete" ? "" : `; diagnostics: ${evidence}`}`,
  );

  function start(cwd, environment, label, command, arguments_) {
    budget.waitOptions();
    const child = startMonitoredProcess(command, arguments_, {
      cwd,
      environment,
    });
    children.push(child);
    for (const [source, name] of [
      [child.stdout, `${label}.stdout.log`],
      [child.stderr, `${label}.stderr.log`],
    ]) {
      const stream = createWriteStream(path.join(evidence, name), {
        flags: "wx",
        mode: 0o600,
      });
      streams.push(stream);
      source.pipe(stream, { end: false });
    }
    console.log(`${label}: ${command} ${arguments_.join(" ")}`);
    return { child, label };
  }

  async function complete(run, expected = 0, timeoutMilliseconds) {
    const [code] = await waitForProcessExit(
      run.child,
      budget.waitOptions(timeoutMilliseconds),
    );
    assert.equal(
      code,
      expected,
      `${run.label}: ${run.child.capturedStandardError}`,
    );
  }

  async function probeSentinel(phase) {
    assertProcessRunning(sentinel.child, "neighboring Fixture Mode");
    const window = await runMonitoredProcess(
      "xwininfo",
      ["-name", "RoonScape", "-int"],
      { environment: sentinelEnvironment, ...budget.waitOptions(5_000) },
    );
    assert.match(window, /Map State: IsViewable/);
    assert.match(window, /Width: 1280\b/);
    assert.match(window, /Height: 720\b/);
    const identity = window.match(/Window id: (\d+)/)?.[1];
    assert.ok(identity);
    if (sentinelWindow) assert.equal(identity, sentinelWindow);
    sentinelWindow = identity;
    console.log(`Sentinel: ${phase}; window ${identity}`);
  }

  async function inspectCaptures(directory, outcome, scope) {
    const item = await json(path.join(directory, "captures.json"));
    assert.equal(item.outcome, outcome);
    assert.equal(item.scope, scope);
    if (scope === "ci-fallback") {
      assert.equal(item.typography, "packaged fallback only");
      assert.equal(item.requested.length, 4);
      assert.ok(
        item.requested.every((entry) => entry.typography === "fallback"),
      );
    }
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
      budget.waitOptions(60_000),
    );
  }
}

async function json(file) {
  return JSON.parse(await readFile(file, "utf8"));
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
