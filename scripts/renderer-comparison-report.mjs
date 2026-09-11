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
                  run.workload === name &&
                  run.side === side &&
                  run.status === "complete",
              )
              .map((run) => run.metrics?.[metric]);
          const baseline = summary(values("baseline"));
          const candidate = summary(values("candidate"));
          const absoluteDelta =
            baseline.mean === null || candidate.mean === null
              ? null
              : candidate.mean - baseline.mean;
          return [
            metric,
            {
              baseline,
              candidate,
              absoluteDelta,
              relativeDeltaPercent:
                absoluteDelta === null || baseline.mean === 0
                  ? null
                  : (absoluteDelta / Math.abs(baseline.mean)) * 100,
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
  const lines = [
    "# Renderer resource comparison",
    "",
    `Status: **${report.status}**. ${report.error ?? "Resource deltas are advisory."}`,
    "",
    "CPU seconds include user + system time of all Renderer threads; CPU % is seconds / sampled wall seconds × 100 (one logical CPU = 100%, may exceed 100%). RSS is bytes sampled for the Renderer process only, excluding Xvfb, publisher, sampler, descendants, and GPU memory. RSS peaks can miss short transients. RSS change is end minus start within each fresh process; compare per-repeat endpoints for repeated-work behavior, not a leak diagnosis.",
    "",
    "No tracing or profiling. Headless software rendering does not establish physical display cadence, content readiness, correctness, or presentation acceptance. No statistical confidence is claimed. One repeat cannot describe repeat variation; unavailable values and zero-baseline relative deltas are null. Do not run substantial analysis concurrently with resource collection.",
    "",
    `Selected: ${report.selectedCoverage.join(", ")}. Omitted: ${report.omittedCoverage.join(", ")}.`,
    "",
    "| Workload / metric | Baseline mean | Candidate mean | Absolute delta | Relative % | Repeats B/C |",
    "| --- | ---: | ---: | ---: | ---: | ---: |",
  ];
  const format = (value) =>
    value === null || value === undefined
      ? "unavailable"
      : Number(value.toFixed(3));
  for (const [workload, values] of Object.entries(report.summaries))
    for (const [metric, value] of Object.entries(values))
      lines.push(
        `| ${workload} / ${metric} | ${format(value.baseline.mean)} | ${format(value.candidate.mean)} | ${format(value.absoluteDelta)} | ${format(value.relativeDeltaPercent)} | ${value.baseline.n}/${value.candidate.n} |`,
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
  for (const run of report.runs) {
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
  // Keep the human report self-contained without duplicating raw sample/publication arrays.
  lines.push(
    "",
    "## Conditions and descriptive variation",
    "",
    "```json",
    JSON.stringify(
      {
        profile: report.profile,
        conditions: report.conditions,
        timing: report.timing,
        workloads: report.workloads,
        summaries: report.summaries,
      },
      null,
      2,
    ),
    "```",
    "",
  );
  await writeFile(
    path.join(output, "report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  await writeFile(path.join(output, "report.md"), lines.join("\n"));
}
