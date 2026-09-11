import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";

test("physical acceptance requires explicit display and output selection", () => {
  const result = spawnSync(
    process.execPath,
    [
      "scripts/accept-presentation.mjs",
      "--baseline",
      "ref:HEAD",
      "--candidate",
      "worktree:.",
    ],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 2);
  assert.match(result.stderr, /--display and --physical-output are required/);
});

import { summarizePhysicalPresentation } from "./physical-presentation.mjs";
function evidence() {
  const window = { startMicros: 1000000, endMicros: 1100000 };
  const frames = [1, 2, 3];
  const times = [1000000, 1016667, 1083333];
  return {
    window,
    animation: [
      {
        event: "display",
        monitorRefreshMillihertz: 60000,
        frameTimeSource: "render-start",
      },
      ...frames.flatMap((frame, i) => [
        {
          event: "state",
          source: "scheduled-update",
          frame,
          frameTimeMicros: times[i],
          observedMicros: times[i],
          values: { animationActive: i !== 2 },
        },
        {
          event: "state",
          source: "composition-painted",
          frame,
          frameTimeMicros: times[i],
          values: { opacity: frame / 3 },
        },
        { event: "after-paint", frame, scene: frame, observedMicros: times[i] },
      ]),
    ],
    trace: [
      {
        event: "start",
        version: 1,
        clock: "CLOCK_MONOTONIC",
        identitySupported: true,
      },
      { event: "subscribed", window: 10, observedMicros: 900000 },
      ...frames.map((frame, i) => ({
        event: "submission",
        frame,
        scene: frame,
        window: 10,
        serial: frame,
        observedMicros: times[i],
        targetMsc: 100 + i,
        options: 0,
      })),
      ...frames.map((frame, i) => ({
        event: "completion",
        window: 10,
        serial: frame,
        ust: times[i] + 1000,
        observedMicros: times[i] + 2000,
        msc: [100, 101, 105][i],
        mode: 1,
      })),
      {
        event: "end",
        complete: true,
        lost: 0,
        records: 7,
        observedMicros: 1200000,
      },
    ],
    draws: frames.map((frame, i) => ({
      event: "native-draw",
      frame,
      scene: frame,
      revision: frame,
      preparedToken: frame,
      observedMicros: times[i] + 100,
      drawing: { ready: true },
    })),
    publications: frames.map((frame, i) => ({
      revision: frame,
      preparationToken: frame,
      artworkResource: frame,
      contentId: `content-${frame}`,
      publishedMicros: times[i] - 1000,
      outcome: "ready",
    })),
  };
}
test("physical analysis joins exact scenes and reports delivery stalls and content latency separately", () => {
  const result = summarizePhysicalPresentation(evidence());
  assert.equal(result.status, "complete");
  assert.equal(result.cadence.missedPresentations, 3);
  assert.equal(result.publications[0].publicationToPresentedMs, 2);
  assert.equal(result.publications[0].scene, 1);
  assert.equal(result.contract.status, "not-requested");
});

for (const [name, mutate] of Object.entries({
  missing: (input) => input.trace.splice(6, 1),
  ambiguous: (input) => input.trace.splice(6, 0, input.trace[6]),
  "scene mismatch": (input) => (input.trace[2].scene = 999),
  overflow: (input) => (input.trace.at(-1).lost = 1),
  "missing footer": (input) => input.trace.pop(),
  "malformed timestamp": (input) => (input.trace[2].observedMicros = "bad"),
}))
  test(`physical analysis cannot pass with ${name} evidence`, () => {
    const input = evidence();
    mutate(input);
    const result = summarizePhysicalPresentation({
      ...input,
      maxMissedRefreshes: 100,
    });
    assert.equal(result.status, "invalid-evidence");
    assert.equal(result.cadence, null);
    assert.equal(result.contract.status, "unavailable");
  });

test("copy completions remain unavailable and never establish physical content visibility", () => {
  const input = evidence();
  input.trace[5].mode = 0;
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.status, "unavailable");
  assert.equal(result.publications[0].publicationToPresentedMs, null);
  assert.equal(result.publications[0].outcome, "unavailable");
});

import { mkdtemp, readFile, rm } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { execFile } from "node:child_process";
const execute = promisify(execFile);

test("physical command retains an unavailable capability report before building sources", async (t) => {
  const scratch = await mkdtemp("/var/tmp/codex/roonscape/task.");
  t.after(() => rm(scratch, { recursive: true, force: true }));
  const output = path.join(scratch, "evidence");
  const result = await execute(process.execPath, [
    "scripts/accept-presentation.mjs",
    "--display",
    ":98765",
    "--physical-output",
    "EXAMPLE-1",
    "--baseline",
    "ref:HEAD",
    "--candidate",
    "worktree:.",
    "--workloads",
    "progress",
    "--profile",
    "smoke",
    "--output",
    output,
  ]).catch((error) => error);
  assert.equal(result.code, 4, result.stderr);
  const report = JSON.parse(
    await readFile(path.join(output, "report.json"), "utf8"),
  );
  assert.equal(report.executionMode, "physical-display");
  assert.equal(report.status, "unavailable");
  assert.equal(report.runs.length, 0);
  assert.match(
    await readFile(path.join(output, "report.md"), "utf8"),
    /physical capability unavailable/,
  );
});

import { createNativeSession } from "./native-session.mjs";
import { waitFor, waitForProcessExit } from "./process-harness.mjs";
import { open } from "node:fs/promises";

test("collector integrity with independent completions and stalled consumers", async (t) => {
  const scratch = await mkdtemp("/var/tmp/codex/roonscape/task.");
  const session = await createNativeSession({ width: 1280, height: 720 });
  t.after(async () => {
    await session.close();
    await rm(scratch, { recursive: true, force: true });
  });
  const collector = path.join(scratch, "collector.so"),
    binary = path.join(scratch, "fixture");
  await execute("c++", [
    "-std=c++17",
    "-O2",
    "-shared",
    "-fPIC",
    "scripts/presentation-collector.cpp",
    "-o",
    collector,
    "-lxcb",
    "-lxcb-present",
    "-ldl",
    "-pthread",
  ]);
  await execute("c++", [
    "-std=c++17",
    "-O2",
    "scripts/presentation-collector-fixture.cpp",
    "-o",
    binary,
    "-lxcb",
    "-lxcb-present",
    "-Wl,--export-dynamic",
  ]);
  for (const [name, count, blocked] of [
    ["normal draining", 2, false],
    ["blocked within capacity", 2000, true],
    ["exhausted recording capacity", 12000, true],
  ])
    await t.test(name, async (t) => {
      const fifo = path.join(scratch, `control-${count}`);
      await execute("mkfifo", [fifo]);
      const child = session.startProcess(binary, [String(count), fifo], {
        LD_PRELOAD: collector,
        ROONSCAPE_PRESENT_EVIDENCE: "-",
      });
      let output = "";
      child.stdout.on("data", (bytes) => {
        output += bytes;
      });
      if (blocked) child.stdout.pause();
      await child.spawned;
      await waitFor(
        () => assert.match(child.capturedStandardError, /ready/),
        child,
        "collector fixture",
      );
      const control = await open(fifo, "w");
      t.after(() => control.close());
      if (!blocked) {
        await control.write("S");
        await waitFor(
          () => assert.match(output, /subscribed/),
          child,
          "completion subscription",
        );
        await control.write("S");
      } else await control.write("B");
      // This marker must arrive while the evidence reader remains paused. The
      // producer cannot depend on consumer progress, including on overflow.
      await waitFor(
        () =>
          assert.match(
            child.capturedStandardError,
            new RegExp(`submitted:${count}\\b`),
          ),
        child,
        "nonblocking frame submission",
      );
      child.stdout.resume();
      if (!blocked)
        await waitFor(
          () =>
            assert.match(output, /"event":"completion"[^\n]*"serial":2[,}]/),
          child,
          "independent completion",
        );
      await control.write("F");
      const [code] = await waitForProcessExit(child);
      assert.equal(code, 0, child.capturedStandardError);
      const rows = output
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      const footer = rows.at(-1);
      assert.equal(footer.records, rows.length - 2);
      const requests = rows.filter((row) => row.event === "submission");
      if (count > 4096) {
        assert.equal(footer.complete, false);
        assert.ok(footer.lost > 0);
        assert.ok(requests.length < count);
      } else {
        assert.equal(footer.complete, true, JSON.stringify(footer));
        assert.equal(footer.lost, 0);
        assert.equal(requests.length, count);
        assert.equal(requests.at(-1).frame, count);
        assert.equal(requests.at(-1).scene, count);
      }
      if (!blocked) {
        const completion = rows.find(
          (row) => row.event === "completion" && row.serial === 2,
        );
        assert.equal(completion.window, requests[1].window);
        assert.ok(completion.ust > 0);
      }
    });
});

test("physical content keeps delayed and superseded outcomes without inventing visibility", () => {
  const input = evidence();
  input.trace[6].mode = 2;
  input.trace[7].ust = 1117667;
  input.trace[7].observedMicros = 1117767;
  input.trace[7].msc = 107;
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.status, "complete");
  assert.equal(result.publications[1].outcome, "superseded");
  assert.equal(result.publications[1].publicationToPresentedMs, null);
  assert.equal(result.publications[2].outcome, "delayed");
  assert.equal(result.publications[2].presentedMicros, 1117667);
});

test("a confirmed quiet scene spanning the window needs no new flips", () => {
  const input = evidence();
  input.window = { startMicros: 1090000, endMicros: 1190000 };
  input.publications = [];
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.status, "complete");
  assert.equal(result.cadence.missedPresentations, 0);
});

for (const [name, mutate] of Object.entries({
  "drawn scene record": (input) => {
    input.animation = input.animation.filter(
      (row) => !(row.event === "after-paint" && row.frame === 2),
    );
  },
  "drawn values": (input) => {
    input.animation = input.animation.filter(
      (row) => !(row.source === "composition-painted" && row.frame === 2),
    );
  },
  "native content identity": (input) => {
    input.draws = input.draws.filter((row) => row.frame !== 2);
  },
  "logical completion": (input) => {
    input.trace.splice(6, 1);
    input.trace.at(-1).records--;
  },
}))
  test(`lost ${name} invalidates an otherwise normally drained run`, () => {
    const input = evidence();
    mutate(input);
    const result = summarizePhysicalPresentation(input);
    assert.equal(result.status, "invalid-evidence");
    assert.equal(result.cadence, null);
  });

test("a later missing completion cannot erase an earlier proven first presentation", () => {
  const input = evidence();
  input.draws[2].revision = 1;
  input.trace.splice(7, 1);
  input.trace.at(-1).records--;
  input.window.endMicros = 1050000;
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.publications[0].outcome, "presented");
  assert.equal(result.publications[0].publicationToPresentedMs, 2);
});

test("an unsupported submission path is unavailable even when native draw callbacks exist", () => {
  const input = evidence();
  input.trace = [
    input.trace[0],
    {
      event: "end",
      complete: true,
      lost: 0,
      records: 0,
      observedMicros: 1200000,
    },
  ];
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.status, "unavailable");
  assert.equal(result.cadence, null);
  assert.ok(
    result.publications.every((p) => p.publicationToPresentedMs === null),
  );
});

test("borrowed display cleanup removes only its private session and preserves the display owner", async (t) => {
  const owner = await createNativeSession({ width: 1280, height: 720 });
  t.after(() => owner.close());
  const borrowed = await createNativeSession({
    width: 1280,
    height: 720,
    physicalDisplay: { display: owner.environment.DISPLAY },
  });
  const directory = borrowed.runtimeDirectory;
  assert.equal(borrowed.environment.DISPLAY, owner.environment.DISPLAY);
  assert.notEqual(
    borrowed.environment.DBUS_SESSION_BUS_ADDRESS,
    owner.environment.DBUS_SESSION_BUS_ADDRESS,
  );
  await borrowed.close();
  await assert.rejects(readFile(path.join(directory, "display.json")), {
    code: "ENOENT",
  });
  const { stdout } = await execute("xwininfo", ["-root"], {
    env: owner.environment,
  });
  assert.match(stdout, /Width: 1280/);
});

test("physical cadence respects refresh stride and records the worst observed gap separately from stalls", () => {
  const input = evidence();
  input.animation[0].monitorRefreshMillihertz = 144000;
  const times = [1000000, 1020833, 1041667];
  for (let i = 0; i < 3; i++) {
    for (const entry of input.animation.filter((row) => row.frame === i + 1)) {
      entry.frameTimeMicros = times[i];
      entry.observedMicros = times[i];
    }
    input.draws[i].observedMicros = times[i] + 100;
    input.publications[i].publishedMicros = times[i] - 1000;
    Object.assign(input.trace[i + 2], {
      observedMicros: times[i],
      targetMsc: 100 + i * 3,
    });
    Object.assign(input.trace[i + 5], {
      ust: times[i] + 1000,
      observedMicros: times[i] + 2000,
      msc: 100 + i * 3,
    });
  }
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.cadence.missedPresentations, 0);
  assert.equal(result.cadence.refreshStride, 3);
  assert.equal(result.worstDeliveryGapMicros, 20834);
  assert.equal(result.worstStallMicros, 0);
});

for (const [name, mutate] of Object.entries({
  "duplicate display sequence": (input) => {
    input.trace[6].msc = 100;
  },
  "backward completion time": (input) => {
    input.trace[6].ust = 999999;
  },
  "mismatched refresh clock": (input) => {
    input.trace[6].msc = 1000;
  },
}))
  test(`${name} cannot establish reliable physical cadence`, () => {
    const input = evidence();
    mutate(input);
    const result = summarizePhysicalPresentation(input);
    assert.equal(result.status, "invalid-evidence");
    assert.equal(result.cadence, null);
  });

test("missing and ambiguous first-presentation evidence remain distinct outcomes", () => {
  const missing = evidence();
  missing.trace.splice(6, 1);
  missing.trace.at(-1).records--;
  const absent = summarizePhysicalPresentation(missing);
  assert.equal(absent.publications[1].outcome, "missing");
  const ambiguous = evidence();
  ambiguous.trace.splice(6, 0, ambiguous.trace[6]);
  ambiguous.trace.at(-1).records++;
  const uncertain = summarizePhysicalPresentation(ambiguous);
  assert.equal(uncertain.publications[1].outcome, "ambiguous");
});

test("only skipped completions cannot pass a physical cadence contract", () => {
  const input = evidence();
  for (const row of input.trace.filter((row) => row.event === "completion"))
    row.mode = 2;
  const result = summarizePhysicalPresentation({
    ...input,
    maxMissedRefreshes: 0,
  });
  assert.equal(result.status, "unavailable");
  assert.equal(result.cadence, null);
  assert.equal(result.contract.status, "unavailable");
});

test("a delayed GUI callback cannot move an unassociated submission outside the window", () => {
  const input = evidence();
  input.animation.find(
    (row) => row.event === "after-paint" && row.frame === 3,
  ).observedMicros = 1110000;
  input.trace.splice(7, 1);
  input.trace.at(-1).records--;
  const result = summarizePhysicalPresentation({
    ...input,
    maxMissedRefreshes: 0,
  });
  assert.equal(result.status, "invalid-evidence");
  assert.equal(result.cadence, null);
  assert.equal(result.contract.status, "unavailable");
});

test("outgoing prepared content can first appear while its replacement is unready", () => {
  const input = evidence();
  input.trace[5].mode = 2;
  input.publications[0].artworkResource = 41;
  Object.assign(input.draws[1], {
    revision: 1,
    preparedToken: 1,
    drawing: { ready: false, targetArtwork: 41, artwork: [[41, 1]] },
  });
  const result = summarizePhysicalPresentation(input);
  assert.equal(result.publications[0].outcome, "presented");
  assert.equal(result.publications[0].frame, 2);
  assert.equal(result.publications[0].publicationToPresentedMs, 18.667);
});

test("unready incoming content cannot claim the outgoing preparation's presentation", () => {
  const input = evidence();
  Object.assign(input.draws[1], {
    preparedToken: 1,
    drawing: { ready: false, targetArtwork: 41, artwork: [[41, 1]] },
  });
  Object.assign(input.publications[1], {
    preparationToken: 2,
    artworkResource: 42,
    outcome: "incomplete",
  });
  const result = summarizePhysicalPresentation(input);
  assert.notEqual(result.publications[1].outcome, "presented");
  assert.equal(result.publications[1].presentedMicros, null);
});
