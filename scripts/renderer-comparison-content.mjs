import { readFile, stat } from "node:fs/promises";

export async function contentEvidence(file, run) {
  const invalid = (message) =>
    Object.assign(new Error(message), { invalidEvidence: true });
  let size;
  try {
    size = (await stat(file)).size;
  } catch {
    throw invalid("content evidence is missing");
  }
  if (size > 64 * 1024 * 1024)
    throw invalid("content evidence exceeds its retained size bound");
  let entries;
  try {
    entries = (await readFile(file, "utf8"))
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
  } catch {
    throw invalid("content evidence is malformed or incomplete");
  }
  const header = entries[0],
    footer = entries.at(-1);
  if (
    header?.event !== "start" ||
    header.version !== 1 ||
    header.clock !== "CLOCK_MONOTONIC" ||
    header.physicalDelivery !== false
  )
    throw invalid("unsupported content evidence header");
  if (
    footer?.event !== "end" ||
    footer.complete !== true ||
    footer.lost !== 0 ||
    footer.records !== entries.length - 2
  )
    throw invalid("content evidence is incomplete or overflowed");
  const rows = entries.slice(1, -1);
  const events = new Set([
    "snapshot-received",
    "preparation-requested",
    "preparation-started",
    "preparation-completed",
    "preparation-adopted",
    "preparation-superseded",
    "artwork-started",
    "artwork-completed",
    "resource-created",
    "resource-released",
    "native-draw",
  ]);
  if (
    rows.some(
      (row) =>
        !events.has(row.event) ||
        !Number.isSafeInteger(row.observedMicros) ||
        row.observedMicros < 0,
    )
  )
    throw invalid("invalid content observation identity or timestamp");
  const identity = (value) => Number.isSafeInteger(value) && value >= 0;
  for (const row of rows) {
    if (
      (row.event.startsWith("preparation-") && !identity(row.token)) ||
      ([
        "snapshot-received",
        "preparation-requested",
        "preparation-started",
        "preparation-completed",
      ].includes(row.event) &&
        !identity(row.revision)) ||
      (row.event.startsWith("resource-") && !identity(row.resource)) ||
      (row.event === "snapshot-received" &&
        (!identity(row.receivedMicros) ||
          row.receivedMicros > row.observedMicros)) ||
      (["preparation-completed", "artwork-completed"].includes(row.event) &&
        typeof row.success !== "boolean") ||
      (row.event.startsWith("artwork-") && typeof row.path !== "string")
    )
      throw invalid("invalid content observation fields");
  }
  rows.sort((a, b) => a.observedMicros - b.observedMicros);
  const seen = new Set();
  const starts = new Map();
  for (const row of rows) {
    if (
      [
        "preparation-requested",
        "preparation-started",
        "preparation-completed",
        "snapshot-received",
      ].includes(row.event)
    ) {
      const key = `${row.event}:${row.event === "snapshot-received" ? row.revision : row.token}`;
      if (seen.has(key)) throw invalid("duplicate content milestone identity");
      seen.add(key);
    }
    if (row.event === "preparation-started") starts.set(row.token, row);
    if (
      row.event === "preparation-completed" &&
      (!starts.has(row.token) ||
        starts.get(row.token).revision !== row.revision)
    )
      throw invalid("preparation completion has no matching start");
  }

  const requested = new Map(
    rows
      .filter((row) => row.event === "preparation-requested")
      .map((row) => [row.token, row]),
  );
  const discarded = new Map(
    rows
      .filter((row) => row.event === "preparation-superseded")
      .map((row) => [row.token, row]),
  );
  const draws = rows.filter((row) => row.event === "native-draw");
  const failures = [];
  const firstDraw = new Map();
  for (const draw of draws) {
    if (
      !identity(draw.frame) ||
      !identity(draw.scene) ||
      !identity(draw.revision) ||
      !identity(draw.frameTimeMicros) ||
      !identity(draw.preparedToken) ||
      typeof draw.drawing?.ready !== "boolean" ||
      !Array.isArray(draw.drawing?.artwork) ||
      !(
        draw.drawing.targetArtwork === null ||
        identity(draw.drawing.targetArtwork)
      ) ||
      draw.drawing.artwork.some(
        (pair) =>
          !Array.isArray(pair) ||
          pair.length !== 2 ||
          !identity(pair[0]) ||
          !Number.isFinite(pair[1]) ||
          pair[1] < 0,
      )
    )
      throw invalid("invalid native draw identity");
    const appears =
      draw.drawing.targetArtwork === null
        ? draw.drawing.ready
        : draw.drawing.artwork.some(
            ([resource, weight]) =>
              resource === draw.drawing.targetArtwork && weight > 0,
          );
    if (!appears) continue;
    if (!firstDraw.has(draw.preparedToken))
      firstDraw.set(draw.preparedToken, draw);
    const superseded = discarded.get(draw.preparedToken);
    if (
      superseded &&
      firstDraw.get(draw.preparedToken) === draw &&
      draw.frameTimeMicros >= superseded.observedMicros
    )
      failures.push(
        `Superseded preparation ${draw.preparedToken} first drew after supersession`,
      );
  }
  const measured = run.publications.filter(
    (publication) => publication.phase === "measurement",
  );
  const publications = measured.map((publication) => {
    const received = rows.find(
      (row) =>
        row.event === "snapshot-received" &&
        row.revision === publication.revision,
    );
    const requests = [...requested.values()].filter(
      (row) => row.revision === publication.revision,
    );
    const tokens = new Set(requests.map((request) => request.token));
    const started = rows.find(
      (row) => row.event === "preparation-started" && tokens.has(row.token),
    );
    const completed = rows.find(
      (row) => row.event === "preparation-completed" && tokens.has(row.token),
    );
    const draw = draws.find(
      (row) => row.revision === publication.revision && row.drawing.ready,
    );
    const superseded =
      !!requests.find((request) => discarded.has(request.token)) ||
      (!draw &&
        !!rows.find(
          (row) =>
            row.event === "snapshot-received" &&
            row.revision > publication.revision,
        ));
    const outcome = draw
      ? draw.observedMicros > run.windows.measurement.endMicros
        ? "delayed"
        : "ready"
      : completed?.success === false
        ? "failed"
        : superseded
          ? "superseded"
          : received
            ? "incomplete"
            : "unavailable";
    const delta = (end, start) => {
      if (!Number.isFinite(end) || !Number.isFinite(start)) return null;
      if (end < start)
        throw invalid(
          "content milestone precedes its publication or preparation",
        );
      return (end - start) / 1000;
    };
    const artwork = rows.find(
      (row) =>
        row.event === "artwork-completed" &&
        row.path === publication.artwork?.path,
    );
    if (publication.asset?.fresh && draw && !artwork?.success)
      failures.push(
        `Fresh publication ${publication.revision} drew without successful uncached artwork preparation`,
      );
    if (completed?.success === false)
      failures.push(
        `Preparation failed for publication ${publication.revision}`,
      );
    return {
      revision: publication.revision,
      publishedMicros: publication.publishedMicros,
      contentId: publication.contentId,
      artwork: publication.artwork,
      fresh: publication.asset?.fresh ?? false,
      outcome,
      receivedMicros: received?.receivedMicros ?? null,
      preparationToken: requests[0]?.token ?? null,
      preparationStartedMicros: started?.observedMicros ?? null,
      preparationCompletedMicros: completed?.observedMicros ?? null,
      nativeDrawObservedMicros: draw?.observedMicros ?? null,
      frame: draw?.frame ?? null,
      scene: draw?.scene ?? null,
      frameTimeMicros: draw?.frameTimeMicros ?? null,
      artworkResource:
        draw?.drawing.targetArtwork ?? completed?.artworkResource ?? null,
      preparationMs: delta(completed?.observedMicros, started?.observedMicros),
      publicationToPreparedMs: delta(
        completed?.observedMicros,
        publication.publishedMicros,
      ),
      publicationToNativeDrawMs: delta(
        draw?.observedMicros,
        publication.publishedMicros,
      ),
      uncachedArtworkPrepared: artwork?.success ?? false,
    };
  });
  if (run.workload === "reused-content") {
    const resources = new Map();
    for (const publication of publications.filter(
      (publication) => publication.artworkResource !== null,
    )) {
      const key = JSON.stringify(publication.artwork);
      if (
        resources.has(key) &&
        resources.get(key) !== publication.artworkResource
      )
        failures.push(
          `Repeated artwork ${publication.contentId} did not reuse its prepared resource`,
        );
      resources.set(key, publication.artworkResource);
    }
  }
  const live = new Set();
  let peak = 0;
  const lifetimes = [];
  for (const row of rows) {
    if (row.event === "resource-created") {
      if (live.has(row.resource))
        throw invalid("duplicate prepared resource identity");
      live.add(row.resource);
    } else if (row.event === "resource-released") {
      if (!live.delete(row.resource))
        throw invalid("unknown released resource identity");
    } else continue;
    peak = Math.max(peak, live.size);
    lifetimes.push({ observedMicros: row.observedMicros, retained: live.size });
  }
  if (live.size)
    failures.push(
      `${live.size} texture handles remained retained after native shutdown`,
    );
  const waitingDraws = draws.filter(
    (draw) =>
      !draw.drawing.ready &&
      draw.observedMicros >= run.windows.measurement.startMicros &&
      draw.observedMicros <= run.windows.measurement.endMicros,
  );
  return {
    status: failures.length ? "behavior-failed" : "complete",
    physicalDelivery: false,
    clock: header.clock,
    publications,
    failures,
    resources: {
      scope:
        "Rust texture handle lifetime; GPU retirement fences and GPU memory are not observed",
      peakRetained: peak,
      retainedAfterShutdown: live.size,
      lifetimes,
    },
    continuingPresentation: {
      drawsWhileIncomingNotReady: waitingDraws.length,
      distinctProgressValues: new Set(
        waitingDraws
          .map((draw) => draw.drawing.progress)
          .filter(Number.isFinite),
      ).size,
    },
    limitations: [
      "Native draw callbacks are readiness observations, not physical first-visible-content latency",
      "Missing readiness before shutdown remains incomplete; delayed observations belong to the original publication window",
      "Sampled RSS can miss short transients; handle release does not prove immediate GPU allocation retirement",
    ],
  };
}
