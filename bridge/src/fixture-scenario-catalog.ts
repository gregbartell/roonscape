import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadSnapshot, type PresentationSnapshot } from "./snapshot.js";

const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));
const defaultCatalogPath = "fixtures/fixture-scenario-catalog.json";
const fixtureScenarioCount = 18;

export type FixturePalette = "dark" | "light" | "fixed-no-art";

const palettes: ReadonlySet<string> = new Set<FixturePalette>([
  "dark",
  "light",
  "fixed-no-art",
]);

export interface FixtureScenario {
  scenario: string;
  label: string;
  fixture: string;
  palette: FixturePalette;
  snapshot: PresentationSnapshot;
}

export interface FixtureScenarioEntry {
  scenario: string;
  label: string;
  fixture: string;
  palette: FixturePalette;
}

export async function loadFixtureScenarioCatalog(
  catalogPath = defaultCatalogPath,
): Promise<FixtureScenario[]> {
  const entries = readFixtureScenarioCatalog(catalogPath);
  return Promise.all(
    entries.map(async (entry) => {
      try {
        return {
          ...entry,
          snapshot: await loadSnapshot(entry.fixture),
        };
      } catch (error) {
        throw new Error(
          `Could not load Fixture Scenario "${entry.label}" from ${entry.fixture}: ${errorMessage(error)}`,
          { cause: error },
        );
      }
    }),
  );
}

export function readFixtureScenarioCatalog(
  catalogPath = defaultCatalogPath,
): FixtureScenarioEntry[] {
  const resolvedCatalogPath = path.resolve(repositoryRoot, catalogPath);
  let candidate: unknown;
  try {
    candidate = JSON.parse(readFileSync(resolvedCatalogPath, "utf8"));
  } catch (error) {
    throw new Error(
      `Could not load Fixture Scenario catalog at ${resolvedCatalogPath}: ${errorMessage(error)}`,
      { cause: error },
    );
  }

  return validateCatalog(candidate);
}

function validateCatalog(candidate: unknown): FixtureScenarioEntry[] {
  if (
    !isRecord(candidate) ||
    candidate.formatVersion !== 1 ||
    !Array.isArray(candidate.scenarios) ||
    candidate.scenarios.length !== fixtureScenarioCount
  ) {
    throw new Error(
      `Invalid Fixture Scenario catalog: expected format version 1 with exactly ${fixtureScenarioCount} scenarios`,
    );
  }

  const entries = candidate.scenarios.map((entry, index) =>
    validateEntry(entry, index),
  );
  if (entries[0]?.scenario !== "playing") {
    throw new Error(
      'Invalid Fixture Scenario catalog entry 1: ordinary Fixture Mode must start with scenario "playing"',
    );
  }

  assertUnique(entries, "scenario");
  assertUnique(entries, "label");
  assertUnique(entries, "fixture");
  return entries;
}

function validateEntry(
  candidate: unknown,
  index: number,
): FixtureScenarioEntry {
  if (
    !isRecord(candidate) ||
    !isNonemptyString(candidate.scenario) ||
    !isNonemptyString(candidate.label) ||
    !isNonemptyString(candidate.fixture) ||
    !isFixturePalette(candidate.palette)
  ) {
    throw new Error(
      `Invalid Fixture Scenario catalog entry ${index + 1}: expected scenario, label, fixture, and palette`,
    );
  }

  return {
    scenario: candidate.scenario,
    label: candidate.label,
    fixture: candidate.fixture,
    palette: candidate.palette,
  };
}

function assertUnique(
  entries: FixtureScenarioEntry[],
  field: keyof FixtureScenarioEntry,
): void {
  const values = new Set<string>();
  for (const [index, entry] of entries.entries()) {
    if (values.has(entry[field])) {
      throw new Error(
        `Invalid Fixture Scenario catalog entry ${index + 1}: duplicate ${field} ${entry[field]}`,
      );
    }
    values.add(entry[field]);
  }
}

function isRecord(candidate: unknown): candidate is Record<string, unknown> {
  return (
    candidate !== null &&
    typeof candidate === "object" &&
    !Array.isArray(candidate)
  );
}

function isNonemptyString(candidate: unknown): candidate is string {
  return typeof candidate === "string" && candidate.length > 0;
}

function isFixturePalette(candidate: unknown): candidate is FixturePalette {
  return typeof candidate === "string" && palettes.has(candidate);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
