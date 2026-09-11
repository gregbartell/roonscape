import {
  associatePresentationEvents,
  summarizeAnimationEvidence,
} from "./animation-evidence.mjs";

// Keep partial associations for diagnosis, but never return a cadence zero/pass
// when an in-window drawn scene lacks an unambiguous independent completion.
export function summarizePhysicalPresentation({
  trace,
  animation,
  draws,
  publications,
  window,
  maxMissedRefreshes = null,
  verticalBlankMicros = 0,
}) {
  const problems = [],
    unavailable = [];
  const header = trace[0],
    footer = trace.at(-1);
  const invalid = (reason) => problems.push(reason);
  if (
    header?.event !== "start" ||
    header.version !== 1 ||
    header.clock !== "CLOCK_MONOTONIC"
  )
    invalid("unsupported collector header");
  if (header?.identitySupported !== true)
    unavailable.push("build does not expose submission identities");
  if (
    footer?.event !== "end" ||
    footer.complete !== true ||
    footer.lost !== 0 ||
    footer.records !== trace.length - 2
  )
    invalid("collector incomplete, overflowed, or missing records");
  const rows = trace.slice(1, -1);
  const integer = (value) => Number.isSafeInteger(value) && value >= 0;
  if (!integer(verticalBlankMicros)) invalid("invalid vertical blank interval");
  const selected = (time) =>
    time >= window.startMicros && time < window.endMicros;
  for (const row of rows) {
    if (
      !["submission", "completion", "subscribed"].includes(row.event) ||
      !integer(row.observedMicros) ||
      !integer(row.window) ||
      row.window === 0
    )
      invalid("malformed collector event");
  }
  const submissions = rows.filter((row) => row.event === "submission");
  const completions = rows.filter((row) => row.event === "completion");
  const association = associatePresentationEvents(submissions, completions);
  const requestsFor = index(submissions, (row) => row.frame);
  const requestsBySerial = index(submissions, presentKey);
  const completionsFor = index(completions, presentKey);
  const subscribed = new Map(
    rows
      .filter((row) => row.event === "subscribed")
      .map((row) => [row.window, row.observedMicros]),
  );
  const painted = animation.filter((row) => row.event === "after-paint");
  const scenesFor = index(painted, (row) => row.frame);
  const statesFor = index(
    animation.filter((row) => row.source === "composition-painted"),
    (row) => row.frame,
  );
  const updatesFor = index(
    animation.filter((row) => row.source === "scheduled-update"),
    (row) => row.frame,
  );
  const nativeDraws = draws.filter((row) => row.event === "native-draw");
  const contentFor = index(nativeDraws, (row) => row.frame);
  const drawsForRevision = index(nativeDraws, (row) => row.revision);
  // GUI delivery of after-paint can lag the actual submission/completion.
  const relevantFrames = new Set(
    submissions
      .filter(
        (row) =>
          selected(row.observedMicros) ||
          completionsFor(presentKey(row)).some((completion) =>
            selected(completion.ust),
          ),
      )
      .map((row) => row.frame),
  );
  const measured = painted.filter(
    (row) => selected(row.observedMicros) || relevantFrames.has(row.frame),
  );
  const preceding = painted
    .filter((row) => row.observedMicros < window.startMicros)
    .at(-1);
  if (preceding && !measured.includes(preceding)) measured.unshift(preceding);
  if (!(footer?.observedMicros >= window.endMicros))
    invalid("collector stopped before the measurement window ended");
  const physicalFrames = new Set(
    submissions
      .filter((request) =>
        completionsFor(presentKey(request)).some((row) => row.mode === 1),
      )
      .map((row) => row.frame),
  );
  const delivered = new Map(
    association.presentations
      .filter((row) => physicalFrames.has(row.frame))
      .map((row) => [row.frame, row]),
  );
  if (
    ![...delivered.values()].some(
      (row) => row.presentedMicros < window.endMicros,
    )
  )
    unavailable.push("no confirmed physical delivery before the window ended");
  if (!submissions.length || !measured.length)
    unavailable.push("no submitted and drawn native frames");
  for (const request of submissions.filter((row) =>
    selected(row.observedMicros),
  )) {
    const scene = scenesFor(request.frame);
    if (scene.length !== 1)
      invalid(`missing or ambiguous drawn scene for frame ${request.frame}`);
  }
  for (const frame of submissions.length ? measured : []) {
    const states = statesFor(frame.frame);
    const updates = updatesFor(frame.frame);
    const content = contentFor(frame.frame);
    if (
      states.length !== 1 ||
      !states[0].values ||
      updates.length !== 1 ||
      typeof updates[0].values?.animationActive !== "boolean"
    )
      invalid(`missing or ambiguous drawn values for frame ${frame.frame}`);
    if (content.length !== 1 || content[0].scene !== frame.scene)
      invalid(
        `missing or ambiguous native content identity for frame ${frame.frame}`,
      );
    const requests = requestsFor(frame.frame);
    if (requests.length !== 1 || requests[0].scene !== frame.scene) {
      invalid(`missing or ambiguous submission for frame ${frame.frame}`);
      continue;
    }
    const request = requests[0];
    if (!(subscribed.get(request.window) <= window.startMicros))
      invalid("completion observation began after measurement");
    const matches = completionsFor(presentKey(request));
    if (matches.length !== 1) {
      invalid(`missing or ambiguous completion for frame ${frame.frame}`);
      continue;
    }
    const completion = matches[0];
    if (completion.mode !== 1 && completion.mode !== 2)
      unavailable.push(
        "requires scanout flip completions; copy does not establish physical delivery",
      );
    if (request.options & 1)
      unavailable.push("asynchronous presentation is unsupported");
    if (
      !integer(request.scene) ||
      !integer(request.options) ||
      !integer(request.targetMsc) ||
      !integer(completion.ust) ||
      !integer(completion.msc) ||
      completion.ust < request.observedMicros ||
      completion.observedMicros < request.observedMicros ||
      // DRM timestamps reference the end of vblank, which can still be in
      // the future when the flip event arrives. Bound this by mode timings.
      completion.observedMicros + verticalBlankMicros < completion.ust
    )
      invalid("invalid presentation clock or request fields");
    if (completion.mode === 1 && !delivered.has(frame.frame))
      invalid(`unassociated frame ${frame.frame}`);
  }
  for (const completion of completions.filter((row) => selected(row.ust))) {
    if (requestsBySerial(presentKey(completion)).length !== 1)
      invalid("unmatched or ambiguous in-window completion");
  }
  const refresh = animation.find(
    (row) => row.event === "display",
  )?.monitorRefreshMillihertz;
  const orderedDeliveries = [...delivered.values()].sort(
    (a, b) => a.frame - b.frame,
  );
  for (let index = 1; index < orderedDeliveries.length; index++) {
    const before = orderedDeliveries[index - 1],
      after = orderedDeliveries[index];
    if (before.window !== after.window) {
      invalid("multiple presentation windows are unsupported");
      continue;
    }
    const sequence = after.displaySequence - before.displaySequence;
    const elapsed = after.presentedMicros - before.presentedMicros;
    if (sequence <= 0 || elapsed <= 0)
      invalid("presentation sequence or timestamp moved backward or repeated");
    if (
      refresh > 0 &&
      Math.abs(elapsed - (sequence * 1_000_000_000) / refresh) >
        Math.max(1000, (1_000_000_000 / refresh) * 0.05)
    )
      invalid(
        "presentation clock disagrees with fixed refresh; variable refresh or changed display conditions are unsupported",
      );
  }
  const content = publications.map((publication) => {
    const candidates = drawsForRevision(publication.revision).filter(
      (draw) =>
        draw.drawing.ready ||
        (publication.preparationToken > 0 &&
          draw.preparedToken === publication.preparationToken &&
          publication.artworkResource != null &&
          draw.drawing.artwork?.some(
            ([resource, weight]) =>
              resource === publication.artworkResource && weight > 0,
          )),
    );
    const visible = candidates
      .map((draw) => ({ draw, presentation: delivered.get(draw.frame) }))
      .filter(
        ({ draw, presentation }) =>
          presentation &&
          requestsFor(draw.frame).some((row) => row.scene === draw.scene),
      )
      .sort(
        (a, b) =>
          a.presentation.presentedMicros - b.presentation.presentedMicros,
      )[0];
    const firstCandidates = candidates.filter(
      (draw) =>
        !visible ||
        (draw.frameTimeMicros ?? draw.observedMicros) <=
          visible.presentation.presentedMicros,
    );
    const observations = firstCandidates.map((draw) => {
      const requests = requestsFor(draw.frame);
      const keys = new Set(requests.map(presentKey));
      return {
        requests,
        completions: [...keys].flatMap((key) => completionsFor(key)),
      };
    });
    const unsupported = observations.some((observation) =>
      observation.completions.some((row) => ![1, 2].includes(row.mode)),
    );
    const missing = observations.some(
      (observation) =>
        !observation.requests.length || !observation.completions.length,
    );
    const ambiguous = observations.some(
      (observation) =>
        observation.requests.length > 1 || observation.completions.length > 1,
    );
    const time = visible?.presentation.presentedMicros;
    if (time !== undefined && time < publication.publishedMicros)
      invalid("presentation precedes publication");
    let outcome;
    if (!submissions.length || unsupported) outcome = "unavailable";
    else if (ambiguous) outcome = "ambiguous";
    else if (missing) outcome = "missing";
    else if (visible)
      outcome = time >= window.endMicros ? "delayed" : "presented";
    else if (
      publication.outcome === "superseded" ||
      draws.some(
        (next) => next.revision > publication.revision && next.drawing?.ready,
      )
    )
      outcome = "superseded";
    else outcome = candidates.length ? "missing" : "unavailable";
    return {
      ...publication,
      readinessOutcome: publication.outcome,
      outcome,
      presentedMicros:
        unsupported || ambiguous || missing ? null : (time ?? null),
      publicationToPresentedMs:
        unsupported || ambiguous || missing || time === undefined
          ? null
          : (time - publication.publishedMicros) / 1000,
      frame: visible?.draw.frame ?? null,
      scene: visible?.draw.scene ?? null,
      displaySequence: visible?.presentation.displaySequence ?? null,
    };
  });
  let cadence = null;
  if (!problems.length && !unavailable.length) {
    try {
      cadence = summarizeAnimationEvidence(
        [
          ...animation,
          { event: "observation-end", observedMicros: footer.observedMicros },
        ],
        { submissions, completions },
        window,
      );
    } catch (error) {
      invalid(error.message);
    }
  }
  const status = problems.length
    ? "invalid-evidence"
    : unavailable.length
      ? "unavailable"
      : "complete";
  return {
    status,
    label: "presentation evidence; not optical verification",
    window,
    reasons: [...new Set([...problems, ...unavailable])],
    associations: association,
    publications: content,
    cadence,
    worstDeliveryGapMicros: cadence
      ? Math.max(0, ...cadence.observedDeliveryGaps.map((gap) => gap.gapMicros))
      : null,
    worstStallMicros: cadence
      ? Math.max(0, ...cadence.deliveryGaps.map((gap) => gap.gapMicros))
      : null,
    contract: {
      maxMissedRefreshes,
      status:
        status !== "complete"
          ? "unavailable"
          : maxMissedRefreshes === null
            ? "not-requested"
            : cadence.missedPresentations > maxMissedRefreshes
              ? "failed"
              : "passed",
    },
    instrumentation: {
      enqueueMicros: footer?.enqueueMicros ?? null,
      submissions: footer?.submissions ?? null,
      capacity: header?.capacity ?? null,
    },
  };
}

function presentKey(row) {
  return `${row.window}:${row.serial}`;
}
function index(rows, key) {
  const groups = new Map();
  for (const row of rows) {
    const identity = key(row);
    if (!groups.has(identity)) groups.set(identity, []);
    groups.get(identity).push(row);
  }
  return (identity) => groups.get(identity) ?? [];
}
