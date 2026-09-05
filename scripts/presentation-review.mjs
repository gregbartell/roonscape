import { appendFileSync, renameSync, writeFileSync } from "node:fs";
import { cp, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  processCancellation,
  runMonitoredProcess,
} from "./process-harness.mjs";
import {
  preflightPresentationCapturePlan,
  groupCompatibleCaptures,
} from "./presentation-capture-planning.mjs";
import { executePresentationCapturePlan } from "./presentation-capture-execution.mjs";
import { createControlledRendererSessionAdapter } from "./presentation-capture-renderer.mjs";
import { publishPresentationCapture } from "./presentation-capture-publication.mjs";

import { buildPresentationCapturePlan } from "./presentation-captures.mjs";

// A deliberately small CI sample: Now Playing, Full-field, and light palette,
// including both the minimum and normal windowed representative viewports.
const ciSelection = [
  ["playing", "1280x720"],
  ["idle", "1280x720"],
  ["long-metadata", "1600x900"],
  ["light-artwork", "1600x900"],
];

function selectCaptures(scope, scenarios = []) {
  const plan = buildPresentationCapturePlan();
  if (scope === "complete") return plan;
  const matrix = plan.filter(({ variant }) => variant === "matrix");
  if (scope === "ci-fallback") {
    return ciSelection.map(([scenario, viewport]) => {
      const capture = matrix.find(
        (candidate) =>
          candidate.scenario === scenario && candidate.viewport === viewport,
      );
      if (!capture)
        throw new Error(
          `CI capture is no longer maintained: ${scenario} ${viewport}`,
        );
      return { ...capture, typography: "fallback" };
    });
  }
  if (scope !== "focused")
    throw new Error("Scope must be focused, complete, or ci-fallback");
  if (!scenarios.length) throw new Error("Focused reviews require --scenario");
  for (const scenario of scenarios) {
    if (!matrix.some((capture) => capture.scenario === scenario))
      throw new Error(`Unknown maintained Fixture Scenario: ${scenario}`);
  }
  return matrix.filter((capture) => scenarios.includes(capture.scenario));
}

function listScopes() {
  const plan = buildPresentationCapturePlan();
  const ciCaptures = selectCaptures("ci-fallback");
  console.log(
    JSON.stringify(
      {
        focused: {
          scenarios: [
            ...new Set(
              plan
                .filter(({ variant }) => variant === "matrix")
                .map(({ scenario }) => scenario),
            ),
          ],
          viewports: [...new Set(plan.map(({ viewport }) => viewport))],
        },
        complete: { requested: plan.length },
        "ci-fallback": {
          requested: ciCaptures.length,
          captures: ciCaptures.map(({ scenario, viewport }) => ({
            scenario,
            viewport,
          })),
          typography: "packaged fallback only",
        },
      },
      null,
      2,
    ),
  );
}

const root = fileURLToPath(new URL("..", import.meta.url));

function parseOptions(args) {
  const options = { scenarios: [] };
  for (let index = 0; index < args.length; index++) {
    const name = args[index];
    if (!["--output", "--scope", "--scenario"].includes(name))
      throw new Error(`Unknown review option: ${name}`);
    const value = args[++index];
    if (!value?.trim() || value.startsWith("--"))
      throw new Error(`${name} requires a value`);
    if (name === "--scenario") options.scenarios.push(value);
    else {
      if (options[name]) throw new Error(`Duplicate option: ${name}`);
      options[name] = value;
    }
  }
  return options;
}

async function captureReview(options) {
  if (!options["--output"]) throw new Error("Capture reviews require --output");
  const scope = options["--scope"];
  if (scope !== "focused" && options.scenarios.length)
    throw new Error("Only focused reviews accept --scenario");
  const selected = selectCaptures(scope, options.scenarios);
  const output = path.resolve(options["--output"]);
  await mkdir(output, { recursive: true });
  const directory = await mkdtemp(path.join(output, "presentation."));
  console.log(`Presentation review: ${directory}`);
  const cancellation = processCancellation();
  const report = {
    scope,
    startedAt: new Date().toISOString(),
    outcome: "incomplete",
    typography:
      scope === "ci-fallback" ? "packaged fallback only" : "host automatic",
    requested: selected.map(
      ({ fileName, scenario, viewport, typography, variant }) => ({
        fileName,
        scenario,
        viewport,
        typography,
        variant,
      }),
    ),
    completed: [],
  };
  let runtime;
  const log = (message) =>
    appendFileSync(path.join(directory, "capture.log"), `${message}\n`);
  try {
    saveCaptureReport(directory, report);
    log(`Requested ${selected.length} captures; scope ${scope}`);
    runtime = await mkdtemp(path.join(os.tmpdir(), "rs-r."));
    const environment = { ...process.env };
    if (scope === "ci-fallback") {
      // Older Fontconfig writes .uuid beside scanned fonts even with a private
      // cache directory. Keep those writes out of the source worktree.
      const fonts = path.join(runtime, "fonts");
      await cp(path.join(root, "src/renderer/assets/fonts"), fonts, {
        recursive: true,
      });
      const config = path.join(runtime, "fonts.conf");
      await writeFile(
        config,
        `<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><dir>${escapeHtml(fonts)}</dir><cachedir>${escapeHtml(runtime)}</cachedir></fontconfig>`,
      );
      environment.FONTCONFIG_FILE = config;
      environment.FONTCONFIG_PATH = runtime;
    }
    if (scope === "complete") {
      // Match the renderer's private home/XDG view rather than counting fonts
      // that are visible only through the maintainer's personal configuration.
      const fontEnvironment = {
        ...environment,
        HOME: runtime,
        XDG_CONFIG_HOME: runtime,
        XDG_DATA_HOME: runtime,
        XDG_CACHE_HOME: runtime,
      };
      const fontInventory = JSON.parse(
        await runMonitoredProcess(
          "python3",
          [path.join(root, "scripts/inspect-host-fonts.py")],
          { environment: fontEnvironment, signal: cancellation.signal },
        ),
      );
      const families = new Set(
        fontInventory.families.map((family) => family.toLowerCase()),
      );
      const missing = ["Sitka Display", "Palatino Linotype", "Segoe UI"].filter(
        (family) => !families.has(family.toLowerCase()),
      );
      if (!fontInventory.hasMoonGlyph) missing.push("glyph fallback for 月");
      if (missing.length)
        throw new Error(
          `Complete profile requires host fonts: ${missing.join(", ")}; no fallback substitution is allowed`,
        );
    }
    if (scope !== "ci-fallback")
      report.typography =
        scope === "complete"
          ? "complete host typography plus packaged fallback representative"
          : "host automatic";
    saveCaptureReport(directory, report);
    const captures = await preflightPresentationCapturePlan(
      selected,
      { output: directory },
      { workingDirectory: root, environment },
    );
    await executePresentationCapturePlan(
      {
        captures,
        sessions: groupCompatibleCaptures(captures),
        incompleteSetName: "Presentation review",
      },
      {
        sessionAdapter: createControlledRendererSessionAdapter({
          environment,
          publishCapture: publishPresentationCapture,
          signal: cancellation.signal,
        }),
        onCaptureStarted: ({ scenario, viewport }) =>
          log(`Capturing ${scenario} at ${viewport}`),
        onCapturePublished: (capturePath) => {
          report.completed.push(path.basename(capturePath));
          log(`Published ${path.basename(capturePath)}`);
          saveCaptureReport(directory, report);
        },
      },
    );
    report.outcome = "complete";
  } catch (error) {
    report.outcome = cancellation.signal.aborted ? "cancelled" : "failed";
    report.error = error.message;
    console.error(error.message);
    log(error.message);
    process.exitCode = cancellation.signal.aborted ? 130 : 1;
  } finally {
    try {
      if (runtime) await rm(runtime, { recursive: true, force: true });
    } catch (error) {
      report.cleanupError = error.message;
      console.error(error.message);
      report.outcome = "incomplete";
      process.exitCode = 1;
    }
    if (cancellation.signal.aborted) {
      report.outcome = "cancelled";
      process.exitCode = 130;
    }
    report.finishedAt = new Date().toISOString();
    saveCaptureReport(directory, report);
    cancellation.dispose();
  }
  console.log(
    `Capture completion: ${report.outcome} (${report.completed.length}/${report.requested.length}); typography: ${report.typography}`,
  );
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function saveCaptureReport(directory, report) {
  const file = path.join(directory, "captures.json");
  writeFileSync(`${file}.tmp`, JSON.stringify(report, null, 2) + "\n", {
    mode: 0o600,
  });
  renameSync(`${file}.tmp`, file);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--list") return listScopes();
  const options = parseOptions(args);
  await captureReview(options);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
