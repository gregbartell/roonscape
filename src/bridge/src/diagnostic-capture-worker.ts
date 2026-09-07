import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  ftruncateSync,
  lstatSync,
  mkdirSync,
  openSync,
  readdirSync,
  unlinkSync,
  writeSync,
} from "node:fs";
import path from "node:path";
import { parentPort, workerData } from "node:worker_threads";

const { directory, sessionId, budgetBytes } = workerData as {
  directory: string;
  sessionId: string;
  budgetBytes: number;
};
const segmentLimit = Math.min(1024 * 1024, Math.floor(budgetBytes / 4));
const ownedName = /^bridge-capture-\d{13}-[0-9a-f-]{36}-\d{10}\.jsonl$/;
const prefix = `bridge-capture-${Date.now()}-${sessionId}`;
const retained: Array<{ file: string; size: number }> = [];
let total = 0;
let segment = 0;
let descriptor: number | undefined;
let current: { file: string; size: number } | undefined;
let lock: number | undefined;
let checkpoint = "";
const lockFile = path.join(directory, ".bridge-capture.lock");

function initialize(): void {
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  const directoryDescriptor = openSync(
    directory,
    constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW,
  );
  try {
    fchmodSync(directoryDescriptor, 0o700);
  } finally {
    closeSync(directoryDescriptor);
  }
  lock ??= openSync(
    lockFile,
    constants.O_WRONLY |
      constants.O_CREAT |
      constants.O_EXCL |
      constants.O_NOFOLLOW,
    0o600,
  );
  retained.length = 0;
  total = 0;
  for (const name of readdirSync(directory).sort()) {
    if (!ownedName.test(name)) continue;
    const file = path.join(directory, name);
    const info = lstatSync(file);
    if (!info.isFile()) continue;
    retained.push({ file, size: info.size });
    total += info.size;
  }
  prune(0);
}

function prune(additional: number): void {
  while (total + additional > budgetBytes && retained.length > 0) {
    const oldest = retained[0]!;
    if (oldest === current) finishSegment();
    unlinkSync(oldest.file);
    total -= oldest.size;
    retained.shift();
  }
}

function finishSegment(): void {
  if (descriptor !== undefined) closeSync(descriptor);
  descriptor = undefined;
  current = undefined;
}

function append(line: string): void {
  const bytes = Buffer.byteLength(line);
  if (bytes > segmentLimit) throw new Error("oversized");
  if (current && current.size + bytes > segmentLimit) finishSegment();
  if (!current && bytes + Buffer.byteLength(checkpoint) > segmentLimit) {
    const envelope = JSON.parse(checkpoint) as Envelope;
    checkpoint =
      JSON.stringify({
        ...envelope,
        data: {
          kind: "retained state",
          unavailable: "checkpoint exceeds segment limit",
        },
      }) + "\n";
  }
  prune(bytes + (current ? 0 : Buffer.byteLength(checkpoint)));
  if (!current) {
    const file = path.join(
      directory,
      `${prefix}-${String(segment++).padStart(10, "0")}.jsonl`,
    );
    descriptor = openSync(
      file,
      constants.O_WRONLY |
        constants.O_CREAT |
        constants.O_EXCL |
        constants.O_NOFOLLOW,
      0o600,
    );
    current = { file, size: 0 };
    retained.push(current);
    writeLine(checkpoint);
  }
  writeLine(line);
}

function writeLine(line: string): void {
  const bytes = Buffer.byteLength(line);
  if (!current) throw new Error("no segment");
  const before = current.size;
  try {
    const buffer = Buffer.from(line);
    let offset = 0;
    while (offset < buffer.length) {
      const written = writeSync(descriptor!, buffer, offset);
      if (written === 0) throw new Error("short write");
      offset += written;
    }
    current.size = fstatSync(descriptor!).size;
    total += bytes;
  } catch {
    try {
      ftruncateSync(descriptor!, before);
    } catch {
      // Account for a partial append even when it cannot be repaired.
      const actual = fstatSync(descriptor!).size;
      total += actual - before;
      current.size = actual;
    }
    finishSegment();
    throw new Error("write failure");
  }
}

interface Envelope {
  sessionId: string;
  sequence: number;
  timestamp: string;
  type: string;
  data: unknown;
}
let initialized = false;
let retryAt = 0;
let gap:
  | {
      firstSequence: number;
      lastSequence: number;
      count: number;
      reason: string;
    }
  | undefined;
let lastEnvelope: Envelope | undefined;
let hadWriteGap = false;

function tryAppend(line: string, force = false): boolean {
  if (!force && Date.now() < retryAt) return false;
  try {
    if (!initialized) {
      initialize();
      initialized = true;
    }
    append(line);
    retryAt = 0;
    return true;
  } catch {
    retryAt = Date.now() + 1000;
    parentPort!.postMessage("failure");
    return false;
  }
}

function gapLine(envelope: Envelope): string {
  return (
    JSON.stringify({
      ...envelope,
      timestamp: new Date().toISOString(),
      type: "gap",
      data: gap,
    }) + "\n"
  );
}

parentPort!.on(
  "message",
  (record: { line: string; checkpoint: string } | null) => {
    if (record === null) {
      if (gap && lastEnvelope) tryAppend(gapLine(lastEnvelope), true);
      try {
        finishSegment();
      } catch {
        parentPort!.postMessage("failure");
      }
      try {
        if (lock !== undefined) {
          closeSync(lock);
          unlinkSync(lockFile);
        }
      } catch {
        parentPort!.postMessage("failure");
      }
      parentPort!.close();
      return;
    }
    const { line } = record;
    const envelope = JSON.parse(line) as Envelope;
    checkpoint =
      JSON.stringify({
        ...envelope,
        type: "checkpoint",
        data: {
          ...JSON.parse(record.checkpoint),
          beforeSequence: envelope.sequence,
          earlierWriteGap: hadWriteGap,
          ...(gap ? { recordingGap: gap } : {}),
        },
      }) + "\n";
    lastEnvelope = envelope;
    if (envelope.type === "gap") finishSegment();
    if (gap) {
      // On recovery, replace this attempted record with an explicit gap. This
      // preserves the parent's sequence order without inventing causal links.
      gap.lastSequence = envelope.sequence;
      gap.count += 1;
      finishSegment();
      if (tryAppend(gapLine(envelope))) gap = undefined;
    } else if (!tryAppend(line)) {
      hadWriteGap = true;
      gap = {
        firstSequence: envelope.sequence,
        lastSequence: envelope.sequence,
        count: 1,
        reason: "write failure",
      };
    }
    parentPort!.postMessage(
      Buffer.byteLength(line) + Buffer.byteLength(record.checkpoint),
    );
  },
);
