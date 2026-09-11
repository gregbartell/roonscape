import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

// State and delivery are independent evidence. In particular, predicted times
// and after-paint notifications cannot substitute for presentation timestamps.
export function summarizeAnimationEvidence(
  entries,
  externalPresentation,
  window,
) {
  if (window) {
    if (!(
      Number.isSafeInteger(window.startMicros) &&
      Number.isSafeInteger(window.endMicros) &&
      window.endMicros > window.startMicros
    )) {
      throw new Error(
        "Measurement window requires increasing integer microsecond bounds",
      );
    }
  }
  const association = externalPresentation
    ? associatePresentationEvents(
        externalPresentation.submissions,
        externalPresentation.completions,
      )
    : undefined;
  if (association) entries = [...entries, ...association.presentations];
  const display = entries.find((entry) => entry.event === "display");
  const refresh = display?.monitorRefreshMillihertz;
  if (!(refresh > 0)) {
    throw new Error("Animation evidence has no physical monitor refresh rate");
  }
  const stride = Math.ceil(refresh / 60_000);
  const interval = (stride * 1_000_000_000) / refresh;
  const inWindow = (time) =>
    !window || (time >= window.startMicros && time < window.endMicros);
  // Retain lifecycle context on both sides of a measurement window. Only the
  // missing deadlines inside its bounds contribute to its totals.
  const deadlineBounds = (start, periods) => {
    const first = window
      ? Math.max(1, Math.ceil((window.startMicros - start) / interval))
      : 1;
    const last = window
      ? Math.min(
          periods - 1,
          Math.ceil((window.endMicros - start) / interval) - 1,
        )
      : periods - 1;
    return { first, last };
  };
  const missedDeadlines = (start, periods) => {
    const { first, last } = deadlineBounds(start, periods);
    return Math.max(0, last - first + 1);
  };
  const presented = new Map();
  let previousUpdate;
  let previousDelivery;
  let previousPaint;
  const frameStates = new Map();
  const frameTimes = new Map();
  const presentationOrigins = new Map();
  const misses = [];
  const renderStartGaps = [];
  const deliveryGaps = [];
  const observedDeliveryGaps = [];
  const missedPhysicalDeadlines = new Set();
  let changedPaints = 0;
  let unchangedPaints = 0;
  let updates = 0;
  let updatesWithoutPaint = 0;
  let updatesWithoutPresentationTime = 0;
  let unchangedUpdatesWithoutPresentation = 0;
  let unchangedPresentedStates = 0;

  const recordDeliveryGap = (prior, frame, gapMicros, periods) => {
    const gap = {
      previousFrame: prior.delivery.frame,
      frame,
      gapMicros,
      missedPresentations: 0,
    };
    const { first, last } = deadlineBounds(
      prior.delivery.presentedMicros,
      periods,
    );
    for (let period = first; period <= last; period += 1) {
      if (prior.deadlineStates.get(period) === true) continue;
      gap.missedPresentations += 1;
      const delivery = prior.delivery;
      const deadline = Number.isSafeInteger(delivery.displaySequence)
        ? delivery.displaySequence + period * stride
        : Math.round(
            (delivery.presentedMicros -
              presentationOrigins.get(delivery.window)) /
              interval,
          ) + period;
      missedPhysicalDeadlines.add(
        JSON.stringify([
          delivery.window,
          Number.isSafeInteger(delivery.displaySequence) ? "sequence" : "time",
          deadline,
        ]),
      );
    }
    if (
      !window ||
      (prior.delivery.presentedMicros < window.endMicros &&
        prior.delivery.presentedMicros + gapMicros > window.startMicros)
    )
      observedDeliveryGaps.push(gap);
    if (gap.missedPresentations > 0) deliveryGaps.push(gap);
  };

  const finishMotion = (time, atRefresh = true) => {
    const periodsUntil = (start) =>
      (atRefresh ? Math.round : Math.ceil)((time - start) / interval);
    const update = previousUpdate;
    if (update && update.values.animationActive !== false) {
      const missing = missedDeadlines(
        update.frameTimeMicros,
        periodsUntil(update.frameTimeMicros),
      );
      if (missing > 0)
        misses.push({
          previousFrame: update.frame,
          frame: null,
          source: update.source,
          missedUpdates: missing,
          gapMicros: time - update.frameTimeMicros,
        });
    }
    const prior = previousDelivery;
    if (prior?.active) {
      const delivery = prior.delivery;
      recordDeliveryGap(
        prior,
        null,
        time - delivery.presentedMicros,
        periodsUntil(delivery.presentedMicros),
      );
    }
    previousUpdate = undefined;
    previousDelivery = undefined;
  };

  for (const entry of entries) {
    if (entry.event === "presentation") presented.set(entry.frame, entry);
    if (
      entry.event === "presentation" &&
      entry.presentedMicros > 0 &&
      !presentationOrigins.has(entry.window)
    )
      presentationOrigins.set(entry.window, entry.presentedMicros);
    if (entry.event === "state")
      frameTimes.set(entry.frame, entry.frameTimeMicros);
    if (entry.event !== "state" || entry.source !== "composition-painted")
      continue;
    if (entry.mapped === false || entry.effectiveOpacity === 0) continue;
    // Each native frame records the complete immutable scene once.
    frameStates.set(entry.frame, JSON.stringify(entry.values));
  }
  for (const [frame, state] of frameStates) {
    if (previousPaint !== undefined && inWindow(frameTimes.get(frame))) {
      if (previousPaint === state) unchangedPaints += 1;
      else changedPaints += 1;
    }
    previousPaint = state;
  }
  const associated = new Map(
    association?.presentations.map((entry) => [entry.frame, entry]) ?? [],
  );
  const scenes = new Map(
    entries
      .filter((entry) => entry.event === "after-paint")
      .map((entry) => [entry.frame, entry.scene]),
  );
  const consecutiveAdvancingPresentations = (previous, entry) => {
    if (display.frameTimeSource !== "render-start") return false;
    const before = associated.get(previous.frame);
    const after = associated.get(entry.frame);
    if (!before || !after || before.window !== after.window) return false;
    if (after.displaySequence - before.displaySequence !== stride) return false;
    if (
      ![before, after].every(
        (delivery) =>
          Number.isSafeInteger(delivery.targetDisplaySequence) &&
          delivery.targetDisplaySequence > 0 &&
          delivery.displaySequence === delivery.targetDisplaySequence,
      )
    )
      return false;
    if (
      !Number.isSafeInteger(scenes.get(previous.frame)) ||
      !Number.isSafeInteger(scenes.get(entry.frame)) ||
      scenes.get(previous.frame) === scenes.get(entry.frame)
    )
      return false;
    const beforeState = frameStates.get(previous.frame);
    const afterState = frameStates.get(entry.frame);
    return (
      beforeState !== undefined &&
      afterState !== undefined &&
      beforeState !== afterState
    );
  };
  for (const entry of entries) {
    if (entry.event !== "state") continue;
    if (entry.source !== "scheduled-update") continue;
    const selected = inWindow(entry.frameTimeMicros);
    if (selected) updates += 1;
    if (entry.mapped === false || entry.effectiveOpacity === 0) {
      finishMotion(entry.observedMicros ?? entry.frameTimeMicros);
      continue;
    }
    const previous = previousUpdate;
    if (previous !== undefined && previous.values.animationActive !== false) {
      const periods = Math.round(
        (entry.frameTimeMicros - previous.frameTimeMicros) / interval,
      );
      const missing = missedDeadlines(previous.frameTimeMicros, periods);
      if (missing > 0) {
        const gap = {
          frame: entry.frame,
          previousFrame: previous.frame,
          source: entry.source,
          gapMicros: entry.frameTimeMicros - previous.frameTimeMicros,
        };
        // A CPU render start is not a planned display deadline. Independent
        // completions can prove that distinct advancing scenes still reached
        // consecutive eligible refreshes, with neither frame late. Preserve
        // the CPU anomaly and remain conservative without that complete proof.
        if (consecutiveAdvancingPresentations(previous, entry))
          renderStartGaps.push({ ...gap, apparentMissedUpdates: missing });
        else misses.push({ ...gap, missedUpdates: missing });
      }
    }
    previousUpdate = entry;
    const state = frameStates.get(entry.frame);
    if (state === undefined && selected) updatesWithoutPaint += 1;
    const delivery = presented.get(entry.frame);
    const prior = previousDelivery;
    // Only a complete scene matching an actual delivery proves an interval
    // quiet. Repeating a change never delivered cannot do so.
    const unchanged = prior && state !== undefined && prior.state === state;
    const observedTime = entry.observedMicros ?? entry.frameTimeMicros;
    if (prior && Number.isSafeInteger(observedTime)) {
      // Keep observations on the actual refresh grid. A later matching value
      // cannot prove a past deadline quiet: motion may have changed and then
      // returned between observations. Newer state supersedes the next plan.
      const position =
        (observedTime - prior.delivery.presentedMicros) / interval;
      prior.deadlineStates.set(Math.ceil(position), Boolean(unchanged));
    }
    if (!(delivery?.presentedMicros > 0)) {
      if (selected) updatesWithoutPresentationTime += 1;
      if (selected && unchanged) unchangedUpdatesWithoutPresentation += 1;
      if (entry.values.animationActive === false) previousDelivery = undefined;
      continue;
    }
    if (state === undefined) {
      previousDelivery = undefined;
      continue;
    }
    if (prior && prior.active && prior.delivery.window === delivery.window) {
      const gapMicros =
        delivery.presentedMicros - prior.delivery.presentedMicros;
      const periods = Math.round(
        Number.isSafeInteger(delivery.displaySequence) &&
          Number.isSafeInteger(prior.delivery.displaySequence)
          ? (delivery.displaySequence - prior.delivery.displaySequence) / stride
          : gapMicros / interval,
      );
      recordDeliveryGap(prior, delivery.frame, gapMicros, periods);
      if (unchanged && inWindow(delivery.presentedMicros))
        unchangedPresentedStates += 1;
    }
    previousDelivery = {
      delivery,
      state,
      deadlineStates: new Map(),
      active: entry.values.animationActive !== false,
    };
  }
  const observedThrough = entries.reduce(
    (latest, entry) =>
      Math.max(latest, entry.observedMicros ?? 0, entry.presentedMicros ?? 0),
    0,
  );
  if (window && observedThrough >= window.endMicros) {
    finishMotion(window.endMicros, false);
  }
  return {
    renderer: display.renderer,
    ...(window && { window }),
    refreshMillihertz: refresh,
    refreshStride: stride,
    scheduledFramesPerSecond: refresh / 1_000 / stride,
    updates,
    missedUpdates: misses.reduce((sum, miss) => sum + miss.missedUpdates, 0),
    misses,
    renderStartGaps,
    changedPaints,
    unchangedPaints,
    updatesWithoutPaint,
    updatesWithoutPresentationTime,
    unchangedUpdatesWithoutPresentation,
    // Stop and delivery ranges can overlap. Their diagnostic counts describe
    // each range; the total counts every physical deadline only once.
    // Deadlines without evidence of unchanged pixels count conservatively;
    // missing observations cannot establish whether a change was required.
    missedPresentations: missedPhysicalDeadlines.size,
    deliveryGaps,
    observedDeliveryGaps,
    unchangedPresentedStates,
    ...(association && { presentationAssociation: association.counts }),
  };
}

// X Present completion serials belong to a window. A nearby timestamp is not
// evidence that a completion delivered a particular rendered frame.
export function associatePresentationEvents(submissions, completions) {
  const requests = new Map();
  const frames = new Map();
  const counts = {
    submissions: submissions.length,
    completions: completions.length,
    invalidSubmissions: 0,
    unmatchedCompletions: 0,
    ambiguousCompletions: 0,
    skippedCompletions: 0,
    invalidCompletions: 0,
    ambiguousFrames: 0,
    associatedFrames: 0,
  };
  const validIdentity = (entry) =>
    Number.isSafeInteger(entry.window) &&
    entry.window > 0 &&
    Number.isSafeInteger(entry.serial) &&
    entry.serial >= 0;
  for (const submission of submissions) {
    if (!validIdentity(submission)) {
      counts.invalidSubmissions += 1;
      continue;
    }
    const key = `${submission.window}:${submission.serial}`;
    if (!requests.has(key)) requests.set(key, []);
    requests.get(key).push(submission);
  }
  for (const completion of completions) {
    if (!validIdentity(completion)) {
      counts.invalidCompletions += 1;
      continue;
    }
    const candidates = requests.get(
      `${completion.window}:${completion.serial}`,
    );
    if (!candidates) {
      counts.unmatchedCompletions += 1;
      continue;
    }
    if (candidates.length !== 1) {
      counts.ambiguousCompletions += 1;
      continue;
    }
    const submission = candidates[0];
    if (completion.mode === 2) {
      counts.skippedCompletions += 1;
      continue;
    }
    if (
      ![0, 1, 3].includes(completion.mode) ||
      !Number.isSafeInteger(submission.frame) ||
      submission.frame < 0 ||
      !Number.isSafeInteger(completion.ust) ||
      completion.ust <= 0 ||
      !Number.isSafeInteger(completion.msc) ||
      completion.msc < 0
    ) {
      counts.invalidCompletions += 1;
      continue;
    }
    if (!frames.has(submission.frame)) frames.set(submission.frame, []);
    frames.get(submission.frame).push({
      event: "presentation",
      frame: submission.frame,
      presentedMicros: completion.ust,
      displaySequence: completion.msc,
      ...(Number.isSafeInteger(submission.targetMsc) && {
        targetDisplaySequence: submission.targetMsc,
      }),
      window: completion.window,
      source: "x-present",
    });
  }
  const presentations = [];
  for (const candidates of frames.values()) {
    if (candidates.length !== 1) counts.ambiguousFrames += 1;
    else presentations.push(candidates[0]);
  }
  counts.associatedFrames = presentations.length;
  return { presentations, counts };
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  if (![3, 5].includes(process.argv.length))
    throw new Error(
      "Usage: node scripts/animation-evidence.mjs EVIDENCE.jsonl [SUBMISSIONS.jsonl COMPLETIONS.jsonl]",
    );
  const readEntries = async (path) =>
    (await readFile(path, "utf8"))
      .split("\n")
      .filter((line) => line.trim())
      .map((line) => JSON.parse(line));
  const [entries, submissions, completions] = await Promise.all(
    process.argv.slice(2).map(readEntries),
  );
  console.log(
    JSON.stringify(
      summarizeAnimationEvidence(
        entries,
        submissions && { submissions, completions },
      ),
      null,
      2,
    ),
  );
}
