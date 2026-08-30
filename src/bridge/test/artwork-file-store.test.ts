import assert from "node:assert/strict";
import { mkdtemp, readFile, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { ArtworkFileStore } from "../src/artwork-file-store.js";

test("keeps only the current artwork and one atomically staged replacement", async () => {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-artwork-store-test."),
  );
  const artworkDirectory = path.join(taskDirectory, "artwork");
  const store = await ArtworkFileStore.open(artworkDirectory);

  try {
    const first = await store.stage(3, Buffer.from("first jpeg"));
    await store.commit(first);

    assert.match(path.basename(first.path), /^artwork-3-.+\.jpg$/);
    assert.deepEqual(await readdir(artworkDirectory), [
      path.basename(first.path),
    ]);
    assert.equal(await readFile(first.path, "utf8"), "first jpeg");
    assert.equal((await stat(first.path)).mode & 0o777, 0o600);

    const replacement = await store.stage(5, Buffer.from("second jpeg"));
    assert.match(path.basename(replacement.path), /^artwork-5-.+\.jpg$/);
    assert.deepEqual(
      (await readdir(artworkDirectory)).sort(),
      [path.basename(first.path), path.basename(replacement.path)].sort(),
    );

    await store.commit(replacement);

    assert.deepEqual(await readdir(artworkDirectory), [
      path.basename(replacement.path),
    ]);
    assert.equal(await readFile(replacement.path, "utf8"), "second jpeg");

    await store.clear();
    assert.deepEqual(await readdir(artworkDirectory), []);
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
});
