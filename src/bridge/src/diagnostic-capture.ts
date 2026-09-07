import { randomUUID } from "node:crypto";
import { Worker } from "node:worker_threads";

import { redactDiagnosticData } from "./diagnostic-redaction.js";

export interface DiagnosticSource {
  sessionId: string;
  sequence: number;
  type: string;
  context: unknown;
}
export type DiagnosticContributors = Partial<
  Record<
    "zone" | "timing" | "lyrics" | "artwork" | "availability",
    DiagnosticSource
  >
>;

export interface DiagnosticCaptureOptions {
  directory: string;
  budgetBytes?: number;
}
export const defaultCaptureBudgetBytes = 100 * 1024 * 1024;
const queueLimit = 1024 * 1024;

/** Operational evidence only: never await capture from a Roon callback. */
export class DiagnosticCapture {
  readonly #sessionId = randomUUID();
  readonly #recordLimit: number;
  readonly #report: (message: string) => void;
  #worker: Worker | undefined;
  #sequence = 0;
  #input: DiagnosticSource | undefined;
  #snapshot: unknown = { unavailable: "no published snapshot observed" };
  #checkpointProvider: () => Record<string, unknown> = () => ({
    roonState: { unavailable: "no Bridge state provider" },
  });

  setCheckpointProvider(provider: () => Record<string, unknown>): void {
    this.#checkpointProvider = provider;
  }

  #boundedContext(value: unknown, limit: number): unknown {
    try {
      const safe = redactDiagnosticData(value, limit);
      if (Buffer.byteLength(JSON.stringify(safe)) > limit) throw new Error();
      return safe;
    } catch {
      return {
        unavailable: "context omitted: diagnostic size or serialization limit",
      };
    }
  }

  #checkpoint(): string {
    const data: Record<string, unknown> = {
      kind: "retained state",
      snapshot: this.#snapshot,
      earlierRecordingGap: this.#hadOmissions,
    };
    try {
      const state = this.#checkpointProvider();
      for (const key of ["roonState", "lyricFeed", "pendingSnapshot"]) {
        data[key] = this.#boundedContext(
          state[key] ?? { unavailable: "not observed" },
          Math.floor(this.#recordLimit / 5),
        );
      }
    } catch {
      data.roonState = { unavailable: "checkpoint provider failed" };
    }
    const serialized = JSON.stringify(data);
    return Buffer.byteLength(serialized) <= this.#recordLimit - 256
      ? serialized
      : JSON.stringify({
          kind: "retained state",
          unavailable: "checkpoint exceeds diagnostic size limit",
        });
  }

  readonly #contributors = new WeakMap<object, DiagnosticContributors>();

  get input(): DiagnosticSource | undefined {
    return this.#input;
  }

  /** Scope only synchronous SDK dispatch; async consumers retain sources explicitly. */
  received<T>(type: string, data: unknown, dispatch: () => T): T {
    const sequence = this.record(type, data);
    const previous = this.#input;
    let context: unknown;
    try {
      context = redactDiagnosticData(data, 16384);
    } catch {
      context = { unavailable: "source context exceeds diagnostic limits" };
    }
    this.#input = { sessionId: this.#sessionId, sequence, type, context };
    try {
      return dispatch();
    } finally {
      this.#input = previous;
    }
  }

  sources(value: object): DiagnosticContributors {
    return this.#contributors.get(value) ?? {};
  }

  annotate<T extends object>(value: T, sources: DiagnosticContributors): T {
    this.#contributors.set(value, sources);
    return value;
  }

  #pendingBytes = 0;
  #pendingRecords = 0;
  #gap:
    { firstSequence: number; lastSequence: number; count: number } | undefined;
  #closed = false;
  #exited = false;
  #closing: Promise<void> | undefined;
  #lastReport = 0;
  #hadOmissions = false;

  constructor(
    options: DiagnosticCaptureOptions,
    report = (message: string) => {
      process.stderr.write(`RoonScape capture: ${message}\n`);
    },
  ) {
    this.#report = report;
    const budgetBytes = options.budgetBytes ?? defaultCaptureBudgetBytes;
    this.#recordLimit = Math.min(256 * 1024, Math.floor(budgetBytes / 8));
    try {
      if (!Number.isSafeInteger(budgetBytes) || budgetBytes < 4096)
        throw new Error();
      this.#worker = new Worker(
        new URL("./diagnostic-capture-worker.js", import.meta.url),
        {
          workerData: { ...options, sessionId: this.#sessionId, budgetBytes },
        },
      );
      this.#worker.on("error", () =>
        this.#failure("writer failed; capture is incomplete"),
      );
      this.#worker.on("exit", () => {
        this.#exited = true;
      });
      this.#worker.on("message", (bytes: number | "failure") => {
        if (bytes === "failure") {
          this.#failure(
            "destination/write failure; capture is incomplete (retrying)",
          );
          return;
        }
        this.#pendingBytes -= bytes;
        this.#pendingRecords -= 1;
        this.#flushGap();
      });
      this.#worker.unref();
      this.record("session", { formatVersion: 2, budgetBytes });
    } catch {
      this.#failure("could not start writer; capture is unavailable");
    }
  }

  record(type: string, data: unknown): number {
    try {
      return this.#record(type, data);
    } finally {
      if (type === "snapshot")
        this.#snapshot = this.#boundedContext(
          data,
          Math.floor(this.#recordLimit / 5),
        );
    }
  }

  #record(type: string, data: unknown): number {
    if (this.#closed || this.#exited || !this.#worker) return ++this.#sequence;
    this.#flushGap();
    const sequence = ++this.#sequence;
    if (
      this.#pendingBytes >= queueLimit - 2 * this.#recordLimit - 1024 ||
      this.#pendingRecords >= 1024
    ) {
      this.#omit(sequence);
      return sequence;
    }
    try {
      const safeData = redactDiagnosticData(data, this.#recordLimit);
      const line = this.#line(sequence, type, safeData);
      if (Buffer.byteLength(line) > this.#recordLimit) throw new Error();
      this.#send(line);
    } catch {
      this.#omit(sequence);
    }
    return sequence;
  }

  /** A hard deadline also covers a blocked filesystem or worker startup. */
  close(): Promise<void> {
    this.#closing ??= this.#close();
    return this.#closing;
  }

  async #close(): Promise<void> {
    this.#closed = true;
    const worker = this.#worker;
    if (!worker || this.#exited) return;
    worker.ref();
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        this.#failure("shutdown flush timed out; capture is incomplete");
        void worker.terminate().catch(() => undefined);
        worker.unref();
        resolve();
      }, 1000);
      worker.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
      // Gap metadata has reserved queue space and follows all admitted records.
      this.#flushGap(true);
      try {
        worker.postMessage(null);
      } catch {
        this.#failure("shutdown flush failed; capture is incomplete");
        clearTimeout(timer);
        void worker.terminate().catch(() => undefined);
        worker.unref();
        resolve();
      }
    });
  }

  #line(sequence: number, type: string, data: unknown): string {
    return (
      JSON.stringify({
        sessionId: this.#sessionId,
        sequence,
        timestamp: new Date().toISOString(),
        type,
        data,
      }) + "\n"
    );
  }
  #send(line: string, minimalCheckpoint = false): void {
    try {
      const checkpoint = minimalCheckpoint
        ? JSON.stringify({
            kind: "retained state",
            unavailable:
              "shutdown gap: checkpoint omitted to preserve queue bound",
            earlierRecordingGap: true,
          })
        : this.#checkpoint();
      this.#worker!.postMessage({ line, checkpoint });
      this.#pendingBytes +=
        Buffer.byteLength(line) + Buffer.byteLength(checkpoint);
      this.#pendingRecords += 1;
    } catch {
      this.#exited = true;
      this.#failure("writer communication failed; capture is incomplete");
    }
  }
  #omit(sequence: number): void {
    this.#hadOmissions = true;
    this.#gap ??= { firstSequence: sequence, lastSequence: sequence, count: 0 };
    this.#gap.lastSequence = sequence;
    this.#gap.count += 1;
    this.#failure(
      "records omitted (serialization, size, or write pressure); capture is incomplete",
    );
  }
  #flushGap(force = false): void {
    if (
      !this.#gap ||
      this.#exited ||
      (!force &&
        (this.#closed ||
          this.#pendingBytes >= queueLimit - 2 * this.#recordLimit - 1024 ||
          this.#pendingRecords >= 1024))
    )
      return;
    const gap = this.#gap;
    this.#gap = undefined;
    this.#send(this.#line(++this.#sequence, "gap", gap), force);
  }
  #failure(message: string): void {
    if (Date.now() - this.#lastReport < 1000) return;
    this.#lastReport = Date.now();
    try {
      this.#report(message);
    } catch {
      /* Diagnostics never control playback. */
    }
  }
}
