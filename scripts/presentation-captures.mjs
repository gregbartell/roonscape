import { fileURLToPath } from "node:url";

import { readFixtureScenarioCatalog } from "../src/bridge/dist/src/fixture-scenario-catalog.js";

const VIEWPORTS = [
  presentationCaptureResolution(1280, 720),
  presentationCaptureResolution(1600, 900),
  presentationCaptureResolution(1600, 1200),
  presentationCaptureResolution(1920, 1200),
  presentationCaptureResolution(2560, 1080),
  presentationCaptureResolution(3840, 2160),
  presentationCaptureResolution(3840, 2400),
];

const defaultCatalogPath = fileURLToPath(
  new URL(
    "../src/shared/fixtures/fixture-scenario-catalog.json",
    import.meta.url,
  ),
);

const REPRESENTATIVES = [
  representative(
    "preferred-typography",
    "glyph-fallback.json",
    "preferred",
    false,
    "dark",
  ),
  representative(
    "fallback-typography",
    "glyph-fallback.json",
    "fallback",
    false,
    "dark",
  ),
  representative(
    "identity-baselines",
    "long-identities.json",
    "automatic",
    false,
    "dark",
  ),
  representative(
    "progress-early",
    "progress-early.json",
    "automatic",
    false,
    "dark",
  ),
  representative("progress-middle", "playing.json", "automatic", false, "dark"),
  representative(
    "progress-near-complete",
    "progress-near-complete.json",
    "automatic",
    false,
    "dark",
  ),
  representative("dark-diagnostics", "playing.json", "automatic", true, "dark"),
  representative(
    "light-diagnostics",
    "light-artwork.json",
    "automatic",
    true,
    "light",
  ),
  representative(
    "fixed-no-art-diagnostics",
    "missing-artwork.json",
    "automatic",
    true,
    "fixed-no-art",
  ),
  representative(
    "light-matte-restraint",
    "cellout-direction.json",
    "automatic",
    false,
    "restrained-light",
  ),
  representative(
    "dark-matte-ownership",
    "forever-direction.json",
    "automatic",
    false,
    "dark-teal",
  ),
];

export function buildPresentationCapturePlan({
  catalogPath = defaultCatalogPath,
} = {}) {
  const scenarios = readFixtureScenarioCatalog(catalogPath).map(
    ({ scenario, fixture, palette }) => ({ scenario, fixture, palette }),
  );
  const matrix = VIEWPORTS.flatMap((viewport) =>
    scenarios.map((scenario) => ({
      variant: "matrix",
      ...scenario,
      ...viewport,
      typography: "automatic",
      diagnostics: false,
      fileName: `${viewport.viewport}--${scenario.scenario}.png`,
    })),
  );
  const representatives = VIEWPORTS.flatMap((viewport) =>
    REPRESENTATIVES.map((capture) => ({
      ...capture,
      ...viewport,
      fileName: `${viewport.viewport}--representative--${capture.scenario}.png`,
    })),
  );

  return [...matrix, ...representatives];
}

export function listFixtureScenarios({
  catalogPath = defaultCatalogPath,
} = {}) {
  return readFixtureScenarioCatalog(catalogPath).map(({ scenario, label }) => ({
    scenario,
    label,
  }));
}

export function presentationCaptureResolution(width, height) {
  return { width, height, viewport: `${width}x${height}` };
}

export function selectFocusedPresentationCapture(plan, scenarioIdentifier) {
  const matchingCaptures = plan.filter(
    (capture) =>
      capture.variant === "matrix" &&
      capture.viewport === "3840x2160" &&
      capture.scenario === scenarioIdentifier,
  );
  if (matchingCaptures.length === 0) {
    throw new Error(
      `unknown Fixture Scenario identifier: ${scenarioIdentifier}`,
    );
  }
  if (matchingCaptures.length > 1) {
    throw new Error(
      `ambiguous Fixture Scenario identifier: ${scenarioIdentifier}`,
    );
  }
  return matchingCaptures[0];
}

function fixtureScenario(scenario, fixtureName, palette) {
  return {
    scenario,
    fixture: `src/shared/fixtures/${fixtureName}`,
    palette,
  };
}

function representative(
  scenario,
  fixtureName,
  typography,
  diagnostics,
  palette,
) {
  return {
    variant: "representative",
    ...fixtureScenario(scenario, fixtureName, palette),
    typography,
    diagnostics,
  };
}
