import assert from "node:assert/strict";
import { access, chmod, mkdir, stat, writeFile } from "node:fs/promises";
import { statSync } from "node:fs";
import path from "node:path";
import test from "node:test";

import {
  type ChildResult,
  type RoonScapeCommandDependencies,
  type RunningChild,
  runRoonScapeCommand,
} from "../src/roonscape-command.js";
import { openRuntimeSession } from "../src/runtime-session.js";
import { withTaskDirectory } from "./support.js";

test("help describes the owner-facing command", async () => {
  const output: string[] = [];
  const dependencies = commandDependencies({
    writeOutput: (line) => output.push(line),
  });

  const result = await runRoonScapeCommand(["--help"], dependencies);

  assert.equal(result, 0);
  assert.match(output.join("\n"), /Usage: roonscape \[--config PATH\]/);
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
