import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import test from "node:test";

test("comparison requires explicit baseline and candidate selections", () => {
  const result = spawnSync(process.execPath, ["scripts/compare-renderer.mjs"], {
    encoding: "utf8",
  });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /--baseline and --candidate are required/);
});

import {
  chmod,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
const execute = promisify(execFile);
const root = process.cwd();

async function fixture(t) {
  const directory = await mkdtemp("/var/tmp/codex/roonscape/task.");
  t.after(() => rm(directory, { recursive: true, force: true }));
  const bin = path.join(directory, "bin");
  const source = path.join(directory, "source");
  await mkdir(bin);
  await mkdir(source);
  await execute("git", ["init", "-q", source]);
  await mkdir(path.join(source, "src/shared/schema"), { recursive: true });
  for (const file of ["presentation-snapshot", "display-configuration"]) {
    await writeFile(
      path.join(source, `src/shared/schema/${file}.schema.json`),
      await readFile(`src/shared/schema/${file}.schema.json`),
    );
  }
  await writeFile(path.join(source, "Cargo.toml"), "[workspace]\nmembers=[]\n");
  for (const relative of ["src/renderer/assets/fonts", "src/desktop/icons"]) {
    await mkdir(path.dirname(path.join(source, relative)), { recursive: true });
    await cp(path.join(root, relative), path.join(source, relative), {
      recursive: true,
    });
  }
  await execute("git", ["-C", source, "add", "."]);
  await execute("git", [
    "-C",
    source,
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.com",
    "commit",
    "-qm",
    "test fixture",
  ]);
  const renderer = `#!/usr/bin/env node\nimport net from 'node:net'; import fs from 'node:fs'; fs.appendFileSync(${JSON.stringify(path.join(directory, "processes.jsonl"))}, JSON.stringify({pid:process.pid,socket:process.env.ROONSCAPE_SOCKET})+'\\n');\nconst socket = net.createConnection(process.env.ROONSCAPE_SOCKET); socket.on('data', () => {}); socket.on('error', () => process.exit(1)); setInterval(() => {}, 1000);\n`;
  const programs = {
    cargo: `#!/usr/bin/env node\nimport fs from 'node:fs';\nif(process.argv.includes('--version')) { console.log('cargo controlled'); } else { if(!process.argv.includes('--release')) process.exit(9); fs.mkdirSync(process.env.CARGO_TARGET_DIR+'/release', {recursive:true}); fs.writeFileSync(process.env.CARGO_TARGET_DIR+'/release/roonscape-renderer', ${JSON.stringify(renderer)}, {mode:0o755}); }\n`,
    Xvfb: "#!/bin/sh\necho 999\nexec sleep 1000\n",
    xwininfo:
      '#!/bin/sh\necho "Width: 1280\nHeight: 720\nMap State: IsViewable"\n',
    python3: `#!/usr/bin/env node\nlet n=0; setInterval(() => { console.log(JSON.stringify({elapsedMs:n*50,cpuSeconds:n*0.01,rssBytes:1000000+n*1000})); n++; },50);\n`,
  };
  for (const [name, body] of Object.entries(programs)) {
    await writeFile(path.join(bin, name), body);
    await chmod(path.join(bin, name), 0o755);
  }
  return {
    directory,
    source,
    bin,
    env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
  };
}

test("compares isolated optimized sources and preserves a dirty candidate", async (t) => {
  const f = await fixture(t);
  await writeFile(path.join(f.source, "uncommitted.txt"), "candidate change");
  const output = path.join(f.directory, "evidence");
  await execute(
    process.execPath,
    [
      path.join(root, "scripts/compare-renderer.mjs"),
      "--baseline",
      "ref:HEAD",
      "--candidate",
      `worktree:${f.source}`,
      "--profile",
      "smoke",
      "--workloads",
      "progress",
      "--output",
      output,
    ],
    { cwd: f.source, env: f.env },
  );
  const report = JSON.parse(
    await readFile(path.join(output, "report.json"), "utf8"),
  );
  assert.equal(report.status, "complete");
  assert.equal(report.sources.baseline.dirty, false);
  assert.equal(report.sources.candidate.dirty, true);
  assert.notEqual(
    report.sources.baseline.contentDigest,
    report.sources.candidate.contentDigest,
  );
  assert.deepEqual(
    report.runs.map((r) => r.side),
    ["baseline", "candidate"],
  );
  assert.deepEqual(report.selectedCoverage, ["progress"]);
  assert.ok(report.omittedCoverage.includes("lyrics"));
  assert.ok(report.runs.every((r) => r.samples.length >= 2));
  assert.ok(report.summaries.progress.cpuSeconds.baseline.mean > 0);
  assert.equal(
    await readFile(path.join(f.source, "uncommitted.txt"), "utf8"),
    "candidate change",
  );
});

async function compareFixture(f, extra = []) {
  const output = path.join(
    f.directory,
    `evidence-${Math.random().toString(16).slice(2)}`,
  );
  let outcome;
  try {
    outcome = await execute(
      process.execPath,
      [
        path.join(root, "scripts/compare-renderer.mjs"),
        "--baseline",
        "ref:HEAD",
        "--candidate",
        `worktree:${f.source}`,
        "--profile",
        "smoke",
        "--workloads",
        "progress",
        "--output",
        output,
        ...extra,
      ],
      { cwd: f.source, env: f.env },
    );
  } catch (error) {
    outcome = error;
  }
  return {
    outcome,
    output,
    report: JSON.parse(
      await readFile(path.join(output, "report.json"), "utf8"),
    ),
  };
}

test("alternates sequential repeats with identical schedules and descriptive memory growth", async (t) => {
  const f = await fixture(t);
  const { report } = await compareFixture(f, ["--repeats", "3"]);
  assert.equal(report.status, "complete");
  assert.deepEqual(
    report.runs.map((run) => run.side),
    ["baseline", "candidate", "candidate", "baseline", "baseline", "candidate"],
  );
  assert.ok(report.runs.every((run) => run.metrics.rssChangeBytes > 0));
  assert.ok(report.runs.every((run) => run.metrics.intervalsMs.length > 0));
  assert.ok(
    report.runs.every(
      (run) =>
        run.publications.filter((p) => p.phase === "measurement").length === 1,
    ),
  );
  assert.equal(report.summaries.progress.peakRssBytes.baseline.n, 3);
  assert.ok(
    Number.isFinite(
      report.summaries.progress.cpuSeconds.baseline.standardDeviation,
    ),
  );
});

test("retains unavailable samples and marks evidence invalid", async (t) => {
  const f = await fixture(t);
  await writeFile(
    path.join(f.bin, "python3"),
    "#!/usr/bin/env node\nlet n=0; setInterval(()=>console.log(JSON.stringify({elapsedMs:n++*50,cpuSeconds:null,rssBytes:null})),50);\n",
  );
  const { report, outcome } = await compareFixture(f);
  assert.equal(outcome.code, 2);
  assert.equal(report.status, "invalid-evidence");
  assert.ok(report.runs[0].samples.length > 1);
  assert.equal(report.summaries.progress.cpuSeconds.baseline.mean, null);
});

test("zero baselines have no relative delta and increases remain advisory", async (t) => {
  const f = await fixture(t);
  await writeFile(
    path.join(f.bin, "python3"),
    `#!/usr/bin/env node\nimport fs from 'node:fs'; const candidate=fs.readFileSync('/proc/'+process.argv[3]+'/cmdline','utf8').includes('/candidate/'); let n=0; setInterval(()=>console.log(JSON.stringify({elapsedMs:n*50,cpuSeconds:candidate?n++*0.1:n++*0,rssBytes:candidate?2000:1000})),50);\n`,
  );
  const { report } = await compareFixture(f);
  assert.equal(report.status, "complete");
  assert.equal(report.summaries.progress.cpuSeconds.baseline.mean, 0);
  assert.equal(report.summaries.progress.cpuSeconds.relativeDeltaPercent, null);
  assert.ok(report.summaries.progress.cpuSeconds.absoluteDelta > 0);
  assert.equal(
    report.summaries.progress.peakRssBytes.relativeDeltaPercent,
    100,
  );
});

test("prerequisite failure preserves a partial report without starting measurements", async (t) => {
  const f = await fixture(t);
  await writeFile(path.join(f.bin, "cargo"), "#!/bin/sh\nexit 7\n");
  const { report, outcome } = await compareFixture(f);
  assert.equal(outcome.code, 1);
  assert.equal(report.status, "execution-failed");
  assert.deepEqual(report.runs, []);
});

test("prepared builds are digest checked and can be reused without compiling", async (t) => {
  const f = await fixture(t);
  const first = await compareFixture(f);
  assert.equal(first.report.status, "complete");
  await writeFile(
    path.join(f.bin, "cargo"),
    '#!/bin/sh\nif [ "$1" = "--version" ]; then echo controlled; else exit 9; fi\n',
  );
  const output = path.join(f.directory, "reused");
  const args = [
    path.join(root, "scripts/compare-renderer.mjs"),
    "--baseline",
    `build:${first.output}/baseline`,
    "--candidate",
    `build:${first.output}/candidate`,
    "--profile",
    "smoke",
    "--workloads",
    "progress",
    "--output",
    output,
  ];
  await execute(process.execPath, args, { env: f.env });
  const report = JSON.parse(
    await readFile(path.join(output, "report.json"), "utf8"),
  );
  assert.equal(report.status, "complete");
  assert.equal(
    report.sources.baseline.build.binaryDigest,
    first.report.sources.baseline.build.binaryDigest,
  );
  await writeFile(
    path.join(first.output, "baseline/target/release/roonscape-renderer"),
    "tampered",
  );
  args[args.length - 1] = path.join(f.directory, "tampered");
  await assert.rejects(
    execute(process.execPath, args, { env: f.env }),
    (error) => error.code === 2,
  );
});

test("cancellation preserves samples and cleans owned processes and sockets", async (t) => {
  const f = await fixture(t);
  const output = path.join(f.directory, "cancelled");
  const neighbor = spawn("sleep", ["60"]);
  t.after(() => neighbor.kill());
  const child = spawn(
    process.execPath,
    [
      path.join(root, "scripts/compare-renderer.mjs"),
      "--baseline",
      "ref:HEAD",
      "--candidate",
      `worktree:${f.source}`,
      "--workloads",
      "progress",
      "--warmup-seconds",
      "0.1",
      "--output",
      output,
    ],
    { cwd: f.source, env: f.env },
  );
  let interrupted = false;
  child.stdout.on("data", (chunk) => {
    if (!interrupted && chunk.toString().includes("Repeat 1")) {
      interrupted = true;
      setTimeout(() => child.kill("SIGTERM"), 1000);
    }
  });
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const code = await new Promise((resolve) => child.once("close", resolve));
  assert.equal(code, 130, stderr);
  const report = JSON.parse(
    await readFile(path.join(output, "report.json"), "utf8"),
  );
  assert.equal(report.status, "cancelled");
  assert.ok(report.runs[0].samples.length > 0);
  assert.equal(neighbor.exitCode, null);
  for (const line of (
    await readFile(path.join(f.directory, "processes.jsonl"), "utf8")
  )
    .trim()
    .split("\n")) {
    const process_ = JSON.parse(line);
    assert.throws(() => process.kill(process_.pid, 0), { code: "ESRCH" });
    await assert.rejects(readFile(process_.socket), { code: "ENOENT" });
  }
});

test("real Renderer consumes synthetic workloads while resources reach reports", async (t) => {
  const f = await fixture(t);
  // Control only compilation in this small integration exercise: normal verification
  // already built this native executable. Full comparisons independently build release.
  await writeFile(
    path.join(f.bin, "cargo"),
    `#!/usr/bin/env node\nimport fs from 'node:fs'; if(process.argv.includes('--version')) console.log('controlled integration build'); else { fs.mkdirSync(process.env.CARGO_TARGET_DIR+'/release',{recursive:true}); fs.copyFileSync(${JSON.stringify(path.join(root, "target/debug/roonscape-renderer"))},process.env.CARGO_TARGET_DIR+'/release/roonscape-renderer'); }\n`,
  );
  for (const name of ["Xvfb", "xwininfo", "python3"])
    await rm(path.join(f.bin, name));
  const { report, outcome } = await compareFixture(f, [
    "--measurement-seconds",
    "0.8",
  ]);
  assert.equal(report.status, "complete", outcome.stderr);
  assert.ok(report.runs.every((run) => run.metrics.peakRssBytes > 0));
  assert.ok(report.runs.every((run) => run.publications.length >= 3));
});

test("interrupted replacements keep identical elapsed-time publications on both sides", async (t) => {
  const f = await fixture(t);
  const output = path.join(f.directory, "replacements");
  await execute(
    process.execPath,
    [
      path.join(root, "scripts/compare-renderer.mjs"),
      "--baseline",
      "ref:HEAD",
      "--candidate",
      `worktree:${f.source}`,
      "--profile",
      "smoke",
      "--workloads",
      "replacements",
      "--measurement-seconds",
      "1.4",
      "--output",
      output,
    ],
    { cwd: f.source, env: f.env },
  );
  const report = JSON.parse(
    await readFile(path.join(output, "report.json"), "utf8"),
  );
  const schedules = report.runs.map((run) =>
    run.publications
      .filter((p) => p.phase === "measurement")
      .map((p) => p.plannedMs),
  );
  assert.deepEqual(schedules, [
    [0, 1000, 1100, 1200],
    [0, 1000, 1100, 1200],
  ]);
  assert.ok(
    report.omittedCoverage.includes("full workload cycles (short measurement)"),
  );
});

test("decreases remain descriptive and single repeats have no variation estimate", async (t) => {
  const f = await fixture(t);
  await writeFile(
    path.join(f.bin, "python3"),
    `#!/usr/bin/env node\nimport fs from 'node:fs'; const candidate=fs.readFileSync('/proc/'+process.argv[3]+'/cmdline','utf8').includes('/candidate/'); let n=0; setInterval(()=>console.log(JSON.stringify({elapsedMs:n*50,cpuSeconds:n++*(candidate?0.01:0.02),rssBytes:candidate?1000:2000})),50);\n`,
  );
  const { report } = await compareFixture(f);
  assert.equal(report.status, "complete");
  assert.equal(
    report.summaries.progress.peakRssBytes.relativeDeltaPercent,
    -50,
  );
  assert.equal(
    report.summaries.progress.peakRssBytes.baseline.standardDeviation,
    null,
  );
  assert.ok(report.summaries.progress.cpuSeconds.absoluteDelta < 0);
});

test("incompatible source contracts are rejected before measurement", async (t) => {
  const f = await fixture(t);
  await writeFile(
    path.join(f.source, "src/shared/schema/presentation-snapshot.schema.json"),
    "{}",
  );
  const { report, outcome } = await compareFixture(f);
  assert.equal(outcome.code, 2);
  assert.equal(report.status, "invalid-evidence");
  assert.deepEqual(report.runs, []);
  assert.match(
    report.error,
    /incompatible candidate presentation-snapshot schema/,
  );
});

test(
  "failed evidence publication still closes the native session",
  { timeout: 15000 },
  async (t) => {
    const f = await fixture(t);
    const output = path.join(f.directory, "write-failure");
    const child = spawn(
      process.execPath,
      [
        path.join(root, "scripts/compare-renderer.mjs"),
        "--baseline",
        "ref:HEAD",
        "--candidate",
        `worktree:${f.source}`,
        "--profile",
        "smoke",
        "--workloads",
        "progress",
        "--output",
        output,
      ],
      { cwd: f.source, env: f.env },
    );
    t.after(() => child.kill());
    let obstructed;
    child.stdout.on("data", (chunk) => {
      if (!obstructed && chunk.toString().includes("Repeat 1"))
        obstructed = mkdir(path.join(output, "run-1.log"));
    });
    let stderr = "";
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    const code = await new Promise((resolve) => child.once("close", resolve));
    await obstructed;
    assert.equal(code, 1, stderr);
    const report = JSON.parse(
      await readFile(path.join(output, "report.json"), "utf8"),
    );
    assert.equal(report.status, "execution-failed");
    assert.match(report.error, /EISDIR/);
    for (const line of (
      await readFile(path.join(f.directory, "processes.jsonl"), "utf8")
    )
      .trim()
      .split("\n")) {
      const owned = JSON.parse(line);
      assert.throws(() => process.kill(owned.pid, 0), { code: "ESRCH" });
      await assert.rejects(readFile(owned.socket), { code: "ENOENT" });
    }
  },
);
