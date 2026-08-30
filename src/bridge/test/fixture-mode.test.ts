import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { access, readFile, stat, writeFile } from "node:fs/promises";
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
  "../../../../src/shared/fixtures/fixture-scenario-catalog.json",
  import.meta.url,
);
const catalogPreflightTimeoutMilliseconds = 1_500;

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
    const { socketPath, controlSocketPath, fixture } =
      startOrdinaryFixture(taskDirectory);

    try {
      const published = await readPublishedSnapshot(fixture.child, socketPath);
      const expected = await loadSnapshot("src/shared/fixtures/playing.json");

      assert.deepEqual(published, {
        ...expected,
        revision: 1,
        progress: {
          ...expected.progress,
          sampledAt: published.progress?.sampledAt,
        },
      });
      assert.notEqual(
        published.progress?.sampledAt,
        expected.progress?.sampledAt,
      );
      assert.equal((await stat(controlSocketPath)).mode & 0o777, 0o600);
    } finally {
      await stop(fixture.child);
    }
  });
});

test("static Fixture Mode preserves the Playing scenario source position", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath, fixture } = startOrdinaryFixture(
      taskDirectory,
      undefined,
      true,
    );

    try {
      const snapshots = await publishedSnapshots(fixture.child, socketPath);
      const expected = await loadSnapshot("src/shared/fixtures/playing.json");

      assert.deepEqual(await snapshots.read(), { ...expected, revision: 1 });
      await sendNavigationIntent(fixture.child, controlSocketPath, "Next");
      assertSelectedSnapshot(
        await snapshots.read(),
        await loadSnapshot("src/shared/fixtures/paused.json"),
        2,
      );
      await sendNavigationIntent(fixture.child, controlSocketPath, "Previous");
      assert.deepEqual(await snapshots.read(), { ...expected, revision: 3 });
      snapshots.close();
    } finally {
      await stop(fixture.child);
    }
  });
});

test("Next publishes the next Fixture Scenario as a complete snapshot", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath, fixture } =
      startOrdinaryFixture(taskDirectory);

    try {
      const snapshots = await publishedSnapshots(fixture.child, socketPath);
      const initial = await snapshots.read();

      await sendNavigationIntent(fixture.child, controlSocketPath, "Next");
      const selected = await snapshots.read();
      const expected = await loadSnapshot("src/shared/fixtures/paused.json");

      assert.equal(initial.revision, 1);
      assert.deepEqual(selected, { ...expected, revision: 2 });
      snapshots.close();
    } finally {
      await stop(fixture.child);
    }
  });
});

test("navigation publishes all Fixture Scenarios in order with wraparound revisions", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath, fixture } =
      startOrdinaryFixture(taskDirectory);

    try {
      const catalog = JSON.parse(
        await readFile(catalogFixture, "utf8"),
      ) as CatalogFile;
      const snapshots = await publishedSnapshots(fixture.child, socketPath);
      const observed = [await snapshots.read()];

      for (let index = 1; index < catalog.scenarios.length; index += 1) {
        await sendNavigationIntent(fixture.child, controlSocketPath, "Next");
        observed.push(await snapshots.read());
      }

      for (const [index, scenario] of catalog.scenarios.entries()) {
        const expected = await loadSnapshot(scenario.fixture);
        assertSelectedSnapshot(observed[index], expected, index + 1);
      }

      await new Promise((resolve) => setTimeout(resolve, 10));
      await sendNavigationIntent(fixture.child, controlSocketPath, "Next");
      const wrappedPlaying = await snapshots.read();
      const expectedPlaying = await loadSnapshot(
        "src/shared/fixtures/playing.json",
      );
      assertSelectedSnapshot(wrappedPlaying, expectedPlaying, 20);
      assert.notEqual(
        wrappedPlaying.progress?.sampledAt,
        observed[0]?.progress?.sampledAt,
      );

      await sendNavigationIntent(fixture.child, controlSocketPath, "Previous");
      assertSelectedSnapshot(
        await snapshots.read(),
        await loadSnapshot("src/shared/fixtures/light-artwork.json"),
        21,
      );
      snapshots.close();
    } finally {
      await stop(fixture.child);
    }
  });
});

test("rapid distinct navigation publishes the latest deliberate selection", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath, fixture } =
      startOrdinaryFixture(taskDirectory);

    try {
      const snapshots = await publishedSnapshots(fixture.child, socketPath);
      await snapshots.read();
      await sendNavigationIntents(fixture.child, controlSocketPath, [
        "Next",
        "Next",
        "Previous",
        "Next",
      ]);

      let latest = await snapshots.read();
      while (latest.revision < 5) {
        latest = await snapshots.read();
      }
      assertSelectedSnapshot(
        latest,
        await loadSnapshot("src/shared/fixtures/loading.json"),
        5,
      );
      snapshots.close();
    } finally {
      await stop(fixture.child);
    }
  });
});

test("ordinary Fixture Mode logs initial and selected Scenario names", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const { socketPath, controlSocketPath, fixture } =
      startOrdinaryFixture(taskDirectory);

    try {
      await readPublishedSnapshot(fixture.child, socketPath);
      await sendNavigationIntent(fixture.child, controlSocketPath, "Next");
      await waitForOutput(fixture.standardOutput, "Fixture Scenario: Paused");

      assert.match(
        fixture.standardOutput(),
        /Fixture Scenario: Playing.*Fixture Scenario: Paused/s,
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
      ROONSCAPE_FIXTURE: "src/shared/fixtures/paused.json",
      ROONSCAPE_FIXTURE_CATALOG: path.join(taskDirectory, "missing.json"),
    });

    try {
      assert.deepEqual(
        await readPublishedSnapshot(fixture.child, socketPath),
        await loadSnapshot("src/shared/fixtures/paused.json"),
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
    finalScenario.fixture = "src/shared/fixtures/invalid.json";
    await writeFile(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`);

    const { fixture } = startOrdinaryFixture(taskDirectory, catalogPath);

    try {
      const result = await closeWithin(
        fixture.child,
        catalogPreflightTimeoutMilliseconds,
      );

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

test("ordinary Fixture Mode preflights every catalog snapshot against the publisher limit", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const socketPath = path.join(taskDirectory, "roonscape.sock");
    const oversizedFixturePath = path.join(
      taskDirectory,
      "oversized-fixture.json",
    );
    const oversized = await loadSnapshot("src/shared/fixtures/playing.json");
    assert.ok(oversized.nowPlaying);
    oversized.nowPlaying.title = "x".repeat(64 * 1024);
    await writeFile(
      oversizedFixturePath,
      `${JSON.stringify(oversized, null, 2)}\n`,
    );

    const catalogPath = path.join(
      taskDirectory,
      "fixture-scenario-catalog.json",
    );
    const catalog = JSON.parse(
      await readFile(catalogFixture, "utf8"),
    ) as CatalogFile;
    const finalScenario = catalog.scenarios.at(-1);
    assert.ok(finalScenario);
    finalScenario.fixture = oversizedFixturePath;
    await writeFile(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`);

    const { fixture, controlSocketPath } = startOrdinaryFixture(
      taskDirectory,
      catalogPath,
    );
    try {
      assert.deepEqual(
        await closeWithin(fixture.child, catalogPreflightTimeoutMilliseconds),
        {
          exitCode: 1,
          signal: null,
        },
      );
      assert.match(
        fixture.standardError(),
        /Light artwork.*Snapshot exceeds 64 KiB/s,
      );
      await assert.rejects(access(socketPath), { code: "ENOENT" });
      await assert.rejects(access(controlSocketPath), { code: "ENOENT" });
    } finally {
      await stop(fixture.child);
    }
  });
});

function startFixture(
  environmentOverrides: Record<string, string | undefined>,
) {
  let standardOutput = "";
  let standardError = "";
  const environment = { ...process.env, ...environmentOverrides };
  for (const [name, value] of Object.entries(environment)) {
    if (value === undefined) {
      delete environment[name];
    }
  }
  const child = spawn(process.execPath, [fixtureEntry], {
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout?.setEncoding("utf8");
  child.stdout?.on("data", (chunk: string) => {
    standardOutput += chunk;
  });
  child.stderr?.setEncoding("utf8");
  child.stderr?.on("data", (chunk: string) => {
    standardError += chunk;
  });

  return {
    child,
    standardOutput: () => standardOutput,
    standardError: () => standardError,
  };
}

function startOrdinaryFixture(
  taskDirectory: string,
  catalogPath?: string,
  staticFixture = false,
) {
  const socketPath = path.join(taskDirectory, "roonscape.sock");
  const controlSocketPath = path.join(taskDirectory, "fixture-navigation.sock");
  const fixture = startFixture({
    ROONSCAPE_SOCKET: socketPath,
    ROONSCAPE_FIXTURE_CONTROL: controlSocketPath,
    ROONSCAPE_FIXTURE: undefined,
    ROONSCAPE_FIXTURE_CATALOG: catalogPath,
    ROONSCAPE_STATIC_FIXTURE: staticFixture ? "1" : undefined,
  });
  return { socketPath, controlSocketPath, fixture };
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

async function publishedSnapshots(child: ChildProcess, socketPath: string) {
  await waitForSocket(child, socketPath);
  const client = createConnection(socketPath);
  const lines = createInterface({ input: client });
  const snapshots = lines[Symbol.asyncIterator]();

  return {
    read: async (): Promise<PresentationSnapshot> => {
      const next = await snapshots.next();
      assert.equal(next.done, false);
      return JSON.parse(next.value ?? "null") as PresentationSnapshot;
    },
    close: () => client.destroy(),
  };
}

async function sendNavigationIntent(
  child: ChildProcess,
  controlSocketPath: string,
  intent: "Previous" | "Next",
): Promise<void> {
  await sendNavigationIntents(child, controlSocketPath, [intent]);
}

async function sendNavigationIntents(
  child: ChildProcess,
  controlSocketPath: string,
  intents: Array<"Previous" | "Next">,
): Promise<void> {
  await waitForSocket(child, controlSocketPath);
  const control = createConnection(controlSocketPath);
  await once(control, "connect");
  const closed = once(control, "close");
  control.end(`${intents.join("\n")}\n`);
  await closed;
}

function assertSelectedSnapshot(
  actual: PresentationSnapshot | undefined,
  expected: PresentationSnapshot,
  revision: number,
): void {
  assert.ok(actual);
  assert.deepEqual(actual, {
    ...expected,
    revision,
    progress:
      expected.playback === "playing" && expected.progress !== null
        ? { ...expected.progress, sampledAt: actual.progress?.sampledAt }
        : expected.progress,
  });
  if (expected.playback === "playing" && expected.progress !== null) {
    assert.notEqual(actual.progress?.sampledAt, expected.progress.sampledAt);
  }
}

async function waitForOutput(
  output: () => string,
  expected: string,
): Promise<void> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (output().includes(expected)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error(`Timed out waiting for output: ${expected}`);
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
