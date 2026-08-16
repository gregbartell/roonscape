import { chmod } from "node:fs/promises";
import { createServer, type Server, type Socket } from "node:net";

import type { FixtureScenario } from "./fixture-scenario-catalog.js";
import {
  assertSnapshotPublishable,
  startSnapshotPublisher,
} from "./fixture-publisher.js";
import type { PresentationSnapshot } from "./snapshot.js";

export type FixtureNavigationIntent = "Previous" | "Next";

export interface FixtureModeSession {
  close(): Promise<void>;
}

export async function startFixtureModeSession(
  catalog: readonly FixtureScenario[],
  socketPath: string,
  controlSocketPath: string,
  now: () => Date = () => new Date(),
  log: (message: string) => void = (message) => process.stdout.write(message),
): Promise<FixtureModeSession> {
  const initialScenario = catalog[0];
  if (initialScenario === undefined) {
    throw new Error("Fixture Scenario catalog is unexpectedly empty");
  }
  for (const scenario of catalog) {
    try {
      assertSnapshotPublishable(scenario.snapshot);
    } catch (error) {
      throw new Error(
        `Could not publish Fixture Scenario "${scenario.label}": ${error instanceof Error ? error.message : String(error)}`,
        { cause: error },
      );
    }
  }

  let selectedIndex = 0;
  let revision = 1;
  const publisher = await startSnapshotPublisher(
    selectedSnapshot(initialScenario, revision, now()),
    socketPath,
  );
  const connections = new Set<Socket>();
  let controlServer: Server;

  try {
    controlServer = createServer((connection) => {
      connections.add(connection);
      connection.setEncoding("utf8");
      let pending = "";
      const removeConnection = (): void => {
        connections.delete(connection);
      };
      connection.once("close", removeConnection);
      connection.once("error", removeConnection);
      connection.on("data", (chunk: string) => {
        pending += chunk;
        let newline = pending.indexOf("\n");
        while (newline >= 0) {
          const intent = pending.slice(0, newline).trimEnd();
          pending = pending.slice(newline + 1);
          if (isNavigationIntent(intent)) {
            selectedIndex = wrappedSelection(
              selectedIndex,
              intent,
              catalog.length,
            );
            revision += 1;
            const selected = catalog[selectedIndex];
            if (selected !== undefined) {
              publisher.publish(selectedSnapshot(selected, revision, now()));
              log(`Fixture Scenario: ${selected.label}\n`);
            }
          }
          newline = pending.indexOf("\n");
        }
      });
    });
    await listen(controlServer, controlSocketPath);
    await chmod(controlSocketPath, 0o600);
  } catch (error) {
    await publisher.close();
    throw error;
  }

  log(`Fixture Scenario: ${initialScenario.label}\n`);
  return {
    close: async () => {
      for (const connection of connections) {
        connection.destroy();
      }
      await Promise.all([close(controlServer), publisher.close()]);
    },
  };
}

function selectedSnapshot(
  scenario: FixtureScenario,
  revision: number,
  selectedAt: Date,
): PresentationSnapshot {
  const snapshot = scenario.snapshot;
  return {
    ...snapshot,
    revision,
    progress:
      snapshot.playback === "playing" && snapshot.progress !== null
        ? { ...snapshot.progress, sampledAt: selectedAt.toISOString() }
        : snapshot.progress,
  };
}

function wrappedSelection(
  current: number,
  intent: FixtureNavigationIntent,
  scenarioCount: number,
): number {
  const delta = intent === "Next" ? 1 : -1;
  return (current + delta + scenarioCount) % scenarioCount;
}

function isNavigationIntent(value: string): value is FixtureNavigationIntent {
  return value === "Previous" || value === "Next";
}

function listen(server: Server, socketPath: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const onError = (error: Error): void => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = (): void => {
      server.off("error", onError);
      resolve();
    };

    server.once("error", onError);
    server.once("listening", onListening);
    server.listen(socketPath);
  });
}

function close(server: Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => {
      if (error === undefined) {
        resolve();
      } else {
        reject(error);
      }
    });
  });
}
