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
  utimes,
  symlink,
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

async function compareFixture(f, extra = [], workload = "progress") {
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
        workload,
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
  await mkdir(path.join(f.source, "src/renderer/src"), { recursive: true });
  await cp(
    path.join(root, "src/renderer/src/content_evidence.rs"),
    path.join(f.source, "src/renderer/src/content_evidence.rs"),
  );
  const { report, outcome } = await compareFixture(
    f,
    ["--measurement-seconds", "3"],
    "fresh-content",
  );
  assert.equal(report.status, "complete", outcome.stderr);
  assert.ok(
    report.runs
      .filter((run) => run.kind === "resources")
      .every((run) => run.metrics.peakRssBytes > 0),
  );
  assert.ok(
    report.runs
      .filter((run) => run.kind === "resources")
      .every((run) => run.publications.length >= 3),
  );
  const diagnostic = report.runs.find(
    (run) => run.side === "candidate" && run.kind === "content",
  );
  assert.equal(diagnostic.content.status, "complete");
  assert.ok(
    diagnostic.content.publications.some(
      (publication) =>
        ["ready", "delayed"].includes(publication.outcome) && publication.fresh,
    ),
    JSON.stringify(diagnostic.content.publications),
  );
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

test("fresh content has distinct measured identities and a separate diagnostic pass", async (t) => {
  const f = await fixture(t);
  const { report, outcome } = await compareFixture(f, [], "fresh-content");
  assert.equal(report.status, "complete", outcome.stderr);
  assert.deepEqual(report.selectedCoverage, ["fresh-content"]);
  assert.equal(report.runs.filter((run) => run.kind === "resources").length, 2);
  assert.equal(report.runs.filter((run) => run.kind === "content").length, 2);
  const measured = report.runs[0].publications.filter(
    (p) => p.phase === "measurement",
  );
  assert.ok(measured.length > 1);
  assert.notEqual(measured[0].contentId, measured[1].contentId);
  assert.equal(
    report.runs.find((run) => run.kind === "content").content.status,
    "unavailable",
  );
});

async function diagnosticFixture(t, mode) {
  const f = await fixture(t);
  await mkdir(path.join(f.source, "src/renderer/src"), { recursive: true });
  await writeFile(
    path.join(f.source, "src/renderer/src/content_evidence.rs"),
    "// controlled observations\n",
  );
  const renderer = `#!/usr/bin/env node
import net from 'node:net';
import fs from 'node:fs';
const mode = ${JSON.stringify(mode)};
const file = process.env.ROONSCAPE_CONTENT_EVIDENCE;
const now = () => Number(process.hrtime.bigint()/1000n);
const rows = [];
const physical = [], animation = [];
const physicalFile = process.env.ROONSCAPE_PRESENT_EVIDENCE;
const animationFile = process.env.ROONSCAPE_ANIMATION_EVIDENCE;
const subscribed = now();
let previousMsc=100;
const emit = (event, fields={}) => rows.push({event,observedMicros:now(),...fields});
let buffer = '', latest;
const socket = net.createConnection(process.env.ROONSCAPE_SOCKET);
socket.on('error',()=>process.exit(1));
socket.on('data',chunk => {
  buffer += chunk;
  while(buffer.includes('\\n')) {
    const end = buffer.indexOf('\\n');
    const snapshot = JSON.parse(buffer.slice(0,end)); buffer = buffer.slice(end+1);
    if (!file) continue;
    const revision = snapshot.revision, token = revision;
    latest = snapshot;
    if(mode === 'unavailable') continue;
    emit('snapshot-received',{revision, receivedMicros:now()});
    emit('preparation-requested',{revision,token});
    emit('preparation-started',{revision,token});
    if (mode === 'incomplete') continue;
    emit('artwork-completed',{path:snapshot.artwork.path,success:true,resource:revision});
    emit('preparation-completed',{revision,token,success:true,artworkResource:revision});
    if(mode === 'behavior') emit('native-draw',{revision,preparedToken:token,frame:revision,scene:revision,frameTimeMicros:now(),drawing:{ready:false,targetArtwork:revision,artwork:[[revision,0]],progress:0.2}});
    if(mode === 'superseded' || mode === 'behavior') emit('preparation-superseded',{token});
    if(mode === 'behavior' || mode.startsWith('physical')) draw(snapshot);
  }
});
function draw(snapshot) {
  if(physicalFile) {
    const frame=snapshot.revision, scene=frame, time=now();
    const msc=Math.max(previousMsc+1,100+Math.ceil((time-subscribed)/ (1000000/60))); previousMsc=msc;
    const presented=Math.round(subscribed+(msc-100)*(1000000/60));
    physical.push({event:'submission',frame,scene,window:10,serial:frame,options:0,targetMsc:msc,observedMicros:time});
    if(mode !== 'physical-missing' || frame===1) physical.push({event:'completion',window:10,serial:frame,mode:1,ust:presented,msc,observedMicros:presented+20});
    animation.push({event:'state',source:'scheduled-update',frame,frameTimeMicros:time,observedMicros:time,values:{animationActive:mode==='physical-stall'}},
      {event:'state',source:'composition-painted',frame,frameTimeMicros:time,observedMicros:time,values:{revision:frame}},
      {event:'after-paint',frame,scene,frameTimeMicros:time,observedMicros:time});
  }
  emit('native-draw' ,{revision:snapshot.revision,preparedToken:snapshot.revision,frame:snapshot.revision,scene:snapshot.revision,frameTimeMicros:now(),drawing:{ready:true,targetArtwork:snapshot.revision,artwork:[[snapshot.revision,1]],progress:0.2}});
}
if(file) {
  fs.writeFileSync(file,JSON.stringify({event:'start',version:1,clock:'CLOCK_MONOTONIC',physicalDelivery:false})+'\\n');
  const control=net.createConnection(process.env.ROONSCAPE_CONTENT_EVIDENCE_CONTROL);
  control.on('error',()=>process.exit(1));
  control.on('data',()=> {
    if(mode === 'delayed') draw(latest);
    if(mode === 'malformed') rows.push({event:'preparation-completed',observedMicros:now()});
    if(mode === 'malformed-draw') emit('native-draw',{revision:1,preparedToken:1,frame:1,scene:1,frameTimeMicros:now(),drawing:{ready:true,targetArtwork:1,artwork:[null]}});
    fs.appendFileSync(file, rows.map(row=>JSON.stringify(row)+'\\n').join('')+JSON.stringify({event:'end',records:rows.length,lost:mode === 'overflow'?1:0,complete:mode !== 'overflow'})+'\\n');
    if(physicalFile) {
      const trace=[{event:'start',version:1,clock:'CLOCK_MONOTONIC',identitySupported:true,capacity:4096},
        {event:'subscribed',window:10,observedMicros:subscribed},...physical,
        {event:'end',complete:true,lost:0,records:physical.length+1,observedMicros:now()}];
      fs.writeFileSync(physicalFile,trace.map(row=>JSON.stringify(row)).join('\\n')+'\\n');
      fs.writeFileSync(animationFile,[{event:'start',version:1,clock:'CLOCK_MONOTONIC'},{event:'display',monitorRefreshMillihertz:60000,frameTimeSource:'render-start'},...animation,{event:'end',complete:true,lost:0,records:animation.length+1,observedMicros:now()}].map(row=>JSON.stringify(row)).join('\\n')+'\\n');
    }
    process.exit(0);
  });
}
setInterval(()=>{},1000);
`;
  await writeFile(
    path.join(f.bin, "cargo"),
    `#!/usr/bin/env node\nimport fs from 'node:fs'; if(process.argv.includes('--version')) console.log('controlled'); else {fs.mkdirSync(process.env.CARGO_TARGET_DIR+'/release',{recursive:true});fs.writeFileSync(process.env.CARGO_TARGET_DIR+'/release/roonscape-renderer',${JSON.stringify(renderer)},{mode:0o755});}\n`,
  );
  if (mode === "delayed" || mode.startsWith("physical")) {
    await execute("git", ["-C", f.source, "add", "."]);
    await execute("git", [
      "-C",
      f.source,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.com",
      "commit",
      "-qm",
      "supported diagnostic baseline",
    ]);
  }
  return f;
}

for (const mode of [
  "delayed",
  "superseded",
  "incomplete",
  "unavailable",
  "overflow",
  "malformed",
  "malformed-draw",
  "behavior",
]) {
  test(`diagnostic command preserves ${mode} content outcomes`, async (t) => {
    const f = await diagnosticFixture(t, mode);
    const { report, outcome, output } = await compareFixture(
      f,
      [],
      "fresh-content",
    );
    const run = report.runs.find(
      (run) => run.side === "candidate" && run.kind === "content",
    );
    if (["overflow", "malformed", "malformed-draw"].includes(mode)) {
      assert.equal(outcome.code, 2, outcome.stderr);
      assert.equal(report.status, "invalid-evidence");
    } else if (mode === "behavior") {
      assert.equal(outcome.code, 3, outcome.stderr);
      assert.equal(run.content.status, "behavior-failed");
      assert.match(run.content.failures.join(), /Superseded preparation/);
    } else {
      assert.equal(report.status, "complete", outcome.stderr);
      assert.ok(run.content.publications.some((p) => p.outcome === mode));
      assert.ok(
        (await readFile(path.join(output, "report.md"), "utf8")).includes(mode),
      );
      if (mode === "delayed") {
        assert.ok(
          run.content.publications.at(-1).publicationToNativeDrawMs > 0,
        );
        assert.equal(
          report.contentSummaries["fresh-content"].publicationToNativeDrawMs
            .candidate.n,
          1,
        );
        assert.equal(
          report.contentSummaries["fresh-content"].publicationToNativeDrawMs
            .baseline.n,
          1,
        );
        assert.ok(
          Number.isFinite(
            report.contentSummaries["fresh-content"].publicationToNativeDrawMs
              .absoluteDelta,
          ),
        );
      }
    }
    assert.equal(report.summaries["fresh-content"].cpuSeconds.candidate.n, 1);
  });
}

test(
  "native slow preparation continues drawing and discards superseded content",
  { timeout: 60000 },
  async (t) => {
    const { createNativeSession } = await import("./native-session.mjs");
    const { createServer } = await import("node:net");
    const { open } = await import("node:fs/promises");
    const { constants } = await import("node:fs");
    const { waitFor, waitForProcessExit } =
      await import("./process-harness.mjs");
    const f = await fixture(t);
    const fifo = path.join(f.directory, "slow.png");
    const png = path.join(f.directory, "pixels.png");
    await execute("ffmpeg", [
      "-v",
      "error",
      "-i",
      path.join(root, "src/shared/fixtures/artwork/light.jpg"),
      "-vf",
      "scale=32:32",
      png,
    ]);
    await execute("mkfifo", [fifo]);
    const writer = await open(fifo, constants.O_RDWR);
    const session = await createNativeSession({ width: 1280, height: 720 });
    const evidence = path.join(f.directory, "native.jsonl");
    let connection, control;
    const server = createServer((socket) => {
      connection = socket;
      socket.on("error", () => {});
    });
    const controls = createServer((socket) => {
      control = socket;
      socket.on("error", () => {});
    });
    const socketPath = path.join(session.runtimeDirectory, "snapshots.sock");
    const controlPath = path.join(session.runtimeDirectory, "content.sock");
    await new Promise((resolve) => server.listen(socketPath, resolve));
    await new Promise((resolve) => controls.listen(controlPath, resolve));
    let released = false;
    t.after(async () => {
      if (!released) await writer.close();
      connection?.destroy();
      control?.destroy();
      await session.close();
      await Promise.all([
        new Promise((resolve) => server.close(resolve)),
        new Promise((resolve) => controls.close(resolve)),
      ]);
    });
    const renderer = session.startProcess(
      path.join(root, "target/debug/roonscape-renderer"),
      ["--config", session.configurationPath],
      {
        ROONSCAPE_SOCKET: socketPath,
        ROONSCAPE_CONTENT_EVIDENCE: evidence,
        ROONSCAPE_CONTENT_EVIDENCE_CONTROL: controlPath,
        ROONSCAPE_CAPTURE_VIEWPORT: "1280x720",
      },
    );
    await renderer.spawned;
    const observe = async (predicate) =>
      waitFor(
        async () => {
          const rows = (await readFile(evidence, "utf8"))
            .trim()
            .split("\n")
            .filter(Boolean)
            .map((line) => JSON.parse(line));
          const result = predicate(rows);
          assert.ok(result, "waiting for native observation");
          return result;
        },
        renderer,
        "native content observations",
        { timeoutMilliseconds: 15000 },
      );
    await waitFor(
      () => assert.ok(connection && control),
      renderer,
      "native connections",
    );
    const snapshot = JSON.parse(
      await readFile(
        path.join(root, "src/shared/fixtures/playing.json"),
        "utf8",
      ),
    );
    snapshot.artwork.path = path.join(root, snapshot.artwork.path);
    snapshot.revision = 1;
    snapshot.timing.position.sampledAt = new Date().toISOString();
    connection.write(JSON.stringify(snapshot) + "\n");
    await observe((rows) =>
      rows.find(
        (row) =>
          row.event === "native-draw" &&
          row.revision === 1 &&
          row.drawing.ready,
      ),
    );
    snapshot.revision = 2;
    snapshot.artwork = { path: fifo, revision: 2 };
    snapshot.nowPlaying.title = "Slow incoming content";
    connection.write(JSON.stringify(snapshot) + "\n");
    const blocked = await observe((rows) =>
      rows.find((row) => row.event === "artwork-started" && row.path === fifo),
    );
    const waiting = await observe((rows) => {
      const draws = rows.filter(
        (row) =>
          row.event === "native-draw" &&
          row.observedMicros >= blocked.observedMicros &&
          !row.drawing.ready,
      );
      return (
        new Set(
          draws.map((row) => row.drawing.progress).filter(Number.isFinite),
        ).size >= 2 && draws
      );
    });
    assert.ok(waiting.every((draw) => draw.drawing.artwork.length > 0));
    snapshot.revision = 3;
    snapshot.artwork = {
      path: path.join(root, "src/shared/fixtures/artwork/dark-teal.jpg"),
      revision: 3,
    };
    snapshot.nowPlaying.title = "Current replacement";
    connection.write(JSON.stringify(snapshot) + "\n");
    await observe((rows) =>
      rows.find(
        (row) => row.event === "preparation-requested" && row.revision === 3,
      ),
    );
    await writer.writeFile(await readFile(png));
    await writer.close();
    released = true;
    await observe((rows) =>
      rows.find(
        (row) =>
          row.event === "native-draw" &&
          row.revision === 3 &&
          row.drawing.ready,
      ),
    );
    const original = await observe((rows) =>
      rows.find(
        (row) => row.event === "preparation-completed" && row.revision === 1,
      ),
    );
    for (let revision = 4; revision <= 9; revision++) {
      const replacement = path.join(f.directory, `replacement-${revision}.png`);
      await cp(png, replacement);
      snapshot.revision = revision;
      snapshot.artwork = { path: replacement, revision };
      snapshot.nowPlaying.title = `Replacement ${revision}`;
      connection.write(JSON.stringify(snapshot) + "\n");
      await observe((rows) =>
        rows.find(
          (row) =>
            row.event === "native-draw" &&
            row.revision === revision &&
            row.drawing.ready,
        ),
      );
    }
    await observe((rows) =>
      rows.find(
        (row) =>
          row.event === "resource-released" &&
          row.resource === original.artworkResource,
      ),
    );
    control.write("F");
    const [code] = await waitForProcessExit(renderer, {
      timeoutMilliseconds: 15000,
    });
    assert.equal(code, 0, renderer.capturedStandardError);
    const rows = (await readFile(evidence, "utf8"))
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    const stale = rows.find(
      (row) => row.event === "preparation-requested" && row.revision === 2,
    ).token;
    assert.ok(
      rows.some(
        (row) => row.event === "preparation-superseded" && row.token === stale,
      ),
    );
    assert.ok(
      !rows.some(
        (row) => row.event === "native-draw" && row.preparedToken === stale,
      ),
    );
    const retained = new Set();
    for (const row of rows) {
      if (row.event === "resource-created") retained.add(row.resource);
      if (row.event === "resource-released") retained.delete(row.resource);
    }
    assert.equal(retained.size, 0);
    assert.equal(rows.at(-1).complete, true);
  },
);

for (const mode of [
  "physical",
  "physical-missing",
  "physical-stall",
  "physical-cancel",
])
  test(`physical command preserves ${mode} evidence, resource separation, and cleanup`, async (t) => {
    const f = await diagnosticFixture(t, mode);
    const compiler = `#!/usr/bin/env node
import fs from 'node:fs';
if(process.argv.includes('--version')) console.log('controlled physical fixture compiler');
else {
 const destination=process.argv[process.argv.indexOf('-o')+1];
 fs.writeFileSync(destination,destination.endsWith('.so')?'': '#!/bin/sh\\necho \\'{"supported":true,"root":1,"outputId":2,"crtc":3,"mode":4,"width":1280,"height":720,"refreshMillihertz":60000,"compositor":false}\\'\\n',{mode:0o755});
}
`;
    await writeFile(path.join(f.bin, "c++"), compiler, { mode: 0o755 });
    const output = path.join(f.directory, "physical-output");
    const operation = execute(
      process.execPath,
      [
        path.join(root, "scripts/accept-presentation.mjs"),
        "--display",
        ":98765",
        "--physical-output",
        "EXAMPLE-1",
        "--baseline",
        `worktree:${f.source}`,
        "--candidate",
        `worktree:${f.source}`,
        "--profile",
        "smoke",
        "--workloads",
        "fresh-content",
        ...(mode === "physical-stall" ? ["--max-missed-refreshes", "0"] : []),
        ...(mode === "physical-cancel" ? ["--measurement-seconds", "2"] : []),
        "--output",
        output,
      ],
      { env: f.env },
    );
    if (mode === "physical-cancel") {
      const { waitFor } = await import("./process-harness.mjs");
      let requested = false;
      operation.child.stdout.on("data", (chunk) => {
        if (requested || !chunk.toString().includes("Physical repeat")) return;
        requested = true;
        void waitFor(
          async () =>
            assert.ok(
              (await readFile(path.join(output, "content-3.jsonl"))).length > 0,
            ),
          operation.child,
          "partial diagnostic evidence",
        ).then(
          () => operation.child.kill("SIGTERM"),
          () => operation.child.kill("SIGTERM"),
        );
      });
    }
    const outcome = await operation
      .then((result) => ({ ...result, code: 0 }))
      .catch((error) => error);
    assert.equal(
      outcome.code,
      mode === "physical"
        ? 0
        : mode === "physical-stall"
          ? 3
          : mode === "physical-cancel"
            ? 130
            : 2,
      outcome.stderr,
    );
    const report = JSON.parse(
      await readFile(path.join(output, "report.json"), "utf8"),
    );
    assert.equal(report.executionMode, "physical-display");
    assert.equal(report.summaries["fresh-content"].cpuSeconds.baseline.n, 1);
    assert.equal(
      report.runs.filter((run) => run.kind === "resources").length,
      2,
    );
    assert.ok(report.runs.some((run) => run.kind === "physical"));
    const observed = report.runs.find((run) => run.kind === "physical");
    if (mode === "physical") {
      assert.equal(report.status, "complete");
      assert.ok(
        observed.physical.publications.some(
          (p) => p.outcome === "presented" && p.publicationToPresentedMs > 0,
        ),
      );
      assert.equal(report.physicalSummaries["fresh-content"].baseline.n, 1);
      assert.equal(report.instrumentationOverhead.length, 2);
      // A display host can run retained builds without source-build tools or Xvfb.
      const runtimeBin = path.join(f.directory, "runtime-bin");
      await mkdir(runtimeBin);
      for (const name of ["node", "sh", "sleep", "dbus-daemon", "fc-list"])
        await symlink(
          name === "node"
            ? process.execPath
            : (await execute("which", [name])).stdout.trim(),
          path.join(runtimeBin, name),
        );
      for (const name of ["c++", "xwininfo", "python3"])
        await symlink(path.join(f.bin, name), path.join(runtimeBin, name));
      const retainedOutput = path.join(f.directory, "physical-retained-output");
      await execute(
        process.execPath,
        [
          path.join(root, "scripts/accept-presentation.mjs"),
          "--display",
          ":98765",
          "--physical-output",
          "EXAMPLE-1",
          "--baseline",
          `build:${output}/baseline`,
          "--candidate",
          `build:${output}/candidate`,
          "--profile",
          "smoke",
          "--workloads",
          "fresh-content",
          "--output",
          retainedOutput,
        ],
        { env: { ...f.env, PATH: runtimeBin } },
      );
      const retained = JSON.parse(
        await readFile(path.join(retainedOutput, "report.json"), "utf8"),
      );
      assert.equal(retained.conditions.qt, null);
      assert.match(retained.conditions.qtVersionSource, /unavailable/);
      assert.equal(retained.conditions.executables.qmake6, undefined);
      assert.equal(retained.status, "complete");
      assert.equal(retained.conditions.executables.cargo, undefined);
      assert.equal(retained.conditions.executables.Xvfb, undefined);
    } else if (mode === "physical-cancel") {
      assert.equal(report.status, "cancelled");
      assert.equal(observed.status, "cancelled");
      assert.throws(() => process.kill(observed.process.pid, 0), {
        code: "ESRCH",
      });
      await assert.rejects(
        readFile(path.join(observed.process.sessionDirectory, "display.json")),
        { code: "ENOENT" },
      );
    } else if (mode === "physical-stall") {
      assert.equal(report.status, "behavior-failed");
      assert.equal(observed.physical.status, "complete");
      assert.equal(observed.physical.contract.status, "failed");
      assert.ok(observed.physical.cadence.missedPresentations > 0);
    } else {
      assert.equal(report.status, "invalid-evidence");
      assert.equal(observed.physical.cadence, null);
    }
    const markdown = await readFile(path.join(output, "report.md"), "utf8");
    assert.match(markdown, /not optical verification/);
    assert.match(markdown, /Instrumented CPU seconds/);
    assert.ok(markdown.split("\n").length < 200, "human report stays concise");
    assert.match(markdown, /\[JSON report\]\(report.json\)/);
    for (const run of report.runs) {
      if (run.evidence)
        assert.ok((await readFile(path.join(output, run.evidence))).length > 0);
    }
  });

test("real native diagnostic recorder connects content and scene evidence without claiming headless delivery", async (t) => {
  const { createNativeSession } = await import("./native-session.mjs");
  const owner = await createNativeSession({ width: 1280, height: 720 });
  t.after(() => owner.close());
  const f = await fixture(t);
  await mkdir(path.join(f.source, "src/renderer/src"), { recursive: true });
  await cp(
    path.join(root, "src/renderer/src/content_evidence.rs"),
    path.join(f.source, "src/renderer/src/content_evidence.rs"),
  );
  await writeFile(
    path.join(f.bin, "cargo"),
    `#!/usr/bin/env node\nimport fs from 'node:fs'; if(process.argv.includes('--version')) console.log('controlled native integration build'); else {fs.mkdirSync(process.env.CARGO_TARGET_DIR+'/release',{recursive:true});fs.copyFileSync(${JSON.stringify(path.join(root, "target/debug/roonscape-renderer"))},process.env.CARGO_TARGET_DIR+'/release/roonscape-renderer');}\n`,
  );
  for (const name of ["Xvfb", "xwininfo", "python3"])
    await rm(path.join(f.bin, name));
  const { stdout: compilerPath } = await execute("sh", [
    "-c",
    "command -v c++",
  ]);
  // Substitute only the capability probe: Xvfb is deliberately NOT a physical
  // display. The real collector/Renderer must still refuse physical claims.
  const probe =
    '#!/bin/sh\necho \'{"supported":true,"width":1280,"height":720,"refreshMillihertz":60000,"compositor":false}\'\n';
  await writeFile(
    path.join(f.bin, "c++"),
    `#!/usr/bin/env node
import fs from 'node:fs';import {spawnSync} from 'node:child_process';
if(process.argv.some(arg=>arg.endsWith('presentation-probe.cpp'))) fs.writeFileSync(process.argv[process.argv.indexOf('-o')+1],${JSON.stringify(probe)},{mode:0o755});
else {const child=spawnSync(${JSON.stringify(compilerPath.trim())},process.argv.slice(2),{stdio:'inherit',env:{...process.env,PATH:${JSON.stringify(process.env.PATH)}}});process.exit(child.status??1);}
`,
    { mode: 0o755 },
  );
  const output = path.join(f.directory, "native-physical");
  const outcome = await execute(
    process.execPath,
    [
      path.join(root, "scripts/accept-presentation.mjs"),
      "--display",
      owner.environment.DISPLAY,
      "--physical-output",
      "EXAMPLE-1",
      "--baseline",
      `worktree:${f.source}`,
      "--candidate",
      `worktree:${f.source}`,
      "--profile",
      "smoke",
      "--measurement-seconds",
      "1",
      "--workloads",
      "fresh-content",
      "--output",
      output,
    ],
    { env: f.env },
  ).catch((error) => error);
  assert.equal(outcome.code, 4, outcome.stderr);
  const report = JSON.parse(
    await readFile(path.join(output, "report.json"), "utf8"),
  );
  const observed = report.runs.find((run) => run.kind === "physical");
  assert.equal(observed.physical.status, "unavailable");
  assert.equal(observed.physical.cadence, null);
  assert.ok(observed.content.publications.some((p) => p.frame !== null));
  const animation = (
    await readFile(path.join(output, observed.animationEvidence), "utf8")
  )
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  assert.equal(animation.at(-1).complete, true);
  assert.ok(animation.some((row) => row.event === "after-paint"));
  for (const run of report.runs) {
    assert.equal(run.process.ownsDisplay, false);
    await assert.rejects(
      readFile(path.join(run.process.sessionDirectory, "display.json")),
      { code: "ENOENT" },
    );
    assert.throws(() => process.kill(run.process.pid, 0), { code: "ESRCH" });
  }
  const { stdout } = await execute("xwininfo", ["-root"], {
    env: owner.environment,
  });
  assert.match(stdout, /Width: 1280/);
});

test("release snapshots with old timestamps cannot reuse the other source's executable", async (t) => {
  const { buildSource } = await import("./renderer-comparison-sources.mjs");
  const scratch = await mkdtemp("/var/tmp/codex/roonscape/task.");
  t.after(() => rm(scratch, { recursive: true, force: true }));
  for (const side of ["baseline", "candidate"]) {
    const source = path.join(scratch, side);
    const evidence = path.join(scratch, `${side}-evidence`);
    await mkdir(evidence);
    for (const relative of ["src/renderer/assets/fonts", "src/desktop/icons"])
      await mkdir(path.join(source, relative), { recursive: true });
    const files = {
      "Cargo.toml":
        '[package]\nname="roonscape-renderer"\nversion="0.1.0"\nedition="2021"\n',
      "Cargo.lock":
        'version = 4\n[[package]]\nname = "roonscape-renderer"\nversion = "0.1.0"\n',
      "src/main.rs": `fn main() { println!("${side}"); }\n`,
    };
    for (const [relative, content] of Object.entries(files)) {
      const file = path.join(source, relative);
      await writeFile(file, content);
      await utimes(file, 1000000000, 1000000000);
    }
    await buildSource(source, evidence, new AbortController().signal);
    const { stdout } = await execute(
      path.join(evidence, "target/release/roonscape-renderer"),
    );
    assert.equal(stdout.trim(), side);
  }
});
