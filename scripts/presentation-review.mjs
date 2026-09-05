import { appendFileSync, renameSync, writeFileSync } from "node:fs";
import {
  appendFile,
  cp,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
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
    if (
      ![
        "--review",
        "--scope",
        "--scenario",
        "--rationale",
        "--record",
        "--verdict-file",
      ].includes(name)
    )
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
  if (!options["--review"] || !options["--rationale"])
    throw new Error("Capture reviews require --review and --rationale");
  const scope = options["--scope"];
  if (scope !== "focused" && options.scenarios.length)
    throw new Error("Only focused reviews accept --scenario");
  const selected = selectCaptures(scope, options.scenarios);
  const review = path.resolve(options["--review"]);
  const verification = JSON.parse(
    await readFile(path.join(review, "verification.json"), "utf8"),
  );
  if (!["complete", "failed", "cancelled"].includes(verification.outcome))
    throw new Error(
      "Wait for verification to finish before extending its evidence",
    );
  const directory = await mkdtemp(path.join(review, "presentation."));
  console.log(`Presentation review: ${directory}`);
  await appendFile(
    path.join(review, "README.md"),
    `\n[Presentation review: ${path.basename(directory)}](${path.basename(directory)}/index.html)\n`,
  );
  const cancellation = processCancellation();
  const report = {
    scope,
    rationale: options["--rationale"],
    startedAt: new Date().toISOString(),
    outcome: "incomplete",
    verificationSource: verification.source,
    automatedOutcome: verification.automatedOutcome ?? verification.outcome,
    source: { root },
    typography:
      scope === "ci-fallback"
        ? "packaged fallback only"
        : "host automatic; inventory pending",
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
    visualAcceptance: "unreviewed",
  };
  let runtime;
  const log = (message) =>
    appendFileSync(path.join(directory, "capture.log"), `${message}\n`);
  try {
    saveCaptureReport(directory, report);
    log(`Requested ${selected.length} captures; scope ${scope}`);
    report.source.revision = (
      await runMonitoredProcess("git", ["rev-parse", "HEAD"], {
        cwd: root,
        signal: cancellation.signal,
      })
    ).trim();
    report.source.workingTree = await runMonitoredProcess(
      "git",
      ["status", "--porcelain=v1", "--untracked-files=all"],
      { cwd: root, signal: cancellation.signal },
    );
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
    // Match the renderer's private home/XDG view rather than counting fonts
    // that are visible only through the maintainer's personal configuration.
    const fontEnvironment = {
      ...environment,
      HOME: runtime,
      XDG_CONFIG_HOME: runtime,
      XDG_DATA_HOME: runtime,
      XDG_CACHE_HOME: runtime,
    };
    report.fontInventory = JSON.parse(
      await runMonitoredProcess(
        "python3",
        [path.join(root, "scripts/inspect-host-fonts.py")],
        { environment: fontEnvironment, signal: cancellation.signal },
      ),
    );
    if (scope === "complete") {
      const families = new Set(
        report.fontInventory.families.map((family) => family.toLowerCase()),
      );
      const missing = ["Sitka Display", "Palatino Linotype", "Segoe UI"].filter(
        (family) => !families.has(family.toLowerCase()),
      );
      if (!report.fontInventory.hasMoonGlyph)
        missing.push("glyph fallback for 月");
      if (missing.length)
        throw new Error(
          `Complete profile requires host fonts: ${missing.join(", ")}; no fallback substitution is allowed`,
        );
    }
    if (scope !== "ci-fallback")
      report.typography =
        scope === "complete"
          ? "complete host typography plus packaged fallback representative"
          : "host automatic; see font inventory (not complete typography coverage)";
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
    log(error.message);
    process.exitCode = cancellation.signal.aborted ? 130 : 1;
  } finally {
    try {
      if (runtime) await rm(runtime, { recursive: true, force: true });
    } catch (error) {
      report.cleanupError = error.message;
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
    `Capture completion: ${report.outcome} (${report.completed.length}/${report.requested.length}); typography: ${report.typography}; visual acceptance: unreviewed`,
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
  const completed = new Set(report.completed);
  const index = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Presentation review</title><style>body{font:18px system-ui;max-width:1100px;margin:40px auto;padding:20px}img{max-width:100%;height:auto}li{margin:24px 0}pre{white-space:pre-wrap}</style><h1>Presentation review</h1><p>Scope: ${escapeHtml(report.scope)}. ${escapeHtml(report.typography)}</p><p>Capture completion: ${escapeHtml(report.outcome)}; ${report.completed.length}/${report.requested.length} completed. Visual acceptance: ${escapeHtml(report.visualAcceptance)}.</p><p>Selection rationale: ${escapeHtml(report.rationale)}</p><p>Automated outcome: ${escapeHtml(report.automatedOutcome)}. Automation does not establish visual acceptance. CI fallback evidence does not establish complete typography coverage.</p><p><a href="captures.json">Coverage and source identity</a> · <a href="capture.log">Capture diagnostics</a> · <a href="../README.md">Verification evidence</a></p><p>${report.verdict ? `<a href="verdict.json">Visual verdict and unresolved judgments</a>: ${escapeHtml(report.verdict.reasons)}` : "No visual verdict recorded."}</p><pre>${escapeHtml(report.error ?? "")}</pre><ul>${report.requested.map(({ fileName, scenario, viewport, typography }) => `<li>${escapeHtml(scenario)} — ${escapeHtml(viewport)} — ${escapeHtml(typography)}: ${completed.has(fileName) ? `<a href="${fileName}">${fileName}</a><br><img loading="lazy" src="${fileName}" alt="${escapeHtml(scenario)} at ${escapeHtml(viewport)}">` : "not completed"}</li>`).join("")}</ul></html>`;
  for (const [name, contents] of [
    ["captures.json", JSON.stringify(report, null, 2) + "\n"],
    ["index.html", index],
  ]) {
    writeFileSync(path.join(directory, `${name}.tmp`), contents, {
      mode: 0o600,
    });
    renameSync(path.join(directory, `${name}.tmp`), path.join(directory, name));
  }
}

async function recordVerdict(options) {
  if (
    !options["--record"] ||
    !options["--verdict-file"] ||
    options["--review"] ||
    options["--scope"] ||
    options["--rationale"] ||
    options.scenarios.length
  )
    throw new Error("Recording requires only --record and --verdict-file");
  const directory = path.resolve(options["--record"]);
  const report = JSON.parse(
    await readFile(path.join(directory, "captures.json"), "utf8"),
  );
  if (!report.finishedAt)
    throw new Error(
      "Wait for capture generation to finish before recording a verdict",
    );
  const input = JSON.parse(await readFile(options["--verdict-file"], "utf8"));
  if (
    !["accepted", "needs-work", "unreviewed"].includes(input.verdict) ||
    typeof input.reasons !== "string" ||
    !input.reasons.trim() ||
    !Array.isArray(input.inspected) ||
    !Array.isArray(input.unresolved) ||
    input.unresolved.some((item) => typeof item !== "string" || !item.trim())
  )
    throw new Error(
      "Verdict requires verdict, reasons, inspected filenames, and unresolved judgments",
    );
  const inspected = new Set(input.inspected);
  if (input.inspected.some((name) => !report.completed.includes(name)))
    throw new Error(
      "Inspected images must belong to the completed capture set",
    );
  if (input.verdict === "accepted") {
    if (
      report.outcome !== "complete" ||
      report.completed.length !== report.requested.length ||
      report.requested.some(({ fileName }) => !inspected.has(fileName))
    )
      throw new Error(
        "Acceptance requires a complete set and inspection of every requested image",
      );
    if (report.scope === "ci-fallback")
      throw new Error("CI fallback evidence cannot claim visual acceptance");
    if (input.unresolved.length)
      throw new Error(
        "Acceptance requires resolving all outstanding judgments",
      );
    for (const name of inspected) await readFile(path.join(directory, name));
  }
  const verdict = {
    verdict: input.verdict,
    reasons: input.reasons,
    inspected: [...inspected],
    unresolved: input.unresolved,
    rationale: report.rationale,
    scope: report.scope,
    completeProfileAccepted:
      input.verdict === "accepted" && report.scope === "complete",
    recordedAt: new Date().toISOString(),
  };
  // Exclusive creation keeps concurrent reviewers from replacing each other's
  // judgment. A new capture review is required for a revised verdict.
  await writeFile(
    path.join(directory, "verdict.json"),
    JSON.stringify(verdict, null, 2) + "\n",
    { flag: "wx", mode: 0o600 },
  );
  report.verdict = verdict;
  report.visualAcceptance =
    input.verdict === "accepted"
      ? `${report.scope} scope accepted`
      : input.verdict;
  saveCaptureReport(directory, report);
  console.log(`Visual verdict: ${report.visualAcceptance}`);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--list") return listScopes();
  const options = parseOptions(args);
  if (options["--record"] || options["--verdict-file"])
    return recordVerdict(options);
  await captureReview(options);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
