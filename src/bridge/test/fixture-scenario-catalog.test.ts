import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import { readFixtureScenarioCatalog } from "../src/fixture-scenario-catalog.js";
import { repositoryRoot } from "../src/repository-root.js";
import { withTaskDirectory } from "./support.js";

const catalogFixture = path.join(
  repositoryRoot,
  "src/shared/fixtures/fixture-scenario-catalog.json",
);

interface CatalogEntry {
  scenario: string;
  label: string;
  fixture: string;
  palette: string;
}

interface CatalogFile {
  formatVersion: number;
  scenarios: CatalogEntry[];
}

test("accepts an additional valid Fixture Scenario", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const catalog = await readCatalogFixture();
    const initialScenarioCount = catalog.scenarios.length;
    catalog.scenarios.push({
      scenario: "additional-scenario",
      label: "Additional scenario",
      fixture: "src/shared/fixtures/additional-scenario.json",
      palette: "dark",
    });
    const catalogPath = await writeCatalog(taskDirectory, catalog);

    const scenarios = readFixtureScenarioCatalog(catalogPath);

    assert.equal(scenarios.length, initialScenarioCount + 1);
    assert.deepEqual(scenarios.at(-1), {
      scenario: "additional-scenario",
      label: "Additional scenario",
      fixture: "src/shared/fixtures/additional-scenario.json",
      palette: "dark",
    });
  });
});

test("accepts a valid Fixture Scenario catalog with an entry removed", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const catalog = await readCatalogFixture();
    const initialScenarioCount = catalog.scenarios.length;
    catalog.scenarios.pop();
    const catalogPath = await writeCatalog(taskDirectory, catalog);

    const scenarios = readFixtureScenarioCatalog(catalogPath);

    assert.equal(scenarios.length, initialScenarioCount - 1);
  });
});

test("requires Fixture Scenario catalog format version 1", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const catalog = await readCatalogFixture();
    catalog.formatVersion = 2;
    const catalogPath = await writeCatalog(taskDirectory, catalog);

    assert.throws(
      () => readFixtureScenarioCatalog(catalogPath),
      /expected format version 1/,
    );
  });
});

test("requires complete Fixture Scenario entries with supported palettes", async (context) => {
  const invalidFields: Array<[keyof CatalogEntry, unknown]> = [
    ["scenario", ""],
    ["label", ""],
    ["fixture", ""],
    ["palette", "unsupported"],
  ];

  for (const [field, invalidValue] of invalidFields) {
    await context.test(field, async () => {
      await withTaskDirectory(async (taskDirectory) => {
        const catalog = await readCatalogFixture();
        Object.assign(catalog.scenarios[1]!, { [field]: invalidValue });
        const catalogPath = await writeCatalog(taskDirectory, catalog);

        assert.throws(
          () => readFixtureScenarioCatalog(catalogPath),
          /expected scenario, label, fixture, and palette/,
        );
      });
    });
  }
});

test("requires unique Fixture Scenario identifiers, labels, and fixtures", async (context) => {
  for (const field of ["scenario", "label", "fixture"] as const) {
    await context.test(field, async () => {
      await withTaskDirectory(async (taskDirectory) => {
        const catalog = await readCatalogFixture();
        catalog.scenarios[1]![field] = catalog.scenarios[0]![field];
        const catalogPath = await writeCatalog(taskDirectory, catalog);

        assert.throws(
          () => readFixtureScenarioCatalog(catalogPath),
          new RegExp(`duplicate ${field}`),
        );
      });
    });
  }
});

test("requires Playing as the initial Fixture Scenario", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const catalog = await readCatalogFixture();
    [catalog.scenarios[0], catalog.scenarios[1]] = [
      catalog.scenarios[1]!,
      catalog.scenarios[0]!,
    ];
    const catalogPath = await writeCatalog(taskDirectory, catalog);

    assert.throws(
      () => readFixtureScenarioCatalog(catalogPath),
      /must start with scenario "playing"/,
    );
  });
});

async function readCatalogFixture(): Promise<CatalogFile> {
  return JSON.parse(await readFile(catalogFixture, "utf8")) as CatalogFile;
}

async function writeCatalog(
  taskDirectory: string,
  catalog: CatalogFile,
): Promise<string> {
  const catalogPath = path.join(taskDirectory, "fixture-scenario-catalog.json");
  await writeFile(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`);
  return catalogPath;
}
