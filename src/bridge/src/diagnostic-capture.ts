import { randomUUID } from "node:crypto";
import { Worker } from "node:worker_threads";

import { redactDiagnosticData } from "./diagnostic-redaction.js";

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
  #pendingBytes = 0;
  #pendingRecords = 0;
  #gap:
    { firstSequence: number; lastSequence: number; count: number } | undefined;
  #closed = false;
  #exited = false;
  #closing: Promise<void> | undefined;
  #lastReport = 0;

  constructor(
    options: DiagnosticCaptureOptions,
    report = (message: string) => {
      process.stderr.write(`RoonScape capture: ${message}\n`);
    },
  ) {
    this.#report = report;
    const budgetBytes = options.budgetBytes ?? defaultCaptureBudgetBytes;
    this.#recordLimit = Math.min(256 * 1024, Math.floor(budgetBytes / 4));
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
      this.record("session", { formatVersion: 1, budgetBytes });
    } catch {
      this.#failure("could not start writer; capture is unavailable");
    }
  }

  record(type: string, data: unknown): void {
    if (this.#closed || this.#exited || !this.#worker) return;
    this.#flushGap();
    const sequence = ++this.#sequence;
    if (
      this.#pendingBytes >= queueLimit - this.#recordLimit ||
      this.#pendingRecords >= 1024
    ) {
      this.#omit(sequence);
      return;
    }
    try {
      const safeData = redactDiagnosticData(data, this.#recordLimit);
      const line = this.#line(sequence, type, safeData);
      if (Buffer.byteLength(line) > this.#recordLimit) throw new Error();
      this.#send(line);
    } catch {
      this.#omit(sequence);
    }
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
  #send(line: string): void {
    try {
      this.#worker!.postMessage(line);
      this.#pendingBytes += Buffer.byteLength(line);
      this.#pendingRecords += 1;
    } catch {
      this.#exited = true;
      this.#failure("writer communication failed; capture is incomplete");
    }
  }
  #omit(sequence: number): void {
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
          this.#pendingBytes >= queueLimit - this.#recordLimit ||
          this.#pendingRecords >= 1024))
    )
      return;
    const gap = this.#gap;
    this.#gap = undefined;
    this.#send(this.#line(++this.#sequence, "gap", gap));
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
