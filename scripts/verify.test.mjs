import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

import { createNativeSession } from "./native-session.mjs";
import {
  startMonitoredProcess,
  stopProcess,
  waitFor,
  waitForProcessExit,
} from "./process-harness.mjs";

const execute = promisify(execFile);
const scratchRoot = "/var/tmp/codex/roonscape";

async function worktree(context) {
  await mkdir(scratchRoot, { recursive: true });
  const directory = await mkdtemp(path.join(scratchRoot, "task.verify-test."));
  context.after(() => rm(directory, { recursive: true, force: true }));
  await mkdir(path.join(directory, "scripts"));
  await mkdir(path.join(directory, "bin"));
  for (const name of [
    "verify.mjs",
    "verification-process.py",
    "native-session.mjs",
    "process-harness.mjs",
  ]) {
    await copyFile(
      new URL(name, import.meta.url),
      path.join(directory, "scripts", name),
    );
  }
  await writeFile(path.join(directory, "tracked.txt"), "source\n");
  for (const args of [
    ["init", "-q"],
    ["add", "."],
    [
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "test: seed worktree",
    ],
  ]) {
    await execute("git", args, { cwd: directory });
  }
  await writeFile(path.join(directory, "tracked.txt"), "changed\n");
  const npm = path.join(directory, "bin/npm");
  await writeFile(
    npm,
    `#!${process.execPath}\nconsole.log('command output: ' + process.argv.slice(2).join(' '));\n`,
  );
  await chmod(npm, 0o755);
  return {
    directory,
    environment: {
      ...process.env,
      PATH: `${path.join(directory, "bin")}:${process.env.PATH}`,
    },
  };
}

test("verification retains source identity, command logs and automated-only completion", async (context) => {
  const fixture = await worktree(context);
  const { stdout } = await execute(
    process.execPath,
    [path.join(fixture.directory, "scripts/verify.mjs"), "--design"],
    { env: fixture.environment },
  );
  const review = stdout.match(/Review directory: (.+)/)?.[1];
  assert.ok(review, stdout);
  context.after(() => rm(review, { recursive: true, force: true }));
  assert.equal(path.dirname(review), scratchRoot);
  const report = JSON.parse(
    await readFile(path.join(review, "verification.json"), "utf8"),
  );
  assert.equal(report.outcome, "complete");
  assert.equal(report.source.root, fixture.directory);
  assert.match(report.source.revision, /^[a-f0-9]{40}$/);
  assert.match(report.source.workingTree, /M tracked.txt/);
  assert.deepEqual(
    report.commands.map((command) => command.arguments),
    [
      ["run", "dev:diagnose", "--", "--evidence", review],
      ["run", "check"],
      ["run", "test:design"],
    ],
  );
  assert.ok(report.commands.every((command) => command.exitCode === 0));
  assert.match(
    await readFile(path.join(review, report.commands[1].stdout), "utf8"),
    /command output: run check/,
  );
  assert.match(
    await readFile(path.join(review, "README.md"), "utf8"),
    /does not establish capture completion or visual acceptance/,
  );
  await assert.rejects(
    readFile(path.join(report.runtimeDirectory, "display.json")),
    { code: "ENOENT" },
  );
});

test("a failed check preserves diagnostics and a later run uses a new directory", async (context) => {
  const fixture = await worktree(context);
  await writeFile(
    path.join(fixture.directory, "bin/npm"),
    `#!${process.execPath}\nconsole.log('completed diagnostic'); if (process.argv[3] === 'check') { console.error('check failure'); process.exitCode = 7; }\n`,
  );
  const reviews = [];
  context.after(() =>
    Promise.all(
      reviews.map((review) => rm(review, { recursive: true, force: true })),
    ),
  );
  for (let index = 0; index < 2; index++) {
    let failure;
    try {
      await execute(
        process.execPath,
        [path.join(fixture.directory, "scripts/verify.mjs"), "--design"],
        { env: fixture.environment },
      );
    } catch (error) {
      failure = error;
    }
    assert.equal(failure?.code, 1);
    const review = failure.stdout.match(/Review directory: (.+)/)[1];
    reviews.push(review);
    const report = JSON.parse(
      await readFile(path.join(review, "verification.json"), "utf8"),
    );
    assert.equal(report.outcome, "failed");
    assert.equal(report.commands.length, 2);
    assert.equal(report.commands[1].exitCode, 7);
    assert.match(
      await readFile(path.join(review, report.commands[1].stderr), "utf8"),
      /check failure/,
    );
    await assert.rejects(
      readFile(path.join(report.runtimeDirectory, "display.json")),
      { code: "ENOENT" },
    );
  }
  assert.notEqual(reviews[0], reviews[1]);
  assert.match(
    await readFile(path.join(reviews[0], "README.md"), "utf8"),
    /failed/,
  );
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  test(`verification retains cancelled outcomes on ${signal} and leaves a neighboring session alive`, async (context) => {
    const fixture = await worktree(context);
    await writeFile(
      path.join(fixture.directory, "bin/npm"),
      `#!${process.execPath}
if (process.argv[3] === 'check') {
  import(${JSON.stringify(path.join(fixture.directory, "scripts/native-session.mjs"))}).then(async ({ createNativeSession }) => {
    const nested = await createNativeSession({ width: 640, height: 480 });
    const probe = nested.startProcess(process.execPath, ['-e', 'setInterval(() => {}, 1000)']);
    await probe.spawned;
    const pids = require('fs').readFileSync('/proc/self/task/' + process.pid + '/children', 'utf8').trim().split(/\\s+/).map(Number);
    console.log('nested ' + JSON.stringify({ runtime: nested.runtimeDirectory, pids }));
    console.log('waiting ' + process.pid);
    setInterval(() => {}, 1000);
  });
}
`,
    );
    const neighbor = await createNativeSession({ width: 640, height: 480 });
    context.after(() => neighbor.close());
    const neighborProcess = neighbor.startProcess(process.execPath, [
      "-e",
      "setInterval(() => {}, 1000)",
    ]);
    await neighborProcess.spawned;
    const child = startMonitoredProcess(
      process.execPath,
      [path.join(fixture.directory, "scripts/verify.mjs")],
      { environment: fixture.environment },
    );
    context.after(() => stopProcess(child));
    await child.spawned;
    const review = await waitFor(
      () => {
        const match = child.capturedStandardOutput.match(
          /Review directory: (.+)/,
        );
        if (!match) throw new Error("waiting for review directory");
        return match[1];
      },
      child,
      "review directory",
    );
    context.after(() => rm(review, { recursive: true, force: true }));
    const pid = await waitFor(
      async () => {
        const output = await readFile(
          path.join(review, "2.stdout.log"),
          "utf8",
        );
        const match = output.match(/waiting (\d+)/);
        if (!match) throw new Error("waiting for command startup");
        return Number(match[1]);
      },
      child,
      "running check",
    );
    const nested = JSON.parse(
      (await readFile(path.join(review, "2.stdout.log"), "utf8")).match(
        /nested (.+)/,
      )[1],
    );
    context.after(async () => {
      for (const pid of nested.pids) {
        try {
          process.kill(-pid, "SIGKILL");
        } catch (error) {
          if (error.code !== "ESRCH") throw error;
        }
      }
      await rm(nested.runtime, { recursive: true, force: true });
    });
    child.kill(signal);
    const [code] = await waitForProcessExit(child, {
      timeoutMilliseconds: 10000,
    });
    assert.equal(code, 130, child.capturedStandardError);
    const report = JSON.parse(
      await readFile(path.join(review, "verification.json"), "utf8"),
    );
    assert.equal(report.outcome, "cancelled");
    assert.equal(report.commands[0].outcome, "complete");
    assert.equal(report.commands[1].outcome, "cancelled");
    assert.equal(report.commands[1].exitCode, 130);
    assert.match(
      await readFile(path.join(review, "2.stdout.log"), "utf8"),
      /waiting/,
    );
    assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
    await assert.rejects(
      readFile(path.join(report.runtimeDirectory, "display.json")),
      { code: "ENOENT" },
    );
    for (const pid of nested.pids)
      assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
    await assert.rejects(readFile(path.join(nested.runtime, "display.json")), {
      code: "ENOENT",
    });
    process.kill(neighborProcess.pid, 0);
    await readFile(neighbor.configurationPath);
    const probe = neighbor.startProcess("xwininfo", ["-root"]);
    await probe.spawned;
    assert.equal(
      (await waitForProcessExit(probe, { timeoutMilliseconds: 5000 }))[0],
      0,
    );
  });
}

test("an evidence write failure still stops the command and removes owned runtime", async (context) => {
  const fixture = await worktree(context);
  const preload = path.join(fixture.directory, "fail-log.cjs");
  await writeFile(
    preload,
    `
const fs = require('node:fs');
const original = fs.writeSync;
fs.writeSync = function (descriptor, data, ...args) {
  if (String(data).includes('simulate evidence failure')) {
    throw Object.assign(new Error('simulated ENOSPC writing evidence'), { code: 'ENOSPC' });
  }
  return original.call(this, descriptor, data, ...args);
};
require('node:module').syncBuiltinESMExports();
`,
  );
  await writeFile(
    path.join(fixture.directory, "bin/npm"),
    `#!${process.execPath}
if (process.argv[3] === 'check') {
  console.error('command pid ' + process.pid);
  setTimeout(() => console.log('simulate evidence failure'), 100);
  setInterval(() => {}, 1000);
}
`,
  );
  let failure;
  try {
    await execute(
      process.execPath,
      [
        "--require",
        preload,
        path.join(fixture.directory, "scripts/verify.mjs"),
      ],
      { env: fixture.environment },
    );
  } catch (error) {
    failure = error;
  }
  assert.equal(failure?.code, 1);
  const review = failure.stdout.match(/Review directory: (.+)/)[1];
  context.after(() => rm(review, { recursive: true, force: true }));
  const report = JSON.parse(
    await readFile(path.join(review, "verification.json"), "utf8"),
  );
  assert.equal(report.outcome, "failed");
  assert.match(report.commands[1].error, /ENOSPC/);
  const pid = Number(
    (await readFile(path.join(review, "2.stderr.log"), "utf8")).match(
      /command pid (\d+)/,
    )[1],
  );
  assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
  await assert.rejects(
    readFile(path.join(report.runtimeDirectory, "display.json")),
    { code: "ENOENT" },
  );
  await assert.rejects(stat(report.commandRuntimeDirectory), {
    code: "ENOENT",
  });
});

test("CI presentation verification runs design and retains failed capture diagnostics", async (context) => {
  const fixture = await worktree(context);
  await writeFile(
    path.join(fixture.directory, "bin/npm"),
    `#!${process.execPath}
console.log('command output: ' + process.argv.slice(2).join(' '));
if (process.argv[3] === 'review:presentations:built') {
  console.error('native capture generation failed after partial publication');
  process.exitCode = 9;
}
`,
  );
  let failure;
  try {
    await execute(
      process.execPath,
      [path.join(fixture.directory, "scripts/verify.mjs"), "--presentation-ci"],
      { env: fixture.environment },
    );
  } catch (error) {
    failure = error;
  }
  const review = failure?.stdout.match(/Review directory: (.+)/)?.[1];
  assert.ok(review, failure?.stderr);
  context.after(() => rm(review, { recursive: true, force: true }));
  const report = JSON.parse(
    await readFile(path.join(review, "verification.json"), "utf8"),
  );
  assert.equal(report.outcome, "failed");
  assert.equal(report.automatedOutcome, "complete");
  assert.equal(report.captureCompletion, "failed");
  assert.match(
    failure.stdout,
    /Workflow: failed; automated checks: complete; captures: failed/,
  );
  assert.equal(report.visualAcceptance, "not assessed");
  assert.deepEqual(report.commands[2].arguments, ["run", "test:design"]);
  assert.ok(report.commands[3].arguments.includes("ci-fallback"));
  assert.equal(report.commands[3].exitCode, 9);
  assert.match(
    await readFile(path.join(review, report.commands[3].stderr), "utf8"),
    /partial publication/,
  );
});
