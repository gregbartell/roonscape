import assert from "node:assert/strict";
import {
  access,
  chmod,
  mkdir,
  readFile,
  readdir,
  stat,
  writeFile,
} from "node:fs/promises";
import { statSync } from "node:fs";
import path from "node:path";
import test from "node:test";

import {
  type ChildResult,
  type RoonScapeCommandDependencies,
  type RunningChild,
  runRoonScapeCommand,
} from "../src/roonscape-command.js";
import { FileDisplayConfigurationStore } from "../src/display-configuration.js";
import type { SetupKey } from "../src/first-time-setup.js";
import { openRuntimeSession } from "../src/runtime-session.js";
import { withTaskDirectory } from "./support.js";

test("help describes the owner-facing command", async () => {
  const output: string[] = [];
  const dependencies = commandDependencies({
    writeOutput: (line) => output.push(line),
  });

  const result = await runRoonScapeCommand(["--help"], dependencies);

  assert.equal(result, 0);
  assert.match(
    output.join("\n"),
    /Usage: roonscape \[--setup\] \[--config PATH\]/,
  );
  assert.match(output.join("\n"), /--setup/);
  assert.match(output.join("\n"), /--help/);
  assert.match(output.join("\n"), /--version/);
});

test("version identifies the RoonScape build", async () => {
  const output: string[] = [];
  const dependencies = commandDependencies({
    writeOutput: (line) => output.push(line),
  });

  const result = await runRoonScapeCommand(["--version"], dependencies);

  assert.equal(result, 0);
  assert.deepEqual(output, ["RoonScape 0.1.0-test"]);
});

test("unsupported arguments fail with actionable usage", async () => {
  const errors: string[] = [];
  const dependencies = commandDependencies({
    writeError: (line) => errors.push(line),
  });

  const result = await runRoonScapeCommand(["--unknown"], dependencies);

  assert.equal(result, 2);
  assert.match(errors.join("\n"), /Unknown option: --unknown/);
  assert.match(errors.join("\n"), /Usage: roonscape/);
});

test("configured start launches bridge then renderer as one session", async () => {
  const events: string[] = [];
  const bridge = pendingChild((signal) => events.push(`bridge:${signal}`));
  const renderer = completedChild({ exitCode: 0, signal: null });
  const dependencies = commandDependencies({
    loadConfiguration: (configurationFile) => {
      events.push(`configuration:${configurationFile}`);
      return { trackedOutputId: "output-gallery" };
    },
    openRuntime: async () => ({
      socketPath: "/runtime/roonscape/roonscape.sock",
      cleanup: async () => {
        events.push("runtime:cleanup");
      },
    }),
    launchBridge: (options) => {
      events.push(`bridge:start:${JSON.stringify(options)}`);
      return bridge;
    },
    launchRenderer: (options) => {
      events.push(`renderer:start:${JSON.stringify(options)}`);
      return renderer;
    },
  });

  const result = await runRoonScapeCommand([], dependencies);

  assert.equal(result, 0);
  assert.deepEqual(events, [
    "configuration:/config/roonscape/display.json",
    'bridge:start:{"authorizationFile":"/state/roonscape/authorization.json","configurationFile":"/config/roonscape/display.json","socketPath":"/runtime/roonscape/roonscape.sock"}',
    'renderer:start:{"configurationFile":"/config/roonscape/display.json","socketPath":"/runtime/roonscape/roonscape.sock"}',
    "bridge:SIGTERM",
    "runtime:cleanup",
  ]);
});

test("rejects the removed Display Configuration environment override", async () => {
  const errors: string[] = [];
  const dependencies = commandDependencies({
    environment: {
      ROONSCAPE_DISPLAY_CONFIG: "/legacy/display.json",
    },
    writeError: (line) => errors.push(line),
  });

  const result = await runRoonScapeCommand([], dependencies);

  assert.equal(result, 2);
  assert.match(
    errors.join("\n"),
    /ROONSCAPE_DISPLAY_CONFIG is no longer supported; use roonscape --config PATH/,
  );
});

test("--config takes precedence over the standard XDG path", async () => {
  const loadedFiles: string[] = [];
  const bridge = pendingChild(() => undefined);
  const dependencies = commandDependencies({
    standardConfigurationFile: () => {
      throw new Error("the standard path should not be selected");
    },
    loadConfiguration: (configurationFile) => {
      loadedFiles.push(configurationFile);
      return { trackedOutputId: "output-gallery" };
    },
    openRuntime: async () => ({
      socketPath: "/runtime/roonscape/roonscape.sock",
      cleanup: async () => undefined,
    }),
    launchBridge: () => bridge,
    launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
  });

  const result = await runRoonScapeCommand(
    ["--config", "settings/display.json"],
    dependencies,
  );

  assert.equal(result, 0);
  assert.deepEqual(loadedFiles, ["/working/settings/display.json"]);
});

test("missing Display Configuration fails promptly without an interactive terminal", async () => {
  const errors: string[] = [];
  const dependencies = commandDependencies({
    writeError: (line) => errors.push(line),
  });

  const result = await runRoonScapeCommand([], dependencies);

  assert.equal(result, 1);
  assert.match(errors.join("\n"), /Display Configuration is missing/);
  assert.match(errors.join("\n"), /interactive terminal/);
  assert.match(errors.join("\n"), /--config PATH/);
});

test("first-time setup saves OLED defaults and continues into the presentation", async () => {
  const output: string[] = [];
  const events: string[] = [];
  let savedConfiguration:
    | Parameters<RoonScapeCommandDependencies["saveConfiguration"]>[1]
    | undefined;
  let keyRead = 0;
  const bridge = pendingChild((signal) => events.push(`bridge:${signal}`));
  const dependencies = commandDependencies({
    terminalIsInteractive: () => true,
    configurationFileExists: () => false,
    discoverTrackedOutputs: async () => [
      {
        trackedOutputId: "output-gallery",
        trackedOutputName: "NUC HDMI",
        trackedZoneName: "Gallery",
      },
    ],
    readSetupKey: (signal) => {
      keyRead += 1;
      if (keyRead === 1) {
        return abortedKeyRead(signal);
      }
      return Promise.resolve("enter");
    },
    saveConfiguration: (configurationFile, configuration) => {
      events.push(`configuration:save:${configurationFile}`);
      savedConfiguration = configuration;
    },
    openRuntime: async () => ({
      socketPath: "/runtime/roonscape/roonscape.sock",
      cleanup: async () => {
        events.push("runtime:cleanup");
      },
    }),
    launchBridge: () => {
      events.push("bridge:start");
      return bridge;
    },
    launchRenderer: () => {
      events.push("renderer:start");
      return completedChild({ exitCode: 0, signal: null });
    },
    writeOutput: (line) => output.push(line),
  });

  const result = await runRoonScapeCommand([], dependencies);

  assert.equal(result, 0);
  assert.match(output.join("\n"), /official Roon client/);
  assert.match(output.join("\n"), /Settings.*Extensions/);
  assert.match(output.join("\n"), /NUC HDMI.*Gallery/);
  assert.doesNotMatch(output.join("\n"), /output-gallery/);
  assert.match(output.join("\n"), /5 minutes/);
  assert.match(output.join("\n"), /35 percent/);
  assert.match(output.join("\n"), /1 minute/);
  assert.deepEqual(savedConfiguration, {
    trackedOutputId: "output-gallery",
    inactivity: {
      gracePeriodSeconds: 300,
      dimmedOpacity: 0.35,
      repositionCadenceSeconds: 60,
    },
  });
  assert.deepEqual(events, [
    "configuration:save:/config/roonscape/display.json",
    "bridge:start",
    "renderer:start",
    "bridge:SIGTERM",
    "runtime:cleanup",
  ]);
});

test("--setup preserves the saved choices and exits without launching", async () => {
  const output: string[] = [];
  const savedConfiguration = {
    trackedOutputId: "output-study",
    inactivity: {
      gracePeriodSeconds: 240,
      dimmedOpacity: 0.3,
      repositionCadenceSeconds: 45,
    },
  };
  let saved: typeof savedConfiguration | undefined;
  const result = await runRoonScapeCommand(
    ["--setup"],
    commandDependencies({
      terminalIsInteractive: () => true,
      loadConfiguration: () => savedConfiguration,
      configurationFileExists: () => true,
      discoverTrackedOutputs: async () => [
        {
          trackedOutputId: "output-gallery",
          trackedOutputName: "NUC HDMI",
          trackedZoneName: "Gallery",
        },
        {
          trackedOutputId: "output-study",
          trackedOutputName: "USB DAC",
          trackedZoneName: "Study",
        },
      ],
      readSetupKey: scriptedSetupKeys("enter", "enter"),
      saveConfiguration: (_configurationFile, configuration) => {
        saved = configuration as typeof savedConfiguration;
      },
      openRuntime: async () => {
        throw new Error("runtime should not be opened");
      },
      writeOutput: (line) => output.push(line),
    }),
  );

  assert.equal(result, 0);
  assert.match(output.join("\n"), /> USB DAC.*Study/);
  assert.match(output.join("\n"), /4 minutes/);
  assert.match(output.join("\n"), /30 percent/);
  assert.match(output.join("\n"), /45 seconds/);
  assert.deepEqual(saved, savedConfiguration);
});

test("--setup refuses to wait for input without an interactive terminal", async () => {
  const errors: string[] = [];
  let inputRead = false;
  const result = await runRoonScapeCommand(
    ["--setup"],
    commandDependencies({
      loadConfiguration: () => ({ trackedOutputId: "output-study" }),
      readSetupKey: async () => {
        inputRead = true;
        return "quit";
      },
      writeError: (line) => errors.push(line),
    }),
  );

  assert.equal(result, 1);
  assert.equal(inputRead, false);
  assert.match(errors.join("\n"), /requires an interactive terminal/);
});

test("--setup prefills OLED values and corrects invalid custom entries", async () => {
  const errors: string[] = [];
  const prompts: Array<{ prompt: string; initialValue: string }> = [];
  const answers = ["0", "6.5", "100", "25", "1.5", "90"];
  let savedConfiguration:
    | Parameters<RoonScapeCommandDependencies["saveConfiguration"]>[1]
    | undefined;
  const result = await runRoonScapeCommand(
    ["--setup"],
    commandDependencies({
      terminalIsInteractive: () => true,
      loadConfiguration: () => ({
        trackedOutputId: "output-study",
        inactivity: {
          gracePeriodSeconds: 240,
          dimmedOpacity: 0.3,
          repositionCadenceSeconds: 45,
        },
      }),
      configurationFileExists: () => true,
      discoverTrackedOutputs: async () => [
        {
          trackedOutputId: "output-study",
          trackedOutputName: "USB DAC",
          trackedZoneName: "Study",
        },
      ],
      readSetupKey: scriptedSetupKeys("enter", "customize"),
      readSetupValue: async (prompt, initialValue) => {
        prompts.push({ prompt, initialValue });
        return answers.shift() ?? null;
      },
      saveConfiguration: (_configurationFile, configuration) => {
        savedConfiguration = configuration;
      },
      writeError: (line) => errors.push(line),
    }),
  );

  assert.equal(result, 0);
  assert.deepEqual(prompts, [
    { prompt: "Grace period in minutes:", initialValue: "4" },
    { prompt: "Grace period in minutes:", initialValue: "4" },
    { prompt: "Dimmed opacity in percent:", initialValue: "30" },
    { prompt: "Dimmed opacity in percent:", initialValue: "30" },
    { prompt: "Reposition cadence in seconds:", initialValue: "45" },
    { prompt: "Reposition cadence in seconds:", initialValue: "45" },
  ]);
  assert.match(errors.join("\n"), /positive number of minutes/);
  assert.match(errors.join("\n"), /less than 100 percent/);
  assert.match(errors.join("\n"), /positive whole number of seconds/);
  assert.deepEqual(savedConfiguration, {
    trackedOutputId: "output-study",
    inactivity: {
      gracePeriodSeconds: 390,
      dimmedOpacity: 0.25,
      repositionCadenceSeconds: 90,
    },
  });
});

test("--setup --config changes only the Tracked Output with a private atomic replacement", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const configurationFile = path.join(taskDirectory, "settings/display.json");
    await mkdir(path.dirname(configurationFile), { recursive: true });
    await writeFile(
      configurationFile,
      '{"trackedOutputId":"output-gallery","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":0.3,"repositionCadenceSeconds":45}}\n',
      { mode: 0o644 },
    );
    const configurationStore = new FileDisplayConfigurationStore(
      configurationFile,
    );
    const result = await runRoonScapeCommand(
      ["--setup", "--config", "settings/display.json"],
      commandDependencies({
        currentDirectory: taskDirectory,
        standardConfigurationFile: () => {
          throw new Error("the standard path should not be selected");
        },
        terminalIsInteractive: () => true,
        loadConfiguration: (selectedFile) => {
          assert.equal(selectedFile, configurationFile);
          return configurationStore.load();
        },
        configurationFileExists: (selectedFile) =>
          selectedFile === configurationFile,
        discoverTrackedOutputs: async () => [
          {
            trackedOutputId: "output-gallery",
            trackedOutputName: "NUC HDMI",
            trackedZoneName: "Gallery",
          },
          {
            trackedOutputId: "output-study",
            trackedOutputName: "USB DAC",
            trackedZoneName: "Study",
          },
        ],
        readSetupKey: scriptedSetupKeys("down", "enter", "enter"),
        saveConfiguration: (selectedFile, configuration) => {
          assert.equal(selectedFile, configurationFile);
          configurationStore.save(configuration);
        },
      }),
    );

    assert.equal(result, 0);
    assert.deepEqual(configurationStore.load(), {
      trackedOutputId: "output-study",
      inactivity: {
        gracePeriodSeconds: 240,
        dimmedOpacity: 0.3,
        repositionCadenceSeconds: 45,
      },
    });
    assert.match(await readFile(configurationFile, "utf8"), /output-study/);
    assert.equal((await stat(configurationFile)).mode & 0o777, 0o600);
    assert.deepEqual(await readdir(path.dirname(configurationFile)), [
      "display.json",
    ]);
  });
});

test("cancelling reconfiguration leaves the Display Configuration byte-for-byte intact", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const configurationFile = path.join(taskDirectory, "display.json");
    const original = `{
  "trackedOutputId": "output-study",
  "inactivity": {
    "gracePeriodSeconds": 240,
    "dimmedOpacity": 0.3,
    "repositionCadenceSeconds": 45
  }
}
`;
    await writeFile(configurationFile, original, { mode: 0o600 });
    const configurationStore = new FileDisplayConfigurationStore(
      configurationFile,
    );
    const result = await runRoonScapeCommand(
      ["--setup", "--config", configurationFile],
      commandDependencies({
        terminalIsInteractive: () => true,
        loadConfiguration: () => configurationStore.load(),
        configurationFileExists: () => true,
        discoverTrackedOutputs: async () => [
          {
            trackedOutputId: "output-study",
            trackedOutputName: "USB DAC",
            trackedZoneName: "Study",
          },
        ],
        readSetupKey: scriptedSetupKeys("enter", "customize"),
        readSetupValue: async () => null,
        saveConfiguration: (_selectedFile, configuration) =>
          configurationStore.save(configuration),
      }),
    );

    assert.equal(result, 0);
    assert.equal(await readFile(configurationFile, "utf8"), original);
  });
});

test("failed reconfiguration leaves the Display Configuration byte-for-byte intact", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const configurationFile = path.join(taskDirectory, "config/display.json");
    const original =
      '{"trackedOutputId":"output-study","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":0.3,"repositionCadenceSeconds":45}}\n';
    await mkdir(path.dirname(configurationFile), { recursive: true });
    await writeFile(configurationFile, original, { mode: 0o600 });
    const configurationStore = new FileDisplayConfigurationStore(
      configurationFile,
    );
    const errors: string[] = [];
    await chmod(path.dirname(configurationFile), 0o500);

    try {
      const result = await runRoonScapeCommand(
        ["--setup", "--config", configurationFile],
        commandDependencies({
          terminalIsInteractive: () => true,
          loadConfiguration: () => configurationStore.load(),
          configurationFileExists: () => true,
          discoverTrackedOutputs: async () => [
            {
              trackedOutputId: "output-study",
              trackedOutputName: "USB DAC",
              trackedZoneName: "Study",
            },
          ],
          readSetupKey: scriptedSetupKeys("enter", "enter"),
          saveConfiguration: (_selectedFile, configuration) =>
            configurationStore.save(configuration),
          writeError: (line) => errors.push(line),
        }),
      );

      assert.equal(result, 1);
      assert.match(errors.join("\n"), /Could not complete setup/);
      assert.equal(await readFile(configurationFile, "utf8"), original);
    } finally {
      await chmod(path.dirname(configurationFile), 0o700);
    }
  });
});

test("setup atomically publishes a private Display Configuration before launch", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const configurationFile = path.join(taskDirectory, "config/display.json");
    const configurationStore = new FileDisplayConfigurationStore(
      configurationFile,
    );
    let keyRead = 0;
    let configurationAtLaunch:
      ReturnType<FileDisplayConfigurationStore["load"]> | undefined;
    const bridge = pendingChild(() => undefined);

    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        standardConfigurationFile: () => configurationFile,
        terminalIsInteractive: () => true,
        configurationFileExists: () => false,
        discoverTrackedOutputs: async () => [
          {
            trackedOutputId: "output-gallery",
            trackedOutputName: "NUC HDMI",
            trackedZoneName: "Gallery",
          },
        ],
        readSetupKey: (signal) => {
          keyRead += 1;
          return keyRead === 1
            ? abortedKeyRead(signal)
            : Promise.resolve("enter");
        },
        saveConfiguration: (_configurationFile, configuration) =>
          configurationStore.save(configuration),
        openRuntime: async () => ({
          socketPath: "/runtime/roonscape/roonscape.sock",
          cleanup: async () => undefined,
        }),
        launchBridge: () => {
          configurationAtLaunch = configurationStore.load();
          return bridge;
        },
        launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      }),
    );

    assert.equal(result, 0);
    assert.equal(configurationAtLaunch?.trackedOutputId, "output-gallery");
    assert.equal((await stat(configurationFile)).mode & 0o777, 0o600);
    assert.deepEqual(await readdir(path.dirname(configurationFile)), [
      "display.json",
    ]);
  });
});

test("authorization wait shows delayed troubleshooting and supports Retry", async () => {
  const output: string[] = [];
  const discoveryAuthorizations: string[] = [];
  let discoveryAttempt = 0;
  let keyRead = 0;
  let cancelledAttempt = false;
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      terminalIsInteractive: () => true,
      discoverTrackedOutputs: (authorizationFile, signal) => {
        discoveryAuthorizations.push(authorizationFile);
        discoveryAttempt += 1;
        if (discoveryAttempt === 1) {
          return new Promise((_, reject) => {
            signal.addEventListener(
              "abort",
              () => {
                cancelledAttempt = true;
                reject(new DOMException("cancelled", "AbortError"));
              },
              { once: true },
            );
          });
        }
        return Promise.resolve([
          {
            trackedOutputId: "output-gallery",
            trackedOutputName: "NUC HDMI",
            trackedZoneName: "Gallery",
          },
        ]);
      },
      readSetupKey: (signal) => {
        keyRead += 1;
        if (keyRead === 1 || keyRead === 3) {
          return abortedKeyRead(signal);
        }
        return Promise.resolve(keyRead === 2 ? "retry" : "quit");
      },
      delay: async () => undefined,
      writeOutput: (line) => output.push(line),
    }),
  );

  assert.equal(result, 0);
  assert.equal(cancelledAttempt, true);
  assert.deepEqual(discoveryAuthorizations, [
    "/state/roonscape/authorization.json",
    "/state/roonscape/authorization.json",
  ]);
  assert.match(output.join("\n"), /Still waiting/);
  assert.match(output.join("\n"), /same network/);
  assert.match(output.join("\n"), /Retrying Roon discovery/);
});

test("quitting authorization wait leaves Display Configuration untouched", async () => {
  let discoveryCancelled = false;
  let saved = false;
  let launched = false;
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      terminalIsInteractive: () => true,
      discoverTrackedOutputs: (_authorizationFile, signal) =>
        new Promise((_, reject) => {
          signal.addEventListener(
            "abort",
            () => {
              discoveryCancelled = true;
              reject(new DOMException("cancelled", "AbortError"));
            },
            { once: true },
          );
        }),
      readSetupKey: async () => "quit",
      delay: () => new Promise(() => undefined),
      saveConfiguration: () => {
        saved = true;
      },
      openRuntime: async () => {
        launched = true;
        throw new Error("runtime should not be opened");
      },
    }),
  );

  assert.equal(result, 0);
  assert.equal(discoveryCancelled, true);
  assert.equal(saved, false);
  assert.equal(launched, false);
});

test("arrow-key selection disambiguates identical Tracked Output choices", async () => {
  const output: string[] = [];
  let selectedTrackedOutput = "";
  let keyRead = 0;
  const bridge = pendingChild(() => undefined);
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      terminalIsInteractive: () => true,
      discoverTrackedOutputs: async () => [
        {
          trackedOutputId: "output-gallery-left",
          trackedOutputName: "USB DAC",
          trackedZoneName: "Gallery",
        },
        {
          trackedOutputId: "output-gallery-right",
          trackedOutputName: "USB DAC",
          trackedZoneName: "Gallery",
        },
      ],
      readSetupKey: (signal) => {
        keyRead += 1;
        if (keyRead === 1) {
          return abortedKeyRead(signal);
        }
        const keys = ["down", "enter", "enter"] as const;
        return Promise.resolve(keys[keyRead - 2] ?? "quit");
      },
      saveConfiguration: (_configurationFile, configuration) => {
        selectedTrackedOutput = configuration.trackedOutputId;
      },
      openRuntime: async () => ({
        socketPath: "/runtime/roonscape/roonscape.sock",
        cleanup: async () => undefined,
      }),
      launchBridge: () => bridge,
      launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      writeOutput: (line) => output.push(line),
    }),
  );

  assert.equal(result, 0);
  assert.equal(selectedTrackedOutput, "output-gallery-right");
  assert.match(output.join("\n"), /USB DAC.*Gallery.*output-gallery-left/);
  assert.match(output.join("\n"), /USB DAC.*Gallery.*output-gallery-right/);
});

test("OLED defaults require explicit acceptance before saving", async () => {
  let keyRead = 0;
  let saved = false;
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      terminalIsInteractive: () => true,
      discoverTrackedOutputs: async () => [
        {
          trackedOutputId: "output-gallery",
          trackedOutputName: "NUC HDMI",
          trackedZoneName: "Gallery",
        },
      ],
      readSetupKey: (signal) => {
        keyRead += 1;
        if (keyRead === 1) {
          return abortedKeyRead(signal);
        }
        const keys = ["enter", "down", "quit"] as const;
        return Promise.resolve(keys[keyRead - 2] ?? "quit");
      },
      saveConfiguration: () => {
        saved = true;
      },
    }),
  );

  assert.equal(result, 0);
  assert.equal(saved, false);
});

test("empty discovery offers Refresh and Quit without guessing an output", async () => {
  const output: string[] = [];
  let discoveryAttempt = 0;
  let keyRead = 0;
  let saved = false;
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      terminalIsInteractive: () => true,
      discoverTrackedOutputs: async () => {
        discoveryAttempt += 1;
        return discoveryAttempt === 1
          ? []
          : [
              {
                trackedOutputId: "output-gallery",
                trackedOutputName: "NUC HDMI",
                trackedZoneName: "Gallery",
              },
            ];
      },
      readSetupKey: (signal) => {
        keyRead += 1;
        if (keyRead === 1 || keyRead === 3) {
          return abortedKeyRead(signal);
        }
        return Promise.resolve(keyRead === 2 ? "retry" : "quit");
      },
      saveConfiguration: () => {
        saved = true;
      },
      writeOutput: (line) => output.push(line),
    }),
  );

  assert.equal(result, 0);
  assert.equal(discoveryAttempt, 2);
  assert.equal(saved, false);
  assert.match(output.join("\n"), /No Tracked Outputs/);
  assert.match(output.join("\n"), /Refresh/);
  assert.match(output.join("\n"), /Quit/);
});

test("malformed interactive Display Configuration is reported before repair is offered", async () => {
  const events: string[] = [];
  let saved = false;
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      terminalIsInteractive: () => true,
      configurationFileExists: () => true,
      readSetupKey: async () => "quit",
      writeError: (line) => events.push(`error:${line}`),
      writeOutput: (line) => events.push(`output:${line}`),
      saveConfiguration: () => {
        saved = true;
      },
    }),
  );

  assert.equal(result, 0);
  assert.equal(saved, false);
  assert.match(events[0] ?? "", /Display Configuration is invalid/);
  assert.match(events[1] ?? "", /repair/);
});

test("malformed noninteractive Display Configuration never enters setup", async () => {
  const errors: string[] = [];
  let inputRead = false;
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      configurationFileExists: () => true,
      terminalIsInteractive: () => false,
      readSetupKey: async () => {
        inputRead = true;
        return "quit";
      },
      writeError: (line) => errors.push(line),
    }),
  );

  assert.equal(result, 1);
  assert.equal(inputRead, false);
  assert.match(errors.join("\n"), /missing or invalid/);
  assert.match(errors.join("\n"), /interactive terminal/);
});

test("configured start owns private XDG runtime state and removes it on exit", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    await mkdir(runtimeRoot);
    await chmod(runtimeRoot, 0o700);
    const events: string[] = [];
    const bridge = pendingChild((signal) => events.push(`bridge:${signal}`));
    const environment = { XDG_RUNTIME_DIR: runtimeRoot };
    const dependencies = commandDependencies({
      environment,
      loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
      openRuntime: async () =>
        openRuntimeSession({
          environment,
          processId: process.pid,
          userId: process.getuid?.() ?? 1_000,
        }),
      launchBridge: ({ socketPath }) => {
        events.push(
          `runtime-mode:${(statSync(path.dirname(socketPath)).mode & 0o777).toString(8)}`,
        );
        return bridge;
      },
      launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
    });

    const result = await runRoonScapeCommand([], dependencies);

    assert.equal(result, 0);
    assert.deepEqual(events, ["runtime-mode:700", "bridge:SIGTERM"]);
    await assert.rejects(access(path.join(runtimeRoot, "roonscape")), {
      code: "ENOENT",
    });
    assert.equal((await stat(runtimeRoot)).mode & 0o777, 0o700);
  });
});

test("uses a validated per-user runtime directory when XDG_RUNTIME_DIR is absent", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "run-user-1000");
    await mkdir(runtimeRoot, { mode: 0o700 });
    const bridge = pendingChild(() => undefined);
    let selectedSocket = "";
    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        environment: {},
        loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
        openRuntime: async () =>
          openRuntimeSession({
            environment: {},
            processId: process.pid,
            userId: process.getuid?.() ?? 1_000,
            fallbackRuntimeRoot: () => runtimeRoot,
          }),
        launchBridge: ({ socketPath }) => {
          selectedSocket = socketPath;
          return bridge;
        },
        launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      }),
    );

    assert.equal(result, 0);
    assert.equal(
      selectedSocket,
      path.join(runtimeRoot, "roonscape/roonscape.sock"),
    );
  });
});

test("fails with remediation when no safe runtime directory is available", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const unavailableRoot = path.join(taskDirectory, "missing-runtime");
    const environment = { XDG_RUNTIME_DIR: unavailableRoot };
    const errors: string[] = [];
    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        environment,
        loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
        openRuntime: async () =>
          openRuntimeSession({
            environment,
            processId: process.pid,
            userId: process.getuid?.() ?? 1_000,
          }),
        writeError: (line) => errors.push(line),
      }),
    );

    assert.equal(result, 1);
    assert.match(
      errors.join("\n"),
      /set XDG_RUNTIME_DIR to a private runtime directory/,
    );
    await assert.rejects(access(unavailableRoot), { code: "ENOENT" });
  });
});

test("a live RoonScape session excludes a second invocation", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    await mkdir(runtimeRoot, { mode: 0o700 });
    const environment = { XDG_RUNTIME_DIR: runtimeRoot };
    const runtimeOptions = {
      environment,
      processId: process.pid,
      userId: process.getuid?.() ?? 1_000,
    };
    const firstSession = openRuntimeSession(runtimeOptions);
    const errors: string[] = [];

    try {
      const result = await runRoonScapeCommand(
        [],
        commandDependencies({
          environment,
          loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
          openRuntime: async () => openRuntimeSession(runtimeOptions),
          writeError: (line) => errors.push(line),
        }),
      );

      assert.equal(result, 1);
      assert.match(errors.join("\n"), /already running/);
    } finally {
      await firstSession.cleanup();
    }
  });
});

test("stale runtime artifacts are reclaimed only after their owner is gone", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    await mkdir(runtimeRoot, { mode: 0o700 });
    const environment = { XDG_RUNTIME_DIR: runtimeRoot };
    const userId = process.getuid?.() ?? 1_000;
    const staleSession = openRuntimeSession({
      environment,
      processId: 424_242,
      userId,
    });
    await writeFile(staleSession.socketPath, "stale socket artifact");
    const bridge = pendingChild(() => undefined);
    let staleArtifactWasRemoved = false;

    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        environment,
        loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
        openRuntime: async () =>
          openRuntimeSession({
            environment,
            processId: process.pid,
            userId,
            processExists: () => false,
          }),
        launchBridge: ({ socketPath }) => {
          try {
            statSync(socketPath);
          } catch (error) {
            staleArtifactWasRemoved =
              error instanceof Error &&
              "code" in error &&
              (error as NodeJS.ErrnoException).code === "ENOENT";
          }
          return bridge;
        },
        launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      }),
    );

    assert.equal(result, 0);
    assert.equal(staleArtifactWasRemoved, true);
  });
});

test("runtime artifacts without verifiable ownership are preserved", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    const runtimeDirectory = path.join(runtimeRoot, "roonscape");
    const unknownArtifact = path.join(runtimeDirectory, "roonscape.sock");
    await mkdir(runtimeDirectory, { mode: 0o700, recursive: true });
    await chmod(runtimeRoot, 0o700);
    await writeFile(unknownArtifact, "unknown owner");
    const environment = { XDG_RUNTIME_DIR: runtimeRoot };
    const errors: string[] = [];

    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        environment,
        loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
        openRuntime: async () =>
          openRuntimeSession({
            environment,
            processId: process.pid,
            userId: process.getuid?.() ?? 1_000,
          }),
        writeError: (line) => errors.push(line),
      }),
    );

    assert.equal(result, 1);
    assert.match(errors.join("\n"), /Cannot verify runtime artifacts/);
    assert.equal(await access(unknownArtifact), undefined);
  });
});

test("an ownership directory without a record fails without spinning", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    const ownershipDirectory = path.join(runtimeRoot, "roonscape/owner");
    await mkdir(ownershipDirectory, { mode: 0o700, recursive: true });
    await chmod(runtimeRoot, 0o700);
    await chmod(path.dirname(ownershipDirectory), 0o700);
    await writeFile(path.join(ownershipDirectory, "unknown"), "unverified");
    const environment = { XDG_RUNTIME_DIR: runtimeRoot };
    const errors: string[] = [];

    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        environment,
        loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
        openRuntime: async () =>
          openRuntimeSession({
            environment,
            processId: process.pid,
            userId: process.getuid?.() ?? 1_000,
          }),
        writeError: (line) => errors.push(line),
      }),
    );

    assert.equal(result, 1);
    assert.match(
      errors.join("\n"),
      /Cannot verify existing RoonScape runtime ownership/,
    );
    assert.equal(await access(ownershipDirectory), undefined);
  });
});

test("an interrupted runtime recovery is reclaimed after its owner is gone", async () => {
  await withTaskDirectory(async (taskDirectory) => {
    const runtimeRoot = path.join(taskDirectory, "runtime");
    const runtimeDirectory = path.join(runtimeRoot, "roonscape");
    const interruptedRecovery = path.join(runtimeDirectory, ".recovering");
    await mkdir(interruptedRecovery, { mode: 0o700, recursive: true });
    await chmod(runtimeRoot, 0o700);
    await chmod(runtimeDirectory, 0o700);
    await writeFile(
      path.join(interruptedRecovery, "session.json"),
      '{"processId":424242,"token":"interrupted"}\n',
      { mode: 0o600 },
    );
    const environment = { XDG_RUNTIME_DIR: runtimeRoot };
    const bridge = pendingChild(() => undefined);
    let bridgeStarted = false;

    const result = await runRoonScapeCommand(
      [],
      commandDependencies({
        environment,
        loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
        openRuntime: async () =>
          openRuntimeSession({
            environment,
            processId: process.pid,
            userId: process.getuid?.() ?? 1_000,
            processExists: () => false,
          }),
        launchBridge: () => {
          bridgeStarted = true;
          return bridge;
        },
        launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      }),
    );

    assert.equal(result, 0);
    assert.equal(bridgeStarted, true);
  });
});

test("a child failure determines the session result and stops its peer", async () => {
  const events: string[] = [];
  const renderer = pendingChild((signal) => events.push(`renderer:${signal}`));
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
      openRuntime: async () => ({
        socketPath: "/runtime/roonscape/roonscape.sock",
        cleanup: async () => {
          events.push("runtime:cleanup");
        },
      }),
      launchBridge: () => completedChild({ exitCode: 7, signal: null }),
      launchRenderer: () => renderer,
    }),
  );

  assert.equal(result, 7);
  assert.deepEqual(events, ["renderer:SIGTERM", "runtime:cleanup"]);
});

test("a child signal remains observable as a launcher failure", async () => {
  const errors: string[] = [];
  const bridge = pendingChild(() => undefined);
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
      openRuntime: async () => ({
        socketPath: "/runtime/roonscape/roonscape.sock",
        cleanup: async () => undefined,
      }),
      launchBridge: () => bridge,
      launchRenderer: () =>
        completedChild({ exitCode: null, signal: "SIGSEGV" }),
      writeError: (line) => errors.push(line),
    }),
  );

  assert.equal(result, 1);
  assert.match(errors.join("\n"), /renderer exited from SIGSEGV/);
});

test("launcher termination stops both children and cleans runtime state", async () => {
  const events: string[] = [];
  const bridge = pendingChild((signal) => events.push(`bridge:${signal}`));
  const renderer = pendingChild((signal) => events.push(`renderer:${signal}`));
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
      openRuntime: async () => ({
        socketPath: "/runtime/roonscape/roonscape.sock",
        cleanup: async () => {
          events.push("runtime:cleanup");
        },
      }),
      launchBridge: () => bridge,
      launchRenderer: () => renderer,
      subscribeToTermination: (handler) => {
        queueMicrotask(() => handler("SIGTERM"));
        return () => events.push("signals:unsubscribe");
      },
    }),
  );

  assert.equal(result, 0);
  assert.deepEqual(events, [
    "bridge:SIGTERM",
    "renderer:SIGTERM",
    "signals:unsubscribe",
    "runtime:cleanup",
  ]);
});

test("a child still running after five seconds is forcibly terminated", async () => {
  const events: string[] = [];
  const bridge = stubbornChild((signal) => events.push(`bridge:${signal}`));
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
      openRuntime: async () => ({
        socketPath: "/runtime/roonscape/roonscape.sock",
        cleanup: async () => undefined,
      }),
      launchBridge: () => bridge,
      launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      delay: async (milliseconds) => {
        events.push(`delay:${milliseconds}`);
      },
    }),
  );

  assert.equal(result, 0);
  assert.deepEqual(events, ["bridge:SIGTERM", "delay:5000", "bridge:SIGKILL"]);
});

test("shutdown remains bounded when a child never reports exit", async () => {
  const events: string[] = [];
  const errors: string[] = [];
  const result = await runRoonScapeCommand(
    [],
    commandDependencies({
      loadConfiguration: () => ({ trackedOutputId: "output-gallery" }),
      openRuntime: async () => ({
        socketPath: "/runtime/roonscape/roonscape.sock",
        cleanup: async () => {
          events.push("runtime:cleanup");
        },
      }),
      launchBridge: () =>
        unresponsiveChild((signal) => events.push(`bridge:${signal}`)),
      launchRenderer: () => completedChild({ exitCode: 0, signal: null }),
      delay: async (milliseconds) => {
        events.push(`delay:${milliseconds}`);
      },
      writeError: (line) => errors.push(line),
    }),
  );

  assert.equal(result, 1);
  assert.deepEqual(events, [
    "bridge:SIGTERM",
    "delay:5000",
    "bridge:SIGKILL",
    "delay:1000",
    "runtime:cleanup",
  ]);
  assert.match(errors.join("\n"), /did not report exit after SIGKILL/);
});

function commandDependencies(
  overrides: Partial<RoonScapeCommandDependencies> = {},
): RoonScapeCommandDependencies {
  return {
    version: "0.1.0-test",
    environment: {},
    currentDirectory: "/working",
    standardConfigurationFile: () => "/config/roonscape/display.json",
    authorizationFile: () => "/state/roonscape/authorization.json",
    loadConfiguration: () => null,
    configurationFileExists: () => false,
    terminalIsInteractive: () => false,
    discoverTrackedOutputs: async () => {
      throw new Error("Tracked Outputs should not be discovered");
    },
    readSetupKey: async () => {
      throw new Error("setup input should not be read");
    },
    readSetupValue: async () => {
      throw new Error("setup value should not be read");
    },
    saveConfiguration: () => {
      throw new Error("Display Configuration should not be saved");
    },
    openRuntime: async () => {
      throw new Error("runtime should not be opened");
    },
    launchBridge: () => {
      throw new Error("bridge should not be launched");
    },
    launchRenderer: () => {
      throw new Error("renderer should not be launched");
    },
    subscribeToTermination: () => () => undefined,
    delay: async () => undefined,
    writeOutput: () => undefined,
    writeError: () => undefined,
    ...overrides,
  };
}

function completedChild(result: ChildResult): RunningChild {
  return {
    result: Promise.resolve(result),
    sendSignal: () => undefined,
  };
}

function abortedKeyRead(signal: AbortSignal): Promise<never> {
  return new Promise((_, reject) => {
    signal.addEventListener(
      "abort",
      () => {
        reject(new DOMException("Setup key read cancelled", "AbortError"));
      },
      { once: true },
    );
  });
}

function scriptedSetupKeys(
  ...keys: SetupKey[]
): RoonScapeCommandDependencies["readSetupKey"] {
  let keyRead = 0;
  return (signal) => {
    keyRead += 1;
    return keyRead === 1
      ? abortedKeyRead(signal)
      : Promise.resolve(keys[keyRead - 2] ?? "quit");
  };
}

function pendingChild(
  observeSignal: (signal: "SIGTERM" | "SIGKILL") => void,
): RunningChild {
  let finish: ((result: ChildResult) => void) | undefined;
  const result = new Promise<ChildResult>((resolve) => {
    finish = resolve;
  });
  return {
    result,
    sendSignal: (signal) => {
      observeSignal(signal);
      finish?.({ exitCode: 0, signal: null });
    },
  };
}

function stubbornChild(
  observeSignal: (signal: "SIGTERM" | "SIGKILL") => void,
): RunningChild {
  let finish: ((result: ChildResult) => void) | undefined;
  const result = new Promise<ChildResult>((resolve) => {
    finish = resolve;
  });
  return {
    result,
    sendSignal: (signal) => {
      observeSignal(signal);
      if (signal === "SIGKILL") {
        finish?.({ exitCode: null, signal });
      }
    },
  };
}

function unresponsiveChild(
  observeSignal: (signal: "SIGTERM" | "SIGKILL") => void,
): RunningChild {
  return {
    result: new Promise(() => undefined),
    sendSignal: observeSignal,
  };
}
