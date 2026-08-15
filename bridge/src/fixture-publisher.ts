import { chmod, mkdir } from "node:fs/promises";
import { createServer, type Server } from "node:net";
import path from "node:path";

import type { PresentationSnapshot } from "./snapshot.js";

export interface FixturePublisher {
  close(): Promise<void>;
}

export async function startFixturePublisher(
  snapshot: PresentationSnapshot,
  socketPath: string,
): Promise<FixturePublisher> {
  const runtimeDirectory = path.dirname(socketPath);
  await mkdir(runtimeDirectory, { mode: 0o700, recursive: true });
  await chmod(runtimeDirectory, 0o700);

  const message = `${JSON.stringify(snapshot)}\n`;
  const server = createServer((connection) => {
    connection.end(message);
  });

  await listen(server, socketPath);
  await chmod(socketPath, 0o600);

  return {
    close: () => close(server),
  };
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
