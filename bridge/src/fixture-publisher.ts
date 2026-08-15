import { chmod, lstat, mkdir, stat, unlink } from "node:fs/promises";
import {
  createConnection,
  createServer,
  type Server,
  type Socket,
} from "node:net";
import path from "node:path";

import type { PresentationSnapshot } from "./snapshot.js";

export const MAX_SNAPSHOT_BYTES = 64 * 1024;

export interface SnapshotPublisher {
  publish(snapshot: PresentationSnapshot): void;
  close(): Promise<void>;
}

export async function startSnapshotPublisher(
  snapshot: PresentationSnapshot,
  socketPath: string,
): Promise<SnapshotPublisher> {
  let message = serializeSnapshot(snapshot);
  const runtimeDirectory = path.dirname(socketPath);
  await mkdir(runtimeDirectory, { mode: 0o700, recursive: true });
  if (((await stat(runtimeDirectory)).mode & 0o077) !== 0) {
    throw new Error(`Runtime directory must be private: ${runtimeDirectory}`);
  }

  const connections = new Set<SnapshotConnection>();
  const server = createServer(
    { highWaterMark: MAX_SNAPSHOT_BYTES },
    (socket) => {
      const connection: SnapshotConnection = {
        socket,
        blocked: false,
        pending: null,
      };
      connections.add(connection);
      const removeConnection = (): void => {
        connections.delete(connection);
      };
      socket.once("close", removeConnection);
      socket.once("error", removeConnection);
      socket.on("drain", () => flushPending(connection));
      writeLatest(connection, message);
    },
  );

  await listen(server, socketPath);
  await chmod(socketPath, 0o600);

  return {
    publish: (nextSnapshot) => {
      message = serializeSnapshot(nextSnapshot);
      for (const connection of connections) {
        writeLatest(connection, message);
      }
    },
    close: () => {
      for (const connection of connections) {
        connection.socket.destroy();
      }
      return close(server);
    },
  };
}

export const startFixturePublisher = startSnapshotPublisher;

interface SnapshotConnection {
  socket: Socket;
  blocked: boolean;
  pending: string | null;
}

function writeLatest(connection: SnapshotConnection, message: string): void {
  if (connection.blocked) {
    connection.pending = message;
    return;
  }

  connection.blocked = !connection.socket.write(message);
}

function flushPending(connection: SnapshotConnection): void {
  connection.blocked = false;
  const pending = connection.pending;
  connection.pending = null;
  if (pending !== null) {
    writeLatest(connection, pending);
  }
}

function serializeSnapshot(snapshot: PresentationSnapshot): string {
  const message = `${JSON.stringify(snapshot)}\n`;
  if (Buffer.byteLength(message, "utf8") > MAX_SNAPSHOT_BYTES) {
    throw new RangeError("Snapshot exceeds 64 KiB");
  }
  return message;
}

async function listen(server: Server, socketPath: string): Promise<void> {
  try {
    await listenOnce(server, socketPath);
  } catch (error) {
    if (!isErrorCode(error, "EADDRINUSE")) {
      throw error;
    }

    let socketMetadata;
    try {
      socketMetadata = await lstat(socketPath);
    } catch (statError) {
      if (isErrorCode(statError, "ENOENT")) {
        await listenOnce(server, socketPath);
        return;
      }
      throw statError;
    }
    if (!socketMetadata.isSocket() || (await acceptsConnections(socketPath))) {
      throw error;
    }

    await unlink(socketPath);
    await listenOnce(server, socketPath);
  }
}

function listenOnce(server: Server, socketPath: string): Promise<void> {
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

function acceptsConnections(socketPath: string): Promise<boolean> {
  return new Promise((resolve, reject) => {
    const socket = createConnection(socketPath);
    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", (error: NodeJS.ErrnoException) => {
      socket.destroy();
      if (error.code === "ECONNREFUSED" || error.code === "ENOENT") {
        resolve(false);
      } else {
        reject(error);
      }
    });
  });
}

function isErrorCode(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
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
