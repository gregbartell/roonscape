import { fileURLToPath } from "node:url";

import { readFixtureScenarioCatalog } from "../src/bridge/dist/src/fixture-scenario-catalog.js";

const REFERENCE_VIEWPORT = {
  key: "3840x2160",
  width: 3840,
  height: 2160,
};

const VIEWPORTS = [
  REFERENCE_VIEWPORT,
  { key: "3840x2400", width: 3840, height: 2400 },
  { key: "1600x900", width: 1600, height: 900 },
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

export function buildGalleryCapturePlan({
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

  return [...matrix, ...REPRESENTATIVES];
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
    ...viewportFields(REFERENCE_VIEWPORT),
    typography,
    diagnostics,
    fileName: `${REFERENCE_VIEWPORT.key}--representative--${scenario}.png`,
  };
}

function viewportFields(viewport) {
  return {
    viewport: viewport.key,
    width: viewport.width,
    height: viewport.height,
  };
}
