import assert from "node:assert/strict";
import { stat, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import test from "node:test";

import {
  FileDisplayConfigurationStore,
  displayConfigurationFilePath,
} from "../src/display-configuration.js";
import { withTaskDirectory } from "./support.js";

const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));

test("persists Display Configuration in a private dedicated file", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const configDirectory = path.join(taskDirectory, "config");
    const configurationFile = path.join(configDirectory, "display.json");
    const store = new FileDisplayConfigurationStore(configurationFile);
    const configuration = { displayOutputId: "output-gallery" };

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
  });
});

test("loads existing Display Configuration without inactivity calibration", async () => {
  const store = new FileDisplayConfigurationStore(
    path.join(
      repositoryRoot,
      "fixtures/display-configuration-output-only.json",
    ),
  );

  assert.deepEqual(store.load(), { displayOutputId: "output-gallery" });
});

test("persists inactivity calibration with Display Output selection", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const configurationFile = path.join(taskDirectory, "display.json");
    const store = new FileDisplayConfigurationStore(configurationFile);
    const configuration = {
      displayOutputId: "output-gallery",
      inactivity: {
        gracePeriodSeconds: 240,
        dimmedOpacity: 0.3,
        repositionCadenceSeconds: 45,
      },
    };

    store.save(configuration);

    assert.deepEqual(store.load(), configuration);
  });
});

test("loads shared inactivity Display Configuration", () => {
  const store = new FileDisplayConfigurationStore(
    path.join(repositoryRoot, "fixtures/display-configuration-inactivity.json"),
  );

  assert.deepEqual(store.load(), {
    displayOutputId: "output-gallery",
    inactivity: {
      gracePeriodSeconds: 240,
      dimmedOpacity: 0.3,
      repositionCadenceSeconds: 45,
    },
  });
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
  await withTaskDirectory(async (taskDirectory) => {
    const configurationFile = path.join(taskDirectory, "display.json");
    const store = new FileDisplayConfigurationStore(configurationFile);

    for (const contents of [
      "not JSON",
      JSON.stringify({ displayOutputId: "" }),
      JSON.stringify({ displayOutputId: "output-1", fallback: "output-2" }),
      JSON.stringify({
        displayOutputId: "output-1",
        inactivity: {
          gracePeriodSeconds: 0,
          dimmedOpacity: 0.3,
          repositionCadenceSeconds: 45,
        },
      }),
      JSON.stringify({
        displayOutputId: "output-1",
        inactivity: {
          gracePeriodSeconds: 240,
          dimmedOpacity: 1,
          repositionCadenceSeconds: 45,
        },
      }),
      JSON.stringify({
        displayOutputId: "output-1",
        inactivity: {
          gracePeriodSeconds: 240,
          dimmedOpacity: 1.1,
          repositionCadenceSeconds: 45,
        },
      }),
    ]) {
      await writeFile(configurationFile, contents);
      assert.equal(store.load(), null);
    }
  });
});
