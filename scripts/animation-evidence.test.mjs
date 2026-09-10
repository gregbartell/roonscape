import assert from "node:assert/strict";
import test from "node:test";
import {
  associatePresentationEvents,
  summarizeAnimationEvidence,
} from "./animation-evidence.mjs";

const display = (rate = 60_000) => ({
  event: "display",
  monitorRefreshMillihertz: rate,
  renderer: "test",
});
const update = (frame, time, active = true) => ({
  event: "state",
  source: "scheduled-update",
  widget: "presentation",
  frame,
  frameTimeMicros: time,
  values: { animationActive: active },
});
const paint = (frame, value) => ({
  event: "state",
  source: "composition-painted",
  widget: "presentation",
  frame,
  values: { opacity: value },
});
const presentation = (frame, time) => ({
  event: "presentation",
  frame,
  presentedMicros: time,
  predictedMicros: time || 1_000_000,
});

function renderStartEvidence(rate = 60_000) {
  const stride = Math.ceil(rate / 60_000);
  const interval = (stride * 1_000_000_000) / rate;
  const times = [0, Math.round(interval * 1.527), Math.round(interval * 2)];
  return {
    entries: [
      { ...display(rate), frameTimeSource: "render-start" },
      ...times.flatMap((time, index) => [
        update(index + 1, time),
        paint(index + 1, index / 3),
        { event: "after-paint", frame: index + 1, scene: index + 10 },
      ]),
    ],
    external: {
      submissions: times.map((_, index) => ({
        window: 10,
        serial: index,
        frame: index + 1,
        targetMsc: 100 + index * stride,
      })),
      completions: times.map((_, index) => ({
        window: 10,
        serial: index,
        mode: 1,
        ust: Math.round(1_000_000 + index * interval),
        msc: 100 + index * stride,
      })),
    },
  };
}

test("advancing on-time presentations distinguish CPU gaps from missed refreshes", () => {
  for (const rate of [60_000, 144_000]) {
    const { entries, external } = renderStartEvidence(rate);
    const result = summarizeAnimationEvidence(entries, external);
    assert.equal(result.missedUpdates, 0);
    assert.equal(result.missedPresentations, 0);
    assert.equal(result.changedPaints, 2);
    assert.equal(result.renderStartGaps.length, 1);
    assert.equal(result.renderStartGaps[0].apparentMissedUpdates, 1);
  }
});

test("incomplete or nonadvancing delivery cannot excuse a render-start gap", () => {
  const changes = {
    "scheduled clock": ({ entries }) => {
      delete entries[0].frameTimeSource;
    },
    "missing completion": ({ external }) => {
      external.completions.splice(1, 1);
    },
    "ambiguous completion": ({ external }) => {
      external.completions.push(external.completions[1]);
    },
    "skipped completion": ({ external }) => {
      external.completions[1].mode = 2;
    },
    "cross-window completion": ({ external }) => {
      external.submissions[1].window = 20;
      external.completions[1].window = 20;
    },
    "late presentation": ({ external }) => {
      external.submissions[1].targetMsc -= 1;
    },
    "missing target": ({ external }) => {
      delete external.submissions[1].targetMsc;
    },
    "repeated scene": ({ entries }) => {
      entries[6].scene = entries[3].scene;
    },
    "repeated state": ({ entries }) => {
      entries[5].values = entries[2].values;
    },
    "missing paint": ({ entries }) => {
      entries.splice(5, 1);
    },
    "skipped refresh": ({ external }) => {
      for (let index = 1; index < 3; index += 1) {
        external.submissions[index].targetMsc += 1;
        external.completions[index].msc += 1;
        external.completions[index].ust += 16_667;
      }
    },
  };
  for (const [reason, change] of Object.entries(changes)) {
    const fixture = renderStartEvidence();
    change(fixture);
    const result = summarizeAnimationEvidence(
      fixture.entries,
      fixture.external,
    );
    assert.equal(result.missedUpdates, 1, reason);
    assert.equal(result.renderStartGaps.length, 0, reason);
    if (reason === "skipped refresh")
      assert.equal(result.missedPresentations, 1);
  }
});

test("render-start corroboration respects window boundaries and unobserved tails", () => {
  const { entries, external } = renderStartEvidence();
  const summarize = (startMicros, endMicros) =>
    summarizeAnimationEvidence(entries, external, { startMicros, endMicros });
  assert.equal(summarize(16_666, 30_000).renderStartGaps.length, 1);
  assert.equal(summarize(0, 16_666).renderStartGaps.length, 0);
  const tail = summarize(40_000, 100_000);
  assert.ok(tail.missedUpdates > 0);
  assert.equal(tail.renderStartGaps.length, 0);
});

test("callbacks alone do not establish advancing content or presentation", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    update(2, 16_667),
  ]);
  assert.equal(evidence.missedUpdates, 0);
  assert.equal(evidence.changedPaints, 0);
  assert.equal(evidence.updatesWithoutPaint, 2);
  assert.equal(evidence.updatesWithoutPresentationTime, 2);
});

test("unused refreshes under the pacing policy are not missed updates", () => {
  const evidence = summarizeAnimationEvidence([
    display(144_000),
    update(1, 0),
    paint(1, 0),
    presentation(1, 1_000_000),
    update(4, 20_833),
    paint(4, 0.5),
    presentation(4, 1_020_833),
    update(7, 41_667),
    paint(7, 1),
    presentation(7, 1_041_667),
  ]);
  assert.equal(evidence.scheduledFramesPerSecond, 48);
  assert.equal(evidence.missedUpdates, 0);
  assert.equal(evidence.changedPaints, 2);
  assert.equal(evidence.updatesWithoutPresentationTime, 0);
});

test("reports every missed scheduled period without a percentage allowance", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    update(4, 50_000),
  ]);
  assert.equal(evidence.missedUpdates, 2);
  assert.deepEqual(
    evidence.misses.map((miss) => miss.gapMicros),
    [50_000],
  );
});

test("a settled endpoint ends the scheduled interval", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0, false),
    update(10, 150_000),
  ]);
  assert.equal(evidence.missedUpdates, 0);
});

test("missing backend timing is not replaced by a prediction or a paint", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.5),
    presentation(1, 0),
  ]);
  assert.equal(evidence.updatesWithoutPaint, 0);
  assert.equal(evidence.updatesWithoutPresentationTime, 1);
});

test("unchanged content is reported separately from a scheduling miss", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 1),
    update(2, 16_667),
    paint(2, 1),
  ]);
  assert.equal(evidence.missedUpdates, 0);
  assert.equal(evidence.unchangedPaints, 1);
  assert.equal(evidence.unchangedUpdatesWithoutPresentation, 0);
});

test("external delivery joins window and serial despite completion order", () => {
  const evidence = summarizeAnimationEvidence(
    [display(), update(1, 0), paint(1, 0.5), presentation(1, 0)],
    {
      submissions: [
        { window: 10, serial: 7, frame: 1 },
        { window: 20, serial: 7, frame: 2 },
      ],
      completions: [
        { window: 20, serial: 7, mode: 1, ust: 20_000, msc: 2 },
        { window: 10, serial: 7, mode: 0, ust: 10_000, msc: 1 },
      ],
    },
  );
  assert.equal(evidence.updatesWithoutPresentationTime, 0);
  assert.equal(evidence.presentationAssociation.associatedFrames, 2);
  const joined = associatePresentationEvents(
    [{ window: 10, serial: 7, frame: 1 }],
    [{ window: 20, serial: 7, mode: 1, ust: 10_000, msc: 1 }],
  );
  assert.equal(joined.presentations.length, 0);
  assert.equal(joined.counts.unmatchedCompletions, 1);
});

test("skipped, ambiguous, and invalid completions cannot prove delivery", () => {
  const submission = (serial, frame = serial) => ({
    window: 10,
    serial,
    frame,
  });
  const completion = (serial, mode = 0, ust = serial * 1_000) => ({
    window: 10,
    serial,
    mode,
    ust,
    msc: serial,
  });
  const joined = associatePresentationEvents(
    [
      submission(1),
      submission(2),
      submission(2),
      submission(3),
      submission(4, -1),
      submission(5),
      submission(6),
      submission(7),
    ],
    [
      completion(1, 2),
      completion(2),
      completion(3),
      completion(3),
      completion(4),
      completion(5, 0, 0),
      completion(6, 99),
      completion(7, 3),
    ],
  );
  assert.deepEqual(
    joined.presentations.map((entry) => entry.frame),
    [7],
  );
  assert.equal(joined.counts.skippedCompletions, 1);
  assert.equal(joined.counts.ambiguousCompletions, 1);
  assert.equal(joined.counts.ambiguousFrames, 1);
  assert.equal(joined.counts.invalidCompletions, 3);
});

test("missing identities and imprecise counters cannot establish a match", () => {
  const joined = associatePresentationEvents(
    [{ frame: 1 }, { window: 10, serial: 1, frame: 2 }],
    [
      { mode: 0, ust: 10_000, msc: 1 },
      { window: 10, serial: 1, mode: 0, ust: 10_000, msc: 1.5 },
    ],
  );
  assert.equal(joined.presentations.length, 0);
  assert.equal(joined.counts.invalidSubmissions, 1);
  assert.equal(joined.counts.invalidCompletions, 2);
});

test("regular updates cannot hide a physical delivery stall", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1_000_000),
    update(2, 16_667),
    paint(2, 0.2),
    presentation(2, 1_050_000),
  ]);
  assert.equal(evidence.missedUpdates, 0);
  assert.equal(evidence.updatesWithoutPresentationTime, 0);
  assert.equal(evidence.missedPresentations, 2);
  assert.equal(evidence.deliveryGaps[0].gapMicros, 50_000);
});

test("physical delivery respects refresh stride and motion endpoints", () => {
  const evidence = summarizeAnimationEvidence([
    display(144_000),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1_000_000),
    update(4, 20_833, false),
    paint(4, 1),
    presentation(4, 1_020_833),
    update(100, 1_000_000),
    paint(100, 0.2),
    presentation(100, 2_000_000),
  ]);
  assert.equal(evidence.missedPresentations, 0);
});

test("hidden painted values cannot establish visible animation delivery", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    { ...paint(1, 0.1), effectiveOpacity: 0 },
    presentation(1, 1_000_000),
    update(2, 16_667),
    { ...paint(2, 0.2), mapped: false },
    presentation(2, 1_050_000),
  ]);
  assert.equal(evidence.changedPaints, 0);
  assert.equal(evidence.updatesWithoutPaint, 2);
  assert.equal(evidence.missedPresentations, 0);
});

test("delivered but unchanged state remains explicit", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1_000_000),
    update(2, 16_667),
    paint(2, 0.1),
    presentation(2, 1_016_667),
  ]);
  assert.equal(evidence.missedPresentations, 0);
  assert.equal(evidence.unchangedPresentedStates, 1);
});

test("unchanged drawn pixels need no new presentation", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    update(2, 16_667),
    paint(2, 0.1),
    update(3, 33_333),
    paint(3, 0.1),
    update(4, 50_000),
    paint(4, 0.2),
    presentation(4, 50_001),
  ]);
  assert.equal(evidence.missedPresentations, 0);
  assert.equal(evidence.unchangedUpdatesWithoutPresentation, 2);
  assert.equal(evidence.updatesWithoutPresentationTime, 2);
});

test("repeated undelivered changes cannot establish unchanged pixels", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    update(2, 16_667),
    paint(2, 0.2),
    update(3, 33_333),
    paint(3, 0.2),
    update(4, 50_000),
    paint(4, 0.3),
    presentation(4, 50_001),
  ]);
  assert.equal(evidence.missedPresentations, 2);
  assert.equal(evidence.unchangedUpdatesWithoutPresentation, 0);
});

test("quiet observations before a delayed delivery cannot move into a window", () => {
  const evidence = summarizeAnimationEvidence(
    [
      display(),
      update(1, 0),
      paint(1, 0.1),
      presentation(1, 1),
      update(2, 16_667),
      paint(2, 0.2),
      presentation(2, 50_001),
      update(3, 33_333),
      paint(3, 0.2),
      update(4, 50_000),
      paint(4, 0.3),
      update(5, 66_667),
      paint(5, 0.4),
      update(6, 83_333),
      paint(6, 0.5),
      presentation(6, 100_001),
    ],
    undefined,
    { startMicros: 55_000, endMicros: 85_000 },
  );
  assert.equal(evidence.missedPresentations, 2);
});

test("a later changed observation revokes a quiet physical deadline", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    update(2, 17_000),
    paint(2, 0.1),
    update(3, 32_000),
    paint(3, 0.2),
    presentation(3, 50_001),
  ]);
  assert.equal(evidence.missedPresentations, 2);
});

test("matching observations after refresh cannot prove an earlier deadline quiet", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    { ...update(2, 16_667), observedMicros: 17_000 },
    paint(2, 0.1),
    { ...update(3, 33_333), observedMicros: 34_000 },
    paint(3, 0.1),
    update(4, 50_000),
    paint(4, 0.2),
    presentation(4, 50_001),
  ]);
  assert.equal(evidence.missedPresentations, 1);
});

test("a later reversion cannot excuse a change needed before that refresh", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    update(2, 16_000),
    paint(2, 0.2),
    update(3, 18_000),
    paint(3, 0.1),
    update(4, 50_000),
    paint(4, 0.3),
    presentation(4, 50_001),
  ]);
  assert.equal(evidence.missedPresentations, 1);
});

test("an unchanged interval does not hide a later delivery stall", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    update(2, 16_667),
    paint(2, 0.1),
    update(3, 33_333),
    paint(3, 0.2),
    presentation(3, 66_668),
  ]);
  assert.equal(evidence.missedPresentations, 2);
});

test("quiet delivery intervals are clipped to the measurement window", () => {
  const evidence = summarizeAnimationEvidence(
    [
      display(),
      update(1, 0),
      paint(1, 0.1),
      presentation(1, 1),
      update(2, 16_667),
      paint(2, 0.1),
      update(3, 33_333),
      paint(3, 0.2),
      update(4, 50_000),
      paint(4, 0.3),
      presentation(4, 50_001),
    ],
    undefined,
    { startMicros: 20_000, endMicros: 55_000 },
  );
  assert.equal(evidence.missedPresentations, 1);
});

test("quiet tail frames cannot hide an unobserved final update", () => {
  const evidence = summarizeAnimationEvidence(
    [
      display(),
      update(1, 0),
      paint(1, 0.1),
      presentation(1, 1),
      update(2, 16_667),
      paint(2, 0.1),
      update(3, 33_333),
      paint(3, 0.1),
      { event: "after-paint", frame: 5, observedMicros: 60_000 },
    ],
    undefined,
    { startMicros: 10_000, endMicros: 60_000 },
  );
  assert.equal(evidence.missedUpdates, 1);
  assert.equal(evidence.missedPresentations, 1);
});

test("measurement windows exclude warmup and retain delayed completions", () => {
  const entries = [display()];
  for (const [frame, time, delivered] of [
    [1, 0, 10_000],
    [2, 50_000, 60_000],
    [3, 66_667, 110_000],
  ]) {
    entries.push(
      update(frame, time),
      { ...paint(frame, frame / 10), frameTimeMicros: time },
      presentation(frame, delivered),
    );
  }
  const evidence = summarizeAnimationEvidence(entries, undefined, {
    startMicros: 50_000,
    endMicros: 70_000,
  });
  assert.equal(evidence.updates, 2);
  assert.equal(evidence.missedUpdates, 0);
  assert.equal(evidence.missedPresentations, 0);
  assert.equal(evidence.changedPaints, 2);
});

test("measurement windows retain preceding motion and clip missing deadlines", () => {
  const evidence = summarizeAnimationEvidence(
    [
      display(),
      update(1, 0),
      paint(1, 0.1),
      presentation(1, 1),
      update(4, 50_000),
      paint(4, 0.4),
      presentation(4, 50_001),
    ],
    undefined,
    { startMicros: 10_000, endMicros: 60_000 },
  );
  assert.equal(evidence.missedUpdates, 2);
  assert.equal(evidence.missedPresentations, 2);
});

test("an endpoint with unavailable delivery still ends motion", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1_000_000),
    update(2, 16_667, false),
    paint(2, 1),
    update(60, 1_000_000),
    paint(60, 0.1),
    presentation(60, 2_000_000),
  ]);
  assert.equal(evidence.updatesWithoutPresentationTime, 1);
  assert.equal(evidence.missedPresentations, 0);
});

test("an observed window end exposes motion that stopped delivering", () => {
  const evidence = summarizeAnimationEvidence(
    [
      display(),
      update(1, 0),
      { ...paint(1, 0.1), frameTimeMicros: 0 },
      presentation(1, 1),
      { event: "after-paint", frame: 5, observedMicros: 60_000 },
    ],
    undefined,
    { startMicros: 10_000, endMicros: 60_000 },
  );
  assert.equal(evidence.missedUpdates, 3);
  assert.equal(evidence.missedPresentations, 3);
});

test("hiding the presentation accounts for its preceding active interval", () => {
  for (const ending of [{ mapped: false }, { effectiveOpacity: 0 }]) {
    const evidence = summarizeAnimationEvidence(
      [
        display(),
        update(1, 0),
        { ...paint(1, 0.1), frameTimeMicros: 0 },
        presentation(1, 1),
        {
          ...update(5, 60_000),
          ...ending,
        },
      ],
      undefined,
      { startMicros: 10_000, endMicros: 50_000 },
    );
    assert.equal(evidence.missedUpdates, 2);
    assert.equal(evidence.missedPresentations, 2);
  }
});

test("hiding on the next refresh is not a missed visible frame", () => {
  const evidence = summarizeAnimationEvidence([
    display(),
    update(1, 0),
    paint(1, 0.1),
    presentation(1, 1),
    {
      ...update(2, 16_667),
      mapped: false,
      observedMicros: 16_700,
    },
  ]);
  assert.equal(evidence.missedUpdates, 0);
  assert.equal(evidence.missedPresentations, 0);
});
