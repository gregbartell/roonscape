import { writeFile } from "node:fs/promises";
import path from "node:path";

const metrics = ["cpuSeconds", "cpuPercent", "peakRssBytes", "rssChangeBytes"];
function summary(values) {
  const available = values.filter(Number.isFinite);
  if (!available.length)
    return { n: 0, mean: null, min: null, max: null, standardDeviation: null };
  const mean = available.reduce((a, b) => a + b, 0) / available.length;
  return {
    n: available.length,
    mean,
    min: Math.min(...available),
    max: Math.max(...available),
    standardDeviation:
      available.length < 2
        ? null
        : Math.sqrt(
            available.reduce((sum, value) => sum + (value - mean) ** 2, 0) /
              (available.length - 1),
          ),
  };
}
function compareSummaries(baseline, candidate) {
  const absoluteDelta =
    baseline.mean === null || candidate.mean === null
      ? null
      : candidate.mean - baseline.mean;
  return {
    baseline,
    candidate,
    absoluteDelta,
    relativeDeltaPercent:
      absoluteDelta === null || baseline.mean === 0
        ? null
        : (absoluteDelta / Math.abs(baseline.mean)) * 100,
  };
}
function outcomeCounts(publications) {
  const counts = {};
  for (const publication of publications)
    counts[publication.outcome] = (counts[publication.outcome] ?? 0) + 1;
  return counts;
}
export function summarizeRuns(runs, selected) {
  return Object.fromEntries(
    selected.map((name) => [
      name,
      Object.fromEntries(
        metrics.map((metric) => {
          const values = (side) =>
            runs
              .filter(
                (run) =>
                  run.kind === "resources" &&
                  run.workload === name &&
                  run.side === side &&
                  run.status === "complete",
              )
              .map((run) => run.metrics?.[metric]);
          const baseline = summary(values("baseline"));
          const candidate = summary(values("candidate"));
          return [
            metric,
            {
              ...compareSummaries(baseline, candidate),
              note:
                baseline.mean === 0
                  ? "zero baseline: relative delta undefined"
                  : baseline.n < 2 || candidate.n < 2
                    ? "insufficient repeats to describe variation"
                    : "descriptive variation only; no confidence interval or significance claim",
            },
          ];
        }),
      ),
    ]),
  );
}
const latencyMetrics = [
  "preparationMs",
  "publicationToPreparedMs",
  "publicationToNativeDrawMs",
];
export function summarizeContent(runs, selected) {
  return Object.fromEntries(
    selected.map((name) => {
      const matching = runs.filter(
        (run) =>
          ["content", "physical"].includes(run.kind) && run.workload === name,
      );
      return [
        name,
        Object.fromEntries(
          latencyMetrics.map((metric) => {
            // Each repeat contributes one mean, so a faster side cannot gain weight
            // merely by completing more publications. Outcome counts retain censoring.
            const sideSummary = (side) =>
              summary(
                matching
                  .filter(
                    (run) =>
                      run.side === side && run.content?.status === "complete",
                  )
                  .map(
                    (run) =>
                      summary(run.content.publications.map((p) => p[metric]))
                        .mean,
                  ),
              );
            const baseline = sideSummary("baseline"),
              candidate = sideSummary("candidate");
            return [
              metric,
              {
                ...compareSummaries(baseline, candidate),
                note: "Completed observations only; compare outcome counts and publication identities before interpreting latency deltas",
              },
            ];
          }),
        ),
      ];
    }),
  );
}
export function resourceMetrics(samples) {
  if (samples.length < 2) return null;
  const first = samples[0],
    last = samples.at(-1);
  const elapsedSeconds = (last.elapsedMs - first.elapsedMs) / 1000;
  const cpuAvailable =
    samples.every((sample) => Number.isFinite(sample.cpuSeconds)) &&
    samples
      .slice(1)
      .every(
        (sample, index) => sample.cpuSeconds >= samples[index].cpuSeconds,
      ) &&
    elapsedSeconds > 0;
  const memoryAvailable = samples.every(
    (sample) => Number.isFinite(sample.rssBytes) && sample.rssBytes >= 0,
  );
  const cpuSeconds = cpuAvailable ? last.cpuSeconds - first.cpuSeconds : null;
  return {
    cpuSeconds,
    cpuPercent:
      cpuSeconds === null ? null : (cpuSeconds / elapsedSeconds) * 100,
    peakRssBytes: memoryAvailable
      ? Math.max(...samples.map((sample) => sample.rssBytes))
      : null,
    rssChangeBytes: memoryAvailable ? last.rssBytes - first.rssBytes : null,
    firstRssBytes: memoryAvailable ? first.rssBytes : null,
    lastRssBytes: memoryAvailable ? last.rssBytes : null,
    sampledDurationMs: last.elapsedMs - first.elapsedMs,
    intervalsMs: samples
      .slice(1)
      .map((sample, index) => sample.elapsedMs - samples[index].elapsedMs),
  };
}
export async function writeReport(output, report) {
  report.summaries = summarizeRuns(report.runs, report.selectedCoverage);
  report.contentSummaries = summarizeContent(
    report.runs,
    report.selectedCoverage,
  );
  const lines = [
    report.executionMode === "physical-display"
      ? "# Renderer physical presentation acceptance"
      : "# Renderer resource comparison",
    "",
    `Status: **${report.status}**. ${report.error ?? "Resource deltas are advisory."}`,
    "",
    "CPU seconds include user + system time of all Renderer threads; CPU % is seconds / sampled wall seconds × 100 (one logical CPU = 100%, may exceed 100%). RSS is bytes sampled for the Renderer process only, excluding Xvfb, publisher, sampler, descendants, and GPU memory. RSS peaks can miss short transients. RSS change is end minus start within each fresh process; compare per-repeat endpoints for repeated-work behavior, not a leak diagnosis.",
    "",
    report.executionMode === "physical-display"
      ? "Explicit physical execution. Presentation evidence is not optical verification. Clean resource passes and detailed physical tracing run in separate processes. Instrumented CPU/RSS samples never enter clean resource summaries; overhead estimates include drift and load uncertainty. Missing evidence cannot pass a requested contract."
      : "Clean resource runs have no diagnostic recorder. Separate content runs observe preparation and native drawing with bounded asynchronous recording; their CPU/RSS values are excluded from resource summaries. Headless software rendering does not establish physical display delivery or presentation acceptance. No statistical confidence is claimed. One repeat cannot describe repeat variation; unavailable values and zero-baseline relative deltas are null. Do not run substantial analysis concurrently with resource collection.",
    "",
    `Selected: ${report.selectedCoverage.join(", ")}. Omitted: ${report.omittedCoverage.join(", ")}.`,
    "",
    "| Workload / metric | Baseline mean | Candidate mean | Absolute delta | Relative % | Repeats B/C | SD B/C |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  const format = (value) =>
    value === null || value === undefined
      ? "unavailable"
      : Number(value.toFixed(3));
  for (const [workload, values] of Object.entries(report.summaries))
    for (const [metric, value] of Object.entries(values))
      lines.push(
        `| ${workload} / ${metric} | ${format(value.baseline.mean)} | ${format(value.candidate.mean)} | ${format(value.absoluteDelta)} | ${format(value.relativeDeltaPercent)} | ${value.baseline.n}/${value.candidate.n} | ${format(value.baseline.standardDeviation)}/${format(value.candidate.standardDeviation)} |`,
      );
  lines.push(
    "",
    "[Complete machine-readable report](report.json), including source manifests, run order, actual durations, sampling intervals, publications, per-repeat values, variation, and partial evidence.",
    "",
  );
  for (const [side, source] of Object.entries(report.sources))
    lines.push(
      `- ${side}: revision ${source.revision}, dirty ${source.dirty}; content SHA-256 ${source.contentDigest}; binary SHA-256 ${source.build?.binaryDigest ?? "unavailable"}. Preparation ${format(source.preparationMs)} ms; build ${format(source.build?.buildMs)} ms${source.reusedFrom ? " (reused; original duration)" : ""}. [Source/build manifest](${side}/source.json), [Build log](${side}/build.log), [executable](${side}/target/release/roonscape-renderer).`,
    );
  for (const run of report.runs)
    lines.push(
      `- ${run.side} ${run.workload} repeat ${run.repeat}: ${run.status}; warmup ${format(run.warmupMs)} ms, measurement ${format(run.measurementMs)} ms; [process log](${run.log}).`,
    );
  lines.push(
    "",
    "## Per-repeat resources",
    "",
    "| Order | Side / workload / repeat | CPU seconds | CPU % | Peak RSS bytes | First RSS bytes | Last RSS bytes | RSS change bytes | Sampled ms |",
    "| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
  );
  for (const run of report.runs.filter((run) => run.kind === "resources")) {
    const values = [
      "cpuSeconds",
      "cpuPercent",
      "peakRssBytes",
      "firstRssBytes",
      "lastRssBytes",
      "rssChangeBytes",
      "sampledDurationMs",
    ].map((key) => format(run.metrics?.[key]));
    lines.push(
      `| ${run.order} | ${run.side} / ${run.workload} / ${run.repeat} | ${values.join(" | ")} |`,
    );
  }
  lines.push(
    "",
    "## Content preparation and native readiness",
    "",
    "These timestamps observe preparation and native draw callbacks, not physical first-visible-content latency. Each repeat contributes one mean of available publication latencies. Missing observations remain null; delayed/superseded/incomplete outcomes are retained and must be considered before comparing means.",
    "",
    "| Workload / milliseconds | Baseline mean | Candidate mean | Absolute delta | Relative % | Repeats B/C | SD B/C |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  );
  for (const [workload, values] of Object.entries(report.contentSummaries))
    for (const [metric, value] of Object.entries(values))
      lines.push(
        `| ${workload} / ${metric} | ${format(value.baseline.mean)} | ${format(value.candidate.mean)} | ${format(value.absoluteDelta)} | ${format(value.relativeDeltaPercent)} | ${value.baseline.n}/${value.candidate.n} | ${format(value.baseline.standardDeviation)}/${format(value.candidate.standardDeviation)} |`,
      );
  lines.push(
    "",
    "Texture counts describe Rust handle lifetimes, not GPU retirement or memory allocations. Per-publication identities and latency milestones remain in the JSON report and linked observations.",
    "",
    "| Side / workload / repeat | Status / outcomes | Peak / final texture handles | Draws while incoming content not ready | Evidence |",
    "| --- | --- | ---: | ---: | --- |",
  );
  for (const run of report.runs.filter((run) =>
    ["content", "physical"].includes(run.kind),
  )) {
    const content = run.content;
    if (content?.publications) {
      content.outcomes = outcomeCounts(content.publications);
      content.latencies = Object.fromEntries(
        latencyMetrics.map((metric) => [
          metric,
          summary(content.publications.map((p) => p[metric])),
        ]),
      );
    }
    lines.push(
      `| ${run.side} / ${run.workload} / ${run.repeat} | ${content?.status ?? run.status}: ${content?.outcomes ? JSON.stringify(content.outcomes) : "unavailable"} ${content?.reason ?? ""} | ${format(content?.resources?.peakRetained)} / ${format(content?.resources?.retainedAfterShutdown)} | ${format(content?.continuingPresentation?.drawsWhileIncomingNotReady)} | ${run.evidence ? `[Observations](${run.evidence})` : "unavailable"} |`,
    );
  }
  appendPhysicalReport(lines, report, format);
  lines.push(
    "",
    "## Conditions and descriptive variation",
    "",
    `Profile: ${report.profile.name}; ${report.profile.repeats} repeats, ${report.profile.warmupMs} ms warmup, ${report.profile.measurementMs} ms measurement, ${report.profile.intervalMs} ms requested sampling interval. Preparation/build ${format(report.timing.preparationAndBuildMs)} ms; comparison ${format(report.timing.comparisonMs)} ms.`,
    "",
    `Environment: ${report.conditions?.platform ?? "unavailable"} / ${report.conditions?.architecture ?? "unavailable"}; viewport ${report.conditions?.viewport ?? "unavailable"}; ${report.conditions?.graphics ?? "backend unavailable"}. Qt metadata: ${report.conditions?.qt ?? "unavailable"}${report.conditions?.qtVersionSource ? ` (${report.conditions.qtVersionSource})` : ""}.`,
    "",
    "SD is descriptive sample standard deviation across repeat means; it is unavailable with fewer than two observations. Relative changes are unavailable for zero baselines. No statistical confidence is claimed. Full per-repeat variation, workload identities and schedules, toolchain metadata, sampling intervals, capabilities, event timestamps and observation windows remain in the [JSON report](report.json).",
    "",
  );
  await writeFile(
    path.join(output, "report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await writeFile(path.join(output, "report.md"), lines.join("\n"));
}

function appendPhysicalReport(lines, report, format) {
  if (report.executionMode !== "physical-display") return;
  const physical = report.runs.filter((run) => run.kind === "physical");
  report.physicalSummaries = Object.fromEntries(
    report.selectedCoverage.map((workload) => {
      const values = (side) =>
        summary(
          physical
            .filter(
              (run) =>
                run.workload === workload &&
                run.side === side &&
                run.physical?.status === "complete",
            )
            .map(
              (run) =>
                summary(
                  run.physical.publications.map(
                    (p) => p.publicationToPresentedMs,
                  ),
                ).mean,
            ),
        );
      const baseline = values("baseline"),
        candidate = values("candidate");
      return [workload, compareSummaries(baseline, candidate)];
    }),
  );
  report.instrumentationOverhead = [];
  lines.push(
    "",
    "## Physical presentation",
    "",
    "Scanout flip completions are joined by window/Present serial to the submitting thread's frame and scene. They are presentation evidence, not optical verification. Copy, unsupported, missing, or ambiguous observations cannot establish first visibility. Latencies use host monotonic microseconds; delayed completions remain attached to the original measurement window. Repeat means include only available observations: compare censoring and outcomes before interpreting deltas.",
    "",
    "| Workload / publication to presentation ms | Baseline mean | Candidate mean | Absolute delta | Relative % | Repeats B/C | SD B/C |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  );
  for (const [workload, value] of Object.entries(report.physicalSummaries))
    lines.push(
      `| ${workload} | ${format(value.baseline.mean)} | ${format(value.candidate.mean)} | ${format(value.absoluteDelta)} | ${format(value.relativeDeltaPercent)} | ${value.baseline.n}/${value.candidate.n} | ${format(value.baseline.standardDeviation)}/${format(value.candidate.standardDeviation)} |`,
    );
  lines.push(
    "",
    "| Side / workload / repeat | Evidence / contract | Missed refreshes | Worst stall / delivery gap µs | Refresh stride / render-start anomalies | Mean content latency ms / outcomes | Measurement window µs | Raw evidence |",
    "| --- | --- | ---: | ---: | ---: | --- | --- | --- |",
  );
  const details = [];
  for (const run of physical) {
    const evidence = run.physical;
    const label = `${run.side} / ${run.workload} / ${run.repeat}`;
    const publications = evidence?.publications;
    const reasons = (evidence?.reasons ?? [run.error]).filter(Boolean);
    lines.push(
      `| ${label} | ${evidence?.status ?? run.status} / ${evidence?.contract.status ?? "unavailable"} | ${format(evidence?.cadence?.missedPresentations)} | ${format(evidence?.worstStallMicros)} / ${format(evidence?.worstDeliveryGapMicros)} | ${format(evidence?.cadence?.refreshStride)} / ${format(evidence?.cadence?.renderStartGaps.length)} | ${format(publications ? summary(publications.map((p) => p.publicationToPresentedMs)).mean : null)} / ${publications ? JSON.stringify(outcomeCounts(publications)) : "unavailable"} | ${evidence ? `${evidence.window.startMicros}–${evidence.window.endMicros}` : "unavailable"} | [Present](${run.presentationEvidence}), [draws](${run.animationEvidence}), [content](${run.evidence}) |`,
    );
    if (reasons.length) details.push(`- ${label}: ${reasons.join("; ")}`);
    const gaps = evidence?.cadence?.deliveryGaps ?? [];
    if (gaps.length)
      details.push(
        `- ${label}: ${gaps.length} delivery gaps; up to three worst with event timestamps: ${JSON.stringify([...gaps].sort((a, b) => b.gapMicros - a.gapMicros).slice(0, 3))}.`,
      );
  }
  lines.push(
    "",
    ...details,
    "",
    "Full frame associations, publication milestones and gap timestamps are retained in the [JSON report](report.json) and the raw evidence linked above.",
  );
  lines.push(
    "",
    "## Instrumentation overhead",
    "",
    "Separate clean and instrumented passes use the same build and schedule. CPU/RSS deltas are advisory and also include run-order/environmental drift. Enqueue timing measures only bounded request recording, not total instrumentation overhead. A single repeat cannot characterize variation; use repeated runs before interpreting small differences.",
    "",
    "| Side / workload | Clean CPU seconds mean | Instrumented CPU seconds mean | Delta | Clean peak RSS mean | Instrumented peak RSS mean |",
    "| --- | ---: | ---: | ---: | ---: | ---: |",
  );
  for (const side of ["baseline", "candidate"])
    for (const workload of report.selectedCoverage) {
      const matching = report.runs.filter(
        (run) =>
          run.side === side &&
          run.workload === workload &&
          run.status === "complete",
      );
      const metric = (kind, name) =>
        summary(
          matching
            .filter((run) => run.kind === kind)
            .map((run) => run.metrics?.[name]),
        );
      const clean = metric("resources", "cpuSeconds"),
        instrumented = metric("physical", "cpuSeconds");
      const delta = compareSummaries(clean, instrumented).absoluteDelta;
      const memory = {
        clean: metric("resources", "peakRssBytes"),
        instrumented: metric("physical", "peakRssBytes"),
      };
      report.instrumentationOverhead.push({
        side,
        workload,
        clean,
        instrumented,
        cpuSecondsDelta: delta,
        memory,
      });
      lines.push(
        `| ${side} / ${workload} | ${format(clean.mean)} | ${format(instrumented.mean)} | ${format(delta)} | ${format(memory.clean.mean)} | ${format(memory.instrumented.mean)} |`,
      );
    }
}
