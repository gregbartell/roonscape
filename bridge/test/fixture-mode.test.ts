import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { access, readFile, writeFile } from "node:fs/promises";
import { createConnection } from "node:net";
import path from "node:path";
import { createInterface } from "node:readline";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { loadSnapshot, type PresentationSnapshot } from "../src/snapshot.js";
import { withTaskDirectory } from "./support.js";

const fixtureEntry = fileURLToPath(
  new URL("../src/fixture.js", import.meta.url),
);
const catalogFixture = new URL(
  "../../../fixtures/fixture-scenario-catalog.json",
  import.meta.url,
);

interface CatalogFile {
  formatVersion: number;
  scenarios: Array<{
    scenario: string;
    label: string;
    fixture: string;
    palette: string;
  }>;
}

test("ordinary Fixture Mode starts predictably at Playing", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const socketPath = path.join(taskDirectory, "roonscape.sock");
    const fixture = startFixture({
      ROONSCAPE_SOCKET: socketPath,
      ROONSCAPE_FIXTURE: undefined,
      ROONSCAPE_FIXTURE_CATALOG: undefined,
    });

    try {
      const published = await readPublishedSnapshot(fixture.child, socketPath);
      const expected = await loadSnapshot("fixtures/playing.json");

      assert.deepEqual(published, {
        ...expected,
        progress: {
          ...expected.progress,
          sampledAt: published.progress?.sampledAt,
        },
      });
      assert.notEqual(
        published.progress?.sampledAt,
        expected.progress?.sampledAt,
      );
    } finally {
      await stop(fixture.child);
    }
  });
});

test("explicit single-fixture startup does not require the ordinary catalog", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const socketPath = path.join(taskDirectory, "roonscape.sock");
    const fixture = startFixture({
      ROONSCAPE_SOCKET: socketPath,
      ROONSCAPE_FIXTURE: "fixtures/paused.json",
      ROONSCAPE_FIXTURE_CATALOG: path.join(taskDirectory, "missing.json"),
    });

    try {
      assert.deepEqual(
        await readPublishedSnapshot(fixture.child, socketPath),
        await loadSnapshot("fixtures/paused.json"),
      );
    } finally {
      await stop(fixture.child);
    }
  });
});

test("ordinary Fixture Mode validates the complete catalog before becoming available", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const socketPath = path.join(taskDirectory, "roonscape.sock");
    const catalogPath = path.join(
      taskDirectory,
      "fixture-scenario-catalog.json",
    );
    const catalog = JSON.parse(
      await readFile(catalogFixture, "utf8"),
    ) as CatalogFile;
    const finalScenario = catalog.scenarios.at(-1);
    assert.ok(finalScenario);
    finalScenario.fixture = "fixtures/invalid.json";
    await writeFile(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`);

    const fixture = startFixture({
      ROONSCAPE_SOCKET: socketPath,
      ROONSCAPE_FIXTURE: undefined,
      ROONSCAPE_FIXTURE_CATALOG: catalogPath,
    });

    try {
      const result = await closeWithin(fixture.child, 1_000);

      assert.deepEqual(result, { exitCode: 1, signal: null });
      assert.match(
        fixture.standardError(),
        /Light artwork.*Invalid presentation snapshot/s,
      );
      await assert.rejects(access(socketPath), { code: "ENOENT" });
    } finally {
      await stop(fixture.child);
    }
  });
});

function startFixture(
  environmentOverrides: Record<string, string | undefined>,
) {
  let standardError = "";
  const environment = { ...process.env, ...environmentOverrides };
  for (const [name, value] of Object.entries(environment)) {
    if (value === undefined) {
      delete environment[name];
    }
  }
  const child = spawn(process.execPath, [fixtureEntry], {
    env: environment,
    stdio: ["ignore", "ignore", "pipe"],
  });
  child.stderr?.setEncoding("utf8");
  child.stderr?.on("data", (chunk: string) => {
    standardError += chunk;
  });

  return { child, standardError: () => standardError };
}

async function readPublishedSnapshot(
  child: ChildProcess,
  socketPath: string,
): Promise<PresentationSnapshot> {
  await waitForSocket(child, socketPath);
  const client = createConnection(socketPath);
  const lines = createInterface({ input: client });
  try {
    const [line] = (await once(lines, "line")) as [string];
    return JSON.parse(line) as PresentationSnapshot;
  } finally {
    client.destroy();
  }
}

async function waitForSocket(
  child: ChildProcess,
  socketPath: string,
): Promise<void> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(
        `Fixture publisher exited before opening the socket (${child.exitCode ?? child.signalCode})`,
      );
    }
    try {
      await access(socketPath);
      return;
    } catch (error) {
      if (!(
        error instanceof Error &&
        "code" in error &&
        error.code === "ENOENT"
      )) {
        throw error;
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 25));
  }

  throw new Error("Timed out waiting for the Fixture Mode publisher");
}

function closeWithin(
  child: ChildProcess,
  milliseconds: number,
): Promise<{ exitCode: number | null; signal: NodeJS.Signals | null } | null> {
  return new Promise((resolve) => {
    const onClose = (
      exitCode: number | null,
      signal: NodeJS.Signals | null,
    ): void => {
      clearTimeout(timer);
      resolve({ exitCode, signal });
    };
    const timer = setTimeout(() => {
      child.off("close", onClose);
      resolve(null);
    }, milliseconds);
    child.once("close", onClose);
  });
}

async function stop(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }

  const closed = once(child, "close");
  child.kill("SIGTERM");
  await closed;
}
