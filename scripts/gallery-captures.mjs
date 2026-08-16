const VIEWPORTS = [
  { key: "3840x2160", width: 3840, height: 2160 },
  { key: "3840x2400", width: 3840, height: 2400 },
  { key: "1600x900", width: 1600, height: 900 },
];

const SCENARIOS = [
  fixtureScenario("playing", "playing.json", "dark"),
  fixtureScenario("paused", "paused.json", "dark"),
  fixtureScenario("loading-with-content", "loading.json", "dark"),
  fixtureScenario(
    "loading-without-content",
    "loading-empty.json",
    "fixed-no-art",
  ),
  fixtureScenario("idle", "stopped.json", "fixed-no-art"),
  fixtureScenario("pairing-required", "pairing-required.json", "fixed-no-art"),
  fixtureScenario("disconnected", "disconnected.json", "fixed-no-art"),
  fixtureScenario(
    "output-unavailable",
    "output-unavailable.json",
    "fixed-no-art",
  ),
  fixtureScenario(
    "playing-without-content",
    "playing-empty.json",
    "fixed-no-art",
  ),
  fixtureScenario("missing-metadata", "missing-metadata.json", "dark"),
  fixtureScenario("missing-artist", "missing-artist.json", "dark"),
  fixtureScenario("missing-album", "missing-album.json", "dark"),
  fixtureScenario("missing-artwork", "missing-artwork.json", "fixed-no-art"),
  fixtureScenario("long-metadata", "long-metadata.json", "dark"),
  fixtureScenario("extreme-metadata", "extreme-metadata.json", "dark"),
  fixtureScenario(
    "indeterminate-progress",
    "indeterminate-progress.json",
    "dark",
  ),
  fixtureScenario("non-square-artwork", "non-square-artwork.json", "dark"),
  fixtureScenario("light-artwork", "light-artwork.json", "light"),
];

const REPRESENTATIVES = [
  representative(
    "preferred-typography",
    "playing.json",
    "preferred",
    false,
    "dark",
  ),
  representative(
    "fallback-typography",
    "playing.json",
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

export function buildGalleryCapturePlan() {
  const matrix = VIEWPORTS.flatMap((viewport) =>
    SCENARIOS.map((scenario) => ({
      variant: "matrix",
      ...scenario,
      viewport: viewport.key,
      width: viewport.width,
      height: viewport.height,
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
    fixture: `fixtures/${fixtureName}`,
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
    viewport: "3840x2160",
    width: 3840,
    height: 2160,
    typography,
    diagnostics,
    fileName: `3840x2160--representative--${scenario}.png`,
  };
}
