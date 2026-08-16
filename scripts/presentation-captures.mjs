import { fileURLToPath } from "node:url";

import { readFixtureScenarioCatalog } from "../src/bridge/dist/src/fixture-scenario-catalog.js";

const VIEWPORTS = [
  { key: "1280x720", width: 1280, height: 720 },
  { key: "1600x1200", width: 1600, height: 1200 },
  { key: "1920x1200", width: 1920, height: 1200 },
  { key: "2560x1080", width: 2560, height: 1080 },
  { key: "3840x2160", width: 3840, height: 2160 },
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
      ...viewportFields(viewport),
      typography: "automatic",
      diagnostics: false,
      fileName: `${viewport.key}--${scenario.scenario}.png`,
    })),
  );
  const representatives = VIEWPORTS.flatMap((viewport) =>
    REPRESENTATIVES.map((capture) => ({
      ...capture,
      ...viewportFields(viewport),
      fileName: `${viewport.key}--representative--${capture.scenario}.png`,
    })),
  );

  return [...matrix, ...representatives];
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

function viewportFields(viewport) {
  return {
    viewport: viewport.key,
    width: viewport.width,
    height: viewport.height,
  };
}
