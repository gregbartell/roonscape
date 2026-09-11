import {
  access,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { constants } from "node:fs";
import os from "node:os";
import { fileURLToPath } from "node:url";
import {
  processCancellation,
  runMonitoredProcess,
} from "./process-harness.mjs";
import {
  buildSource,
  copyRuntimeResources,
  digest,
  prepareSource,
} from "./renderer-comparison-sources.mjs";
import {
  coverage,
  profiles,
  workloads,
  prepareWorkloadContent,
  workloadRoot,
} from "./renderer-comparison-workloads.mjs";
import { measureRenderer } from "./renderer-comparison-run.mjs";
import { writeReport } from "./renderer-comparison-report.mjs";
import { failureOutcome } from "./renderer-comparison-outcome.mjs";

import { preparePhysical } from "./physical-presentation-run.mjs";

export async function compare(options) {
  const cancellation = processCancellation();
  const { signal } = cancellation;
  const started = performance.now();
  let output, scratch;
  const report = {
    version: 1,
    executionMode: options.physical ? "physical-display" : "headless-resources",
    status: "preparing",
    profile: {
      name: options.profile,
      ...profiles[options.profile],
      ...options.overrides,
    },
    selectedCoverage: [],
    omittedCoverage: [],
    sources: {},
    runs: [],
    timing: {},
  };
  let exitCode;
  try {
    const selected = await workloads(options.workloads);
    report.selectedCoverage = selected.map((workload) => workload.name);
    report.omittedCoverage = [
      ...coverage.filter((name) => !report.selectedCoverage.includes(name)),
      ...(options.physical ? [] : ["physical presentation cadence"]),
      "GPU resources",
      "correctness/presentation contracts",
    ];
    if (report.profile.measurementMs < 8000)
      report.omittedCoverage.push("full workload cycles (short measurement)");
    if (report.profile.repeats < 2)
      report.omittedCoverage.push("repeat variation (single repeat)");
    await mkdir("/var/tmp/codex/roonscape", { recursive: true });
    if (options.output) {
      await mkdir(options.output, { recursive: false });
      output = options.output;
    } else output = await mkdtemp("/var/tmp/codex/roonscape/comparison.");
    console.log(`Renderer comparison evidence: ${output}`);
    scratch = await mkdtemp("/var/tmp/codex/roonscape/task.");
    await writeReport(output, report);
    if (options.physical) {
      report.physicalSelection = options.physical;
      report.physicalTools = await preparePhysical(options, output, signal);
      options.physicalTools = report.physicalTools;
    }
    if (process.platform !== "linux")
      throw new Error("resource comparisons require Linux /proc");
    report.conditions = {
      platform: process.platform,
      architecture: process.arch,
      kernel: os.release(),
      cpuModel: os.cpus()[0]?.model,
      logicalCpus: os.cpus().length,
      memoryBytes: os.totalmem(),
      viewport: `${options.width}x${options.height}`,
      graphics: options.physical
        ? "explicit physical X11 display, Qt xcb GLX, unit scale"
        : "private Xvfb, Qt xcb, LIBGL_ALWAYS_SOFTWARE=1, unit scale",
      locale: "C.UTF-8",
      maximumPublicationLatenessMs: 50,
      inactivity: { gracePeriodSeconds: 1, repositionCadenceSeconds: 2 },
      instrumentation: {
        name: "external /proc process sampler",
        requestedIntervalMs: report.profile.intervalMs,
        samplerDigest: digest(
          await readFile(
            fileURLToPath(
              new URL("renderer-resource-sampler.py", import.meta.url),
            ),
          ),
        ),
      },
      loadBefore: os.loadavg(),
      node: process.version,
    };
    const driverFiles = [
      "compare-renderer.mjs",
      "renderer-comparison.mjs",
      "renderer-comparison-sources.mjs",
      "renderer-comparison-workloads.mjs",
      "renderer-comparison-run.mjs",
      "renderer-comparison-report.mjs",
      "renderer-comparison-outcome.mjs",
      "renderer-comparison-content.mjs",
      "accept-presentation.mjs",
      "physical-presentation.mjs",
      "physical-presentation-run.mjs",
      "animation-evidence.mjs",
      "native-session.mjs",
      "process-harness.mjs",
    ];
    report.conditions.instrumentation.driverDigest = digest(
      JSON.stringify(
        await Promise.all(
          driverFiles.map(async (file) => ({
            file,
            sha256: digest(
              await readFile(fileURLToPath(new URL(file, import.meta.url))),
            ),
          })),
        ),
      ),
    );
    report.conditions.contentInstrumentation = {
      version: 1,
      clock: "CLOCK_MONOTONIC",
      queueRecords: 1024,
      maximumRecords: 65536,
      maximumRecordBytes: 65536,
      normalDrainMs: report.profile.drainMs ?? 1000,
      scope:
        "separate diagnostic processes; preparation and native draw callbacks, not physical delivery",
    };
    report.conditions.instrumentation.differencesBetweenSides =
      "none: both sides use this driver and sampler with identical settings";
    report.conditions.executables = {};
    for (const name of [
      "git",
      "tar",
      "cargo",
      "rustc",
      "qmake6",
      "fc-list",
      "python3",
      "Xvfb",
      "xwininfo",
      "dbus-daemon",
    ]) {
      const executable = await resolveExecutable(name);
      report.conditions.executables[name] = {
        path: executable,
        sha256: digest(await readFile(executable)),
      };
    }
    for (const [key, command, args] of [
      ["cargo", "cargo", ["--version"]],
      ["rustc", "rustc", ["--version"]],
      ["qt", "qmake6", ["-query", "QT_VERSION"]],
    ])
      report.conditions[key] = (
        await runMonitoredProcess(command, args, { signal })
      ).trim();
    report.conditions.affinity =
      (await readFile("/proc/self/status", "utf8")).match(
        /^Cpus_allowed_list:.*$/m,
      )?.[0] ?? "unavailable";
    report.conditions.fontInventoryDigest = digest(
      await runMonitoredProcess(
        "fc-list",
        ["--format", "%{family}: %{file}\n"],
        { signal },
      ),
    );
    for (const side of ["baseline", "candidate"]) {
      const directory = path.join(scratch, side);
      const evidence = path.join(output, side);
      await mkdir(evidence);
      const schemas = {};
      for (const schema of ["presentation-snapshot", "display-configuration"])
        schemas[schema] = digest(
          await readFile(
            path.join(workloadRoot, `src/shared/schema/${schema}.schema.json`),
          ),
        );
      if (options[side].startsWith("build:")) {
        const previous = path.resolve(options[side].slice(6));
        const source = JSON.parse(
          await readFile(path.join(previous, "source.json"), "utf8"),
        );
        const binary = await readFile(
          path.join(previous, "target/release/roonscape-renderer"),
        );
        if (
          source.build?.profile !== "release" ||
          source.build.optLevel !== 3 ||
          digest(binary) !== source.build.binaryDigest ||
          JSON.stringify(source.schemas) !== JSON.stringify(schemas)
        )
          throw Object.assign(
            new Error("incompatible or modified prepared build"),
            { invalidEvidence: true },
          );
        for (const file of source.manifest.filter(
          (file) =>
            file.path.startsWith("src/renderer/assets/fonts/") ||
            file.path.startsWith("src/desktop/icons/"),
        )) {
          if (
            digest(await readFile(path.join(previous, file.path))) !==
            file.sha256
          )
            throw Object.assign(
              new Error("modified prepared build resources"),
              { invalidEvidence: true },
            );
        }
        report.sources[side] = { ...source, reusedFrom: previous };
        await mkdir(path.join(evidence, "target/release"), { recursive: true });
        await copyRuntimeResources(previous, evidence);
        await copyFile(
          path.join(previous, "target/release/roonscape-renderer"),
          path.join(evidence, "target/release/roonscape-renderer"),
        );
        await copyFile(
          path.join(previous, "build.log"),
          path.join(evidence, "build.log"),
        );
      } else {
        report.sources[side] = await prepareSource(
          options[side],
          directory,
          signal,
        );
        for (const [schema, expected] of Object.entries(schemas)) {
          if (
            digest(
              await readFile(
                path.join(directory, `src/shared/schema/${schema}.schema.json`),
              ),
            ) !== expected
          )
            throw Object.assign(
              new Error(`incompatible ${side} ${schema} schema`),
              { invalidEvidence: true },
            );
        }
        report.sources[side].schemas = schemas;
        console.log(
          `Building ${side} ${report.sources[side].revision}${report.sources[side].dirty ? " (dirty)" : ""} with release optimization`,
        );
        report.sources[side].build = {
          ...(await buildSource(directory, evidence, signal)),
          compiler: report.conditions.rustc,
          cargo: report.conditions.cargo,
        };
      }
      await writeFile(
        path.join(evidence, "source.json"),
        JSON.stringify(report.sources[side], null, 2),
      );
      await writeReport(output, report);
    }
    await prepareWorkloadContent(selected, report.profile, output);
    report.workloads = selected;
    report.timing.preparationAndBuildMs = performance.now() - started;
    const measurementStarted = performance.now();
    report.status = "measuring";
    for (const kind of options.physical
      ? ["resources", "physical"]
      : ["resources", "content"]) {
      const selectedRuns =
        kind === "content"
          ? selected.filter((workload) => workload.diagnostic)
          : selected;
      for (let repeat = 0; repeat < report.profile.repeats; repeat++) {
        const order =
          repeat % 2 ? ["candidate", "baseline"] : ["baseline", "candidate"];
        for (const workload of selectedRuns)
          for (const side of order) {
            const run = {
              side,
              kind,
              workload: workload.name,
              repeat: repeat + 1,
              order: report.runs.length + 1,
              status: "running",
              phase: "startup",
              publications: [],
              samples: [],
            };
            report.runs.push(run);
            if (
              ["content", "physical"].includes(kind) &&
              !report.sources[side].manifest.some(
                (file) => file.path === "src/renderer/src/content_evidence.rs",
              )
            ) {
              run.status = "unavailable";
              run.content = {
                status: "unavailable",
                reason:
                  "This Renderer build does not provide content observations",
              };
              if (options.physical)
                throw Object.assign(
                  new Error(
                    "selected build does not provide required native content observations",
                  ),
                  { unsupported: true },
                );
              continue;
            }
            console.log(
              `${kind === "resources" ? "Repeat" : kind === "physical" ? "Physical repeat" : "Diagnostic repeat"} ${run.repeat}: ${side} / ${workload.name}`,
            );
            try {
              await measureRenderer(
                path.join(output, side, "target/release/roonscape-renderer"),
                workload,
                { ...options, ...report.profile },
                run,
                output,
                signal,
              );
            } finally {
              await writeReport(output, report);
            }
          }
      }
    }
    report.timing.comparisonMs = performance.now() - measurementStarted;
    report.conditions.loadAfter = os.loadavg();
    report.status = "complete";
    exitCode = 0;
  } catch (error) {
    const outcome = failureOutcome(error, signal);
    report.status = outcome.status;
    report.error = error.message;
    if (options.physical && !report.physicalTools)
      report.physicalCapabilities = { supported: false, reason: error.message };
    console.error(error.message);
    exitCode = outcome.exitCode;
  } finally {
    try {
      if (scratch) await rm(scratch, { recursive: true, force: true });
    } catch (error) {
      report.status = "execution-failed";
      report.cleanupError = error.message;
      exitCode = 1;
    }
    report.timing.totalMs = performance.now() - started;
    if (output) await writeReport(output, report);
    cancellation.dispose();
  }
  return exitCode;
}

async function resolveExecutable(name) {
  for (const directory of (process.env.PATH ?? "").split(path.delimiter)) {
    const executable = path.resolve(directory, name);
    try {
      await access(executable, constants.X_OK);
      return executable;
    } catch {
      /* Continue through PATH; execution will still validate the executable. */
    }
  }
  throw new Error(`missing prerequisite executable: ${name}`);
}
