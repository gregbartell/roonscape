import { chmod, mkdir, stat } from "node:fs/promises";
import { createServer, type Server, type Socket } from "node:net";
import path from "node:path";

import type { PresentationSnapshot } from "./snapshot.js";

export interface SnapshotPublisher {
  publish(snapshot: PresentationSnapshot): void;
  close(): Promise<void>;
}

export async function startSnapshotPublisher(
  snapshot: PresentationSnapshot,
  socketPath: string,
): Promise<SnapshotPublisher> {
  const runtimeDirectory = path.dirname(socketPath);
  await mkdir(runtimeDirectory, { mode: 0o700, recursive: true });
  if (((await stat(runtimeDirectory)).mode & 0o077) !== 0) {
    throw new Error(`Runtime directory must be private: ${runtimeDirectory}`);
  }

  let message = `${JSON.stringify(snapshot)}\n`;
  const connections = new Set<Socket>();
  const server = createServer((connection) => {
    connections.add(connection);
    connection.once("close", () => connections.delete(connection));
    connection.once("error", () => connections.delete(connection));
    connection.write(message);
  });

  await listen(server, socketPath);
  await chmod(socketPath, 0o600);

  return {
    publish: (nextSnapshot) => {
      message = `${JSON.stringify(nextSnapshot)}\n`;
      for (const connection of connections) {
        connection.write(message);
      }
    },
    close: () => {
      for (const connection of connections) {
        connection.destroy();
      }
      return close(server);
    },
  };
}

export const startFixturePublisher = startSnapshotPublisher;

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
