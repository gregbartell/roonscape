import assert from "node:assert/strict";
import test from "node:test";

import {
  runDisplayConfigurationCommand,
  type DiscoverableDisplayOutput,
} from "../src/display-configuration-command.js";
import type {
  DisplayConfiguration,
  DisplayConfigurationStore,
} from "../src/display-configuration.js";

test("lists discoverable outputs with the IDs needed for host selection", async () => {
  const lines: string[] = [];
  const outputs: DiscoverableDisplayOutput[] = [
    {
      outputId: "output-gallery",
      displayName: "NUC HDMI",
      displayZoneName: "Gallery",
    },
    {
      outputId: "output-kitchen",
      displayName: "Kitchen Speaker",
      displayZoneName: "Kitchen",
    },
  ];

  const exitCode = await runDisplayConfigurationCommand(["list"], {
    configurationStore: unusedConfigurationStore(),
    discoverOutputs: async () => outputs,
    writeLine: (line) => lines.push(line),
  });

  assert.equal(exitCode, 0);
  assert.deepEqual(lines, [
    "OUTPUT ID\tDISPLAY OUTPUT\tDISPLAY ZONE",
    "output-gallery\tNUC HDMI\tGallery",
    "output-kitchen\tKitchen Speaker\tKitchen",
  ]);
});

test("saves the selected Display Output without changing Roon", async () => {
  let saved: DisplayConfiguration | undefined;
  const lines: string[] = [];
  const configurationStore: DisplayConfigurationStore = {
    load: () => null,
    save: (configuration) => {
      saved = configuration;
    },
  };

  const exitCode = await runDisplayConfigurationCommand(
    ["select", "output-gallery"],
    {
      configurationStore,
      discoverOutputs: async () => {
        throw new Error("select must not contact or control Roon");
      },
      writeLine: (line) => lines.push(line),
    },
  );

  assert.equal(exitCode, 0);
  assert.deepEqual(saved, { displayOutputId: "output-gallery" });
  assert.deepEqual(lines, ["Selected Display Output: output-gallery"]);
});

function unusedConfigurationStore(): DisplayConfigurationStore {
  return {
    load: () => null,
    save: () => {
      throw new Error("list must not change Display Configuration");
    },
  };
}
