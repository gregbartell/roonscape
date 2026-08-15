import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
} from "../src/display-configuration.js";

test("persists Display Configuration in a private dedicated file", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const configDirectory = path.join(taskDirectory, "config");
  const configurationFile = path.join(configDirectory, "display.json");
  const store = new FileDisplayConfigurationStore(configurationFile);
  const configuration = { displayOutputId: "output-gallery" };

  try {
    assert.equal(store.load(), null);
    store.save(configuration);

    assert.deepEqual(
      {
        configuration: store.load(),
        directoryMode: (await stat(configDirectory)).mode & 0o777,
        fileMode: (await stat(configurationFile)).mode & 0o777,
      },
      {
        configuration,
        directoryMode: 0o700,
        fileMode: 0o600,
      },
    );
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
});

test("uses a Display Configuration path separate from Roon authorization", () => {
  assert.equal(
    displayConfigurationFilePath({
      HOME: "/home/owner",
      XDG_CONFIG_HOME: "/var/config",
      XDG_STATE_HOME: "/var/state",
    }),
    "/var/config/roonscape/display.json",
  );
});

test("treats malformed or invalid Display Configuration as unavailable", async () => {
  const scratchRoot = "/tmp/codex/roonscape";
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  const configurationFile = path.join(taskDirectory, "display.json");
  const store = new FileDisplayConfigurationStore(configurationFile);

  try {
    for (const contents of [
      "not JSON",
      JSON.stringify({ displayOutputId: "" }),
      JSON.stringify({ displayOutputId: "output-1", fallback: "output-2" }),
    ]) {
      await writeFile(configurationFile, contents);
      assert.equal(store.load(), null);
    }
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
});
