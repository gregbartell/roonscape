import assert from "node:assert/strict";
import {
  readFile,
  readdir,
  stat,
  writeFile,
  rename,
  unlink,
} from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";

import { DiagnosticCapture } from "../src/diagnostic-capture.js";
import { withTaskDirectory } from "./support.js";

test("retains ordered private JSONL with nested credentials redacted", async () => {
  await withTaskDirectory(async (directory) => {
    const capture = new DiagnosticCapture({ directory });
    capture.record("inbound", {
      connectionId: "ordinary-1",
      body: {
        token: "synthetic-registration",
        nested: [
          {
            Authorization: "Bearer synthetic-auth",
            password: "synthetic-password",
            image_key: "retained-image",
          },
        ],
      },
    });
    capture.record("snapshot", { revision: 1 });
    await capture.close();
    const files = (await readdir(directory)).filter((name) =>
      name.endsWith(".jsonl"),
    );
    assert.ok(files.length > 0);
    const contents = await readCaptureText(directory);
    assert.doesNotMatch(contents, /synthetic-/);
    const records = contents
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.deepEqual(
      records.map((record) => record.type),
      ["checkpoint", "session", "inbound", "snapshot"],
    );
    assert.deepEqual(
      records.map((record) => record.sequence),
      [1, 1, 2, 3],
    );
    assert.equal(records[2].data.body.nested[0].image_key, "retained-image");
    assert.equal(new Set(records.map((record) => record.sessionId)).size, 1);
    assert.ok(
      records.every((record) => Number.isFinite(Date.parse(record.timestamp))),
    );
    assert.equal(
      (await stat(path.join(directory, files[0]!))).mode & 0o777,
      0o600,
    );
  });
});

test("retention includes previous launches and never removes unrelated files", async () => {
  await withTaskDirectory(async (directory) => {
    await writeFile(path.join(directory, "operator-notes.jsonl"), "keep me");
    for (let launch = 0; launch < 3; launch += 1) {
      const capture = new DiagnosticCapture({ directory, budgetBytes: 8192 });
      for (let index = 0; index < 20; index += 1) {
        capture.record("inbound", { index, text: "x".repeat(200) });
      }
      await capture.close();
      const files = (await readdir(directory)).filter((name) =>
        name.startsWith("bridge-capture-"),
      );
      const sizes = await Promise.all(
        files.map(
          async (name) => (await stat(path.join(directory, name))).size,
        ),
      );
      assert.ok(sizes.reduce((sum, size) => sum + size, 0) <= 8192);
      const records = (await readCaptureText(directory))
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      assert.equal(records.at(-1).data.index, 19);
    }
    assert.equal(
      await readFile(path.join(directory, "operator-notes.jsonl"), "utf8"),
      "keep me",
    );
  });
});

test("serialization omissions and pressure produce bounded, readable gaps", async () => {
  await withTaskDirectory(async (directory) => {
    const capture = new DiagnosticCapture({ directory });
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    assert.doesNotThrow(() => capture.record("inbound", cyclic));
    capture.record("inbound", { text: "x".repeat(2 * 1024 * 1024) });
    for (let index = 0; index < 4000; index += 1)
      capture.record("inbound", { index, text: "x".repeat(2000) });
    await capture.close();
    const contents = await readCaptureText(directory);
    assert.ok(Buffer.byteLength(contents) < 2 * 1024 * 1024);
    const records = contents
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.ok(records.some((record) => record.type === "gap"));
    assert.ok(
      records
        .filter((record) => record.type === "gap")
        .reduce((sum, record) => sum + record.data.count, 0) > 0,
    );
    assert.deepEqual(
      records.map((record) => record.sequence),
      records.map((record) => record.sequence).sort((a, b) => a - b),
    );
  });
});

test("a recovered destination records a write gap and resumes without leaking errors", async () => {
  await withTaskDirectory(async (directory) => {
    const destination = path.join(directory, "capture");
    const moved = path.join(directory, "moved");
    const logs: string[] = [];
    const capture = new DiagnosticCapture(
      { directory: destination, budgetBytes: 32768 },
      (line) => logs.push(line),
    );
    let observed = {
      status: "observed",
      zones: [] as Array<{ zone_id: string }>,
    };
    capture.setCheckpointProvider(() => ({ roonState: observed }));
    try {
      for (let attempt = 0; attempt < 100; attempt += 1) {
        if (
          (await readdir(destination).catch(() => [])).some((name) =>
            name.endsWith(".jsonl"),
          )
        )
          break;
        await delay(10);
      }
      await rename(destination, moved);
      await writeFile(destination, "synthetic-secret");
      for (let index = 0; index < 80; index += 1)
        capture.record("inbound", { index, text: "x".repeat(200) });
      observed = { status: "observed", zones: [{ zone_id: "during-gap" }] };
      capture.record("snapshot", {
        snapshot: { revision: 8 },
        contributors: {},
      });
      await delay(100);
      await unlink(destination);
      await rename(moved, destination);
      await delay(1100);
      capture.record("inbound", { marker: "after-recovery" });
      capture.record("inbound", { marker: "resumed" });
    } finally {
      await capture.close();
    }
    const contents = await readCaptureText(destination);
    const records = contents
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.ok(
      records.some(
        (record) =>
          record.type === "gap" && record.data.reason === "write failure",
      ),
    );
    assert.ok(records.some((record) => record.data.marker === "resumed"));
    const recovered = records.find(
      (record) => record.type === "checkpoint" && record.data.recordingGap,
    );
    assert.deepEqual(recovered.data.roonState, observed);
    assert.equal(recovered.data.snapshot.snapshot.revision, 8);
    assert.ok(logs.length > 0);
    assert.doesNotMatch(logs.join("\n"), /synthetic-secret|ENOTDIR|moved/);
  });
});

test("redacts credential-bearing fields and URLs while preserving non-artwork binary content", async () => {
  await withTaskDirectory(async (directory) => {
    const capture = new DiagnosticCapture({ directory });
    capture.record("inbound", {
      headers: {
        "X-API-Key": "synthetic-api-key",
        Authentication: "synthetic-authentication",
      },
      nested: {
        access_key: "synthetic-access-key",
        pwd: "synthetic-pwd",
        url: "https://user:synthetic-url-password@example.test/path?access_token=synthetic-query-token&image_key=keep-key",
      },
      body: Buffer.from([0, 255, 128, 65]),
    });
    await capture.close();
    const contents = await readCaptureText(directory);
    assert.doesNotMatch(contents, /synthetic-/);
    const inbound = contents
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line))
      .find((record) => record.type === "inbound");
    assert.deepEqual(
      Buffer.from(inbound.data.body.data, "base64"),
      Buffer.from([0, 255, 128, 65]),
    );
    assert.match(inbound.data.nested.url, /image_key=keep-key/);
  });
});

async function readCaptureText(directory: string): Promise<string> {
  const files = (await readdir(directory))
    .filter(
      (name) => name.startsWith("bridge-capture-") && name.endsWith(".jsonl"),
    )
    .sort();
  return (
    await Promise.all(
      files.map((name) => readFile(path.join(directory, name), "utf8")),
    )
  ).join("");
}

test("capture failure and a throwing diagnostic logger keep shutdown bounded", async () => {
  await withTaskDirectory(async (directory) => {
    const destination = path.join(directory, "not-a-directory");
    await writeFile(destination, "preserve");
    const capture = new DiagnosticCapture({ directory: destination }, () => {
      throw new Error("synthetic-logger-secret");
    });
    for (let index = 0; index < 4000; index += 1)
      capture.record("inbound", { index, text: "x".repeat(2000) });
    const started = performance.now();
    await capture.close();
    await capture.close();
    assert.ok(performance.now() - started < 1500);
    assert.equal(await readFile(destination, "utf8"), "preserve");
  });
});

test("every rotated file begins with retained state and source context", async () => {
  await withTaskDirectory(async (directory) => {
    const capture = new DiagnosticCapture({ directory, budgetBytes: 32768 });
    capture.record("snapshot", { snapshot: { revision: 1 }, contributors: {} });
    for (let index = 0; index < 100; index += 1) {
      capture.record("inbound", { index, text: "x".repeat(200) });
      await delay(2);
    }
    await capture.close();
    const files = (await readdir(directory))
      .filter((name) => name.endsWith(".jsonl"))
      .sort();
    assert.ok(files.length > 1);
    assert.ok(!files[0]!.endsWith("0000000000.jsonl"));
    for (const file of files) {
      const first = JSON.parse(
        (await readFile(path.join(directory, file), "utf8")).split("\n")[0]!,
      );
      assert.equal(first.type, "checkpoint");
      assert.equal(first.data.kind, "retained state");
      assert.deepEqual(first.data.snapshot.snapshot, { revision: 1 });
    }
  });
});
