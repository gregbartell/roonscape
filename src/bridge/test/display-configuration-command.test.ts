import assert from "node:assert/strict";
import test from "node:test";

import {
  runDisplayConfigurationCommand,
  type DiscoverableTrackedOutput,
} from "../src/display-configuration-command.js";
import type {
  DisplayConfiguration,
  DisplayConfigurationStore,
} from "../src/display-configuration.js";

test("lists discoverable Tracked Outputs with the IDs needed for host selection", async () => {
  const lines: string[] = [];
  const outputs: DiscoverableTrackedOutput[] = [
    {
      trackedOutputId: "output-speaker-system",
      trackedOutputName: "Speaker System",
      trackedZoneName: "Living Room",
    },
    {
      trackedOutputId: "output-kitchen",
      trackedOutputName: "Kitchen Speaker",
      trackedZoneName: "Kitchen",
    },
  ];

  const exitCode = await runDisplayConfigurationCommand(["list"], {
    configurationStore: unusedConfigurationStore(),
    discoverTrackedOutputs: async () => outputs,
    writeLine: (line) => lines.push(line),
  });

  assert.equal(exitCode, 0);
  assert.deepEqual(lines, [
    "TRACKED OUTPUT ID\tTRACKED OUTPUT\tTRACKED ZONE",
    "output-speaker-system\tSpeaker System\tLiving Room",
    "output-kitchen\tKitchen Speaker\tKitchen",
  ]);
});

test("saves the selected Tracked Output without changing Roon", async () => {
  let saved: DisplayConfiguration | undefined;
  const lines: string[] = [];
  const configurationStore: DisplayConfigurationStore = {
    load: () => null,
    save: (configuration) => {
      saved = configuration;
    },
  };

  const exitCode = await runDisplayConfigurationCommand(
    ["select", "output-speaker-system"],
    {
      configurationStore,
      discoverTrackedOutputs: async () => {
        throw new Error("select must not contact or control Roon");
      },
      writeLine: (line) => lines.push(line),
    },
  );

  assert.equal(exitCode, 0);
  assert.deepEqual(saved, { trackedOutputId: "output-speaker-system" });
  assert.deepEqual(lines, ["Selected Tracked Output: output-speaker-system"]);
});

test("preserves inactivity calibration when changing the Tracked Output", async () => {
  const inactivity = {
    gracePeriodSeconds: 240,
    dimmedOpacity: 0.3,
    repositionCadenceSeconds: 45,
  };
  let saved: DisplayConfiguration | undefined;

  const exitCode = await runDisplayConfigurationCommand(
    ["select", "output-library"],
    {
      configurationStore: {
        load: () => ({ trackedOutputId: "output-speaker-system", inactivity }),
        save: (configuration) => {
          saved = configuration;
        },
      },
      discoverTrackedOutputs: async () => [],
      writeLine: () => {},
    },
  );

  assert.equal(exitCode, 0);
  assert.deepEqual(saved, {
    trackedOutputId: "output-library",
    inactivity,
  });
});

test("configures OLED inactivity without changing the Tracked Output", async () => {
  let saved: DisplayConfiguration | undefined;
  const lines: string[] = [];

  const exitCode = await runDisplayConfigurationCommand(
    ["inactivity", "240", "0.3", "45"],
    {
      configurationStore: {
        load: () => ({ trackedOutputId: "output-speaker-system" }),
        save: (configuration) => {
          saved = configuration;
        },
      },
      discoverTrackedOutputs: async () => [],
      writeLine: (line) => lines.push(line),
    },
  );

  assert.equal(exitCode, 0);
  assert.deepEqual(saved, {
    trackedOutputId: "output-speaker-system",
    inactivity: {
      gracePeriodSeconds: 240,
      dimmedOpacity: 0.3,
      repositionCadenceSeconds: 45,
    },
  });
  assert.deepEqual(lines, [
    "OLED inactivity: grace 240s, opacity 0.3, reposition every 45s",
  ]);
});

function unusedConfigurationStore(): DisplayConfigurationStore {
  return {
    load: () => null,
    save: () => {
      throw new Error("list must not change Display Configuration");
    },
  };
}
