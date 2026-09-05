import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { chmod, stat } from "node:fs/promises";
import {
  startMonitoredProcess,
  stopProcess,
  waitFor,
  waitForProcessExit,
} from "./process-harness.mjs";

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import test from "node:test";

const execute = promisify(execFile);
const cli = new URL("./presentation-review.mjs", import.meta.url).pathname;

test("review lists maintained focused, complete, and CI fallback coverage", async () => {
  const { stdout } = await execute(process.execPath, [cli, "--list"]);
  const scopes = JSON.parse(stdout);
  assert.equal(scopes.focused.viewports.length, 7);
  assert.ok(scopes.focused.scenarios.includes("paused"));
  assert.ok(scopes.complete.requested > scopes.focused.scenarios.length * 7);
  assert.equal(scopes["ci-fallback"].requested, 4);
  assert.equal(scopes["ci-fallback"].typography, "packaged fallback only");
});

async function reviewDirectory(context) {
  const root = "/var/tmp/codex/roonscape";
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(path.join(root, "task.review-test."));
  context.after(() => rm(directory, { recursive: true, force: true }));
  await writeFile(
    path.join(directory, "verification.json"),
    JSON.stringify({
      outcome: "complete",
      source: { revision: "test-revision" },
    }),
  );
  await writeFile(path.join(directory, "README.md"), "# Verification\n");
  return directory;
}

async function capture(context, scope, extra = [], environment = process.env) {
  const review = await reviewDirectory(context);
  let result;
  try {
    result = await execute(
      process.execPath,
      [
        cli,
        "--review",
        review,
        "--scope",
        scope,
        "--rationale",
        "Exercise the affected presentation",
        ...extra,
      ],
      { env: environment },
    );
  } catch (error) {
    result = error;
  }
  const directory = result.stdout?.match(/Presentation review: (.+)/)?.[1];
  assert.ok(directory, result.stderr);
  const report = JSON.parse(
    await readFile(path.join(directory, "captures.json"), "utf8"),
  );
  return { review, directory, report, result };
}

test("focused review publishes seven native captures with inspectable links and separate acceptance", async (context) => {
  const { review, directory, report, result } = await capture(
    context,
    "focused",
    ["--scenario", "idle"],
  );
  assert.equal(result.code, undefined, result.stderr);
  assert.equal(report.outcome, "complete");
  assert.equal(report.requested.length, 7);
  assert.equal(report.completed.length, 7);
  assert.equal(report.visualAcceptance, "unreviewed");
  assert.equal(report.rationale, "Exercise the affected presentation");
  assert.equal(report.verificationSource.revision, "test-revision");
  const index = await readFile(path.join(directory, "index.html"), "utf8");
  for (const filename of report.completed) {
    assert.ok(index.includes(`href="${filename}"`));
    assert.ok((await readFile(path.join(directory, filename))).length > 1000);
  }
  assert.match(
    await readFile(path.join(review, "README.md"), "utf8"),
    /presentation\..+\/index.html/,
  );
  const verdictFile = path.join(directory, "input.json");
  const verdict = {
    verdict: "accepted",
    reasons: "Every viewport inspected",
    inspected: report.completed,
    unresolved: ["Check the physical display"],
  };
  await writeFile(verdictFile, JSON.stringify(verdict));
  await assert.rejects(
    execute(process.execPath, [
      cli,
      "--record",
      directory,
      "--verdict-file",
      verdictFile,
    ]),
    (error) => /outstanding judgments/.test(error.stderr),
  );
  verdict.unresolved = [];
  await writeFile(verdictFile, JSON.stringify(verdict));
  await execute(process.execPath, [
    cli,
    "--record",
    directory,
    "--verdict-file",
    verdictFile,
  ]);
  const saved = JSON.parse(
    await readFile(path.join(directory, "verdict.json"), "utf8"),
  );
  assert.equal(saved.verdict, "accepted");
  assert.equal(saved.completeProfileAccepted, false);
  await assert.rejects(
    execute(process.execPath, [
      cli,
      "--record",
      directory,
      "--verdict-file",
      verdictFile,
    ]),
    (error) => /EEXIST/.test(error.stderr),
  );
});

test("CI scope forces packaged fonts and retains four linked captures without visual acceptance", async (context) => {
  const { report, result } = await capture(context, "ci-fallback");
  assert.equal(result.code, undefined, result.stderr);
  assert.equal(report.requested.length, 4);
  assert.equal(report.completed.length, 4);
  assert.equal(report.typography, "packaged fallback only");
  assert.ok(report.fontInventory.families.includes("Libre Baskerville"));
  assert.ok(!report.fontInventory.families.includes("Palatino Linotype"));
  assert.ok(!report.fontInventory.families.includes("Sitka Display"));
  assert.equal(report.visualAcceptance, "unreviewed");
});

test("complete scope fails explicitly without required fonts and retains requested coverage", async (context) => {
  const directory = await reviewDirectory(context);
  const config = path.join(directory, "fonts.conf");
  await writeFile(config, '<?xml version="1.0"?><fontconfig></fontconfig>');
  const { report, result } = await capture(context, "complete", [], {
    ...process.env,
    FONTCONFIG_FILE: config,
    FONTCONFIG_PATH: directory,
  });
  assert.equal(result.code, 1);
  assert.match(
    report.error,
    /Complete profile requires host fonts: Sitka Display/,
  );
  assert.equal(report.outcome, "failed");
  assert.ok(report.requested.length > 200);
  assert.equal(report.completed.length, 0);
});

test("visual verdict requires complete inspected coverage and retains reasons independently", async (context) => {
  const { directory, report } = await capture(context, "ci-fallback");
  const verdictFile = path.join(directory, "input.json");
  const verdict = {
    verdict: "accepted",
    reasons: "All requested compositions are legible",
    inspected: [],
    unresolved: [],
  };
  await writeFile(verdictFile, JSON.stringify(verdict));
  await assert.rejects(
    execute(process.execPath, [
      cli,
      "--record",
      directory,
      "--verdict-file",
      verdictFile,
    ]),
    (error) => /every requested image/.test(error.stderr),
  );
  verdict.inspected = report.completed;
  await writeFile(verdictFile, JSON.stringify(verdict));
  // CI scope cannot make an aesthetic acceptance claim even after inspection.
  await assert.rejects(
    execute(process.execPath, [
      cli,
      "--record",
      directory,
      "--verdict-file",
      verdictFile,
    ]),
    (error) => /CI fallback/.test(error.stderr),
  );
  verdict.verdict = "needs-work";
  verdict.unresolved = ["Physical display contrast requires human review"];
  await writeFile(verdictFile, JSON.stringify(verdict));
  await execute(process.execPath, [
    cli,
    "--record",
    directory,
    "--verdict-file",
    verdictFile,
  ]);
  const saved = JSON.parse(
    await readFile(path.join(directory, "verdict.json"), "utf8"),
  );
  assert.equal(saved.verdict, "needs-work");
  assert.deepEqual(saved.unresolved, verdict.unresolved);
  assert.match(
    await readFile(path.join(directory, "index.html"), "utf8"),
    /verdict.json/,
  );
  assert.equal(
    JSON.parse(await readFile(path.join(directory, "captures.json"), "utf8"))
      .outcome,
    "complete",
  );
});

async function failingCaptureEnvironment(context, mode) {
  const directory = await reviewDirectory(context);
  const executable = path.join(directory, "scrot");
  await writeFile(
    executable,
    `#!${process.execPath}
const fs = require('node:fs');
const marker = ${JSON.stringify(path.join(directory, "called"))};
if (!fs.existsSync(marker)) {
  fs.writeFileSync(marker, '1');
  require('node:child_process').execFileSync('/usr/bin/scrot', process.argv.slice(2));
} else {
  fs.writeFileSync(${JSON.stringify(path.join(directory, "waiting"))}, String(process.pid));
  ${mode === "fail" ? "console.error('deliberate second capture failure'); process.exitCode = 7;" : "setInterval(() => {}, 1000);"}
}
`,
  );
  await chmod(executable, 0o755);
  return {
    directory,
    environment: { ...process.env, PATH: `${directory}:${process.env.PATH}` },
  };
}

test("partial publication retains the first image, diagnostics, and denies acceptance", async (context) => {
  const { environment } = await failingCaptureEnvironment(context, "fail");
  const { directory, report, result } = await capture(
    context,
    "ci-fallback",
    [],
    environment,
  );
  assert.equal(result.code, 1);
  assert.equal(report.outcome, "failed");
  assert.equal(report.requested.length, 4);
  assert.equal(report.completed.length, 1);
  await stat(path.join(directory, report.completed[0]));
  assert.match(
    await readFile(path.join(directory, "capture.log"), "utf8"),
    /deliberate second capture failure/,
  );
  const verdictFile = path.join(directory, "attempt.json");
  await writeFile(
    verdictFile,
    JSON.stringify({
      verdict: "accepted",
      reasons: "First image looks correct",
      inspected: report.completed,
      unresolved: [],
    }),
  );
  await assert.rejects(
    execute(process.execPath, [
      cli,
      "--record",
      directory,
      "--verdict-file",
      verdictFile,
    ]),
    (error) => /complete set/.test(error.stderr),
  );
});

test("cancelled review preserves partial images beside a concurrent successful review", async (context) => {
  const review = await reviewDirectory(context);
  const { environment, directory: fixture } = await failingCaptureEnvironment(
    context,
    "wait",
  );
  const args = [
    cli,
    "--review",
    review,
    "--scope",
    "ci-fallback",
    "--rationale",
    "Cancellation evidence",
  ];
  const child = startMonitoredProcess(process.execPath, args, { environment });
  context.after(() => stopProcess(child));
  await child.spawned;
  const neighbor = execute(process.execPath, args);
  // Attach a rejection handler immediately while waiting for the blocked capture.
  const neighborResult = neighbor.then(
    (result) => result,
    (error) => error,
  );
  await waitFor(
    () => readFile(path.join(fixture, "waiting"), "utf8"),
    child,
    "second native capture",
  );
  child.kill("SIGTERM");
  assert.equal(
    (await waitForProcessExit(child, { timeoutMilliseconds: 10000 }))[0],
    130,
  );
  const cancelled = child.capturedStandardOutput.match(
    /Presentation review: (.+)/,
  )[1];
  const report = JSON.parse(
    await readFile(path.join(cancelled, "captures.json"), "utf8"),
  );
  assert.equal(report.outcome, "cancelled");
  assert.equal(report.completed.length, 1);
  await stat(path.join(cancelled, report.completed[0]));
  const successful = await neighborResult;
  assert.equal(successful.code, undefined, successful.stderr);
  const completed = successful.stdout.match(/Presentation review: (.+)/)[1];
  assert.notEqual(cancelled, completed);
  assert.equal(
    JSON.parse(await readFile(path.join(completed, "captures.json"), "utf8"))
      .completed.length,
    4,
  );
});
