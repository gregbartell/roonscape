import path from "node:path";

import {
  type DisplayConfiguration,
  rejectRemovedDisplayConfigurationOverride,
} from "./display-configuration.js";
import { runSetup, type SetupDependencies } from "./first-time-setup.js";

export type TerminationSignal = "SIGINT" | "SIGTERM";

export interface ChildResult {
  exitCode: number | null;
  signal: NodeJS.Signals | null;
}

export interface RunningChild {
  readonly result: Promise<ChildResult>;
  sendSignal(signal: "SIGTERM" | "SIGKILL"): void;
}

export interface OwnedRuntime {
  readonly socketPath: string;
  cleanup(): Promise<void>;
}

export interface BridgeLaunchOptions {
  authorizationFile: string;
  configurationFile: string;
  socketPath: string;
}

export interface RendererLaunchOptions {
  configurationFile: string;
  socketPath: string;
}

export interface RoonScapeCommandDependencies extends SetupDependencies {
  readonly version: string;
  readonly environment: NodeJS.ProcessEnv;
  readonly currentDirectory: string;
  standardConfigurationFile(): string;
  loadConfiguration(configurationFile: string): DisplayConfiguration | null;
  terminalIsInteractive(): boolean;
  openRuntime(): Promise<OwnedRuntime>;
  launchBridge(options: BridgeLaunchOptions): RunningChild;
  launchRenderer(options: RendererLaunchOptions): RunningChild;
  subscribeToTermination(
    handler: (signal: TerminationSignal) => void,
  ): () => void;
}

const usage = `Usage: roonscape [--setup] [--config PATH]

Launch RoonScape as one foreground session.

Options:
  --setup        Reconfigure and exit without launching
  --config PATH  Use this Display Configuration
  --help         Show this help
  --version      Show the RoonScape version`;

export async function runRoonScapeCommand(
  arguments_: string[],
  dependencies: RoonScapeCommandDependencies,
): Promise<number> {
  if (arguments_.length === 1 && arguments_[0] === "--help") {
    dependencies.writeOutput(usage);
    return 0;
  }

  if (arguments_.length === 1 && arguments_[0] === "--version") {
    dependencies.writeOutput(`RoonScape ${dependencies.version}`);
    return 0;
  }

  const options = parseLaunchOptions(arguments_);
  if (options === null) {
    dependencies.writeError(
      `Unknown option: ${arguments_[0] ?? ""}\n\n${usage}`,
    );
    return 2;
  }

  try {
    rejectRemovedDisplayConfigurationOverride(dependencies.environment);
  } catch (error) {
    dependencies.writeError(
      error instanceof Error ? error.message : String(error),
    );
    return 2;
  }

  const configurationFile =
    options.configurationPath === undefined
      ? dependencies.standardConfigurationFile()
      : path.resolve(dependencies.currentDirectory, options.configurationPath);
  const configuration = dependencies.loadConfiguration(configurationFile);
  if (options.setupRequested && !dependencies.terminalIsInteractive()) {
    dependencies.writeError(
      "roonscape --setup requires an interactive terminal.",
    );
    return 1;
  }
  if (configuration === null && !dependencies.terminalIsInteractive()) {
    dependencies.writeError(
      `Display Configuration is missing or invalid: ${configurationFile}. Run roonscape in an interactive terminal to complete setup, or supply a valid Display Configuration with --config PATH.`,
    );
    return 1;
  }

  if (configuration === null || options.setupRequested) {
    try {
      const completed = await runSetup(
        configurationFile,
        dependencies,
        configuration,
      );
      if (!completed || options.setupRequested) {
        return 0;
      }
    } catch (error) {
      dependencies.writeError(
        `Could not complete setup: ${error instanceof Error ? error.message : String(error)}`,
      );
      return 1;
    }
  }

  try {
    return await runConfiguredSession(configurationFile, dependencies);
  } catch (error) {
    dependencies.writeError(
      `Could not launch RoonScape: ${error instanceof Error ? error.message : String(error)}`,
    );
    return 1;
  }
}

interface LaunchOptions {
  setupRequested: boolean;
  configurationPath?: string;
}

function parseLaunchOptions(arguments_: string[]): LaunchOptions | null {
  const options: LaunchOptions = { setupRequested: false };

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--setup" && !options.setupRequested) {
      options.setupRequested = true;
      continue;
    }
    if (argument === "--config" && options.configurationPath === undefined) {
      const configurationPath = arguments_[index + 1];
      if (configurationPath === undefined || configurationPath === "") {
        return null;
      }
      options.configurationPath = configurationPath;
      index += 1;
      continue;
    }
    return null;
  }

  return options;
}

interface MonitoredChild {
  readonly child: RunningChild;
  readonly result: Promise<ChildResult>;
  readonly settled: boolean;
  stopping?: Promise<void>;
}

type SessionEnding =
  | {
      kind: "child";
      role: "bridge" | "renderer";
      result: ChildResult;
    }
  | { kind: "signal"; signal: TerminationSignal };

async function runConfiguredSession(
  configurationFile: string,
  dependencies: RoonScapeCommandDependencies,
): Promise<number> {
  const runtime = await dependencies.openRuntime();
  let bridge: MonitoredChild | undefined;
  let renderer: MonitoredChild | undefined;
  let receiveTermination: ((signal: TerminationSignal) => void) | undefined;
  const termination = new Promise<TerminationSignal>((resolve) => {
    receiveTermination = resolve;
  });
  let unsubscribe = (): void => undefined;
  let sessionResult: number | undefined;
  let sessionError: unknown;

  try {
    unsubscribe = dependencies.subscribeToTermination((signal) => {
      receiveTermination?.(signal);
    });
    bridge = monitorChild(
      dependencies.launchBridge({
        authorizationFile: dependencies.authorizationFile(),
        configurationFile,
        socketPath: runtime.socketPath,
      }),
    );
    renderer = monitorChild(
      dependencies.launchRenderer({
        configurationFile,
        socketPath: runtime.socketPath,
      }),
    );

    const ending = await Promise.race<SessionEnding>([
      bridge.result.then((result) => ({
        kind: "child",
        role: "bridge",
        result,
      })),
      renderer.result.then((result) => ({
        kind: "child",
        role: "renderer",
        result,
      })),
      termination.then((signal) => ({ kind: "signal", signal })),
    ]);

    if (ending.kind === "signal") {
      await Promise.all([
        stopChild(bridge, dependencies),
        stopChild(renderer, dependencies),
      ]);
      sessionResult = 0;
    } else {
      const peer = ending.role === "bridge" ? renderer : bridge;
      await stopChild(peer, dependencies);
      if (ending.result.signal !== null) {
        dependencies.writeError(
          `RoonScape ${ending.role} exited from ${ending.result.signal}`,
        );
        sessionResult = 1;
      } else {
        sessionResult = ending.result.exitCode ?? 1;
      }
    }
  } catch (error) {
    sessionError = error;
  }

  const shutdownErrors: unknown[] = [];
  try {
    unsubscribe();
  } catch (error) {
    shutdownErrors.push(error);
  }
  const stoppedChildren = await Promise.allSettled([
    stopChild(bridge, dependencies),
    stopChild(renderer, dependencies),
  ]);
  shutdownErrors.push(...rejectedReasons(stoppedChildren));
  try {
    await runtime.cleanup();
  } catch (error) {
    shutdownErrors.push(error);
  }

  if (sessionError !== undefined) {
    const additionalShutdownErrors = shutdownErrors.filter(
      (error) => error !== sessionError,
    );
    if (additionalShutdownErrors.length > 0) {
      throw new AggregateError(
        [sessionError, ...additionalShutdownErrors],
        "RoonScape session and shutdown both failed",
      );
    }
    throw sessionError;
  }
  if (shutdownErrors.length > 0) {
    throw new AggregateError(shutdownErrors, "Could not shut down RoonScape");
  }
  return sessionResult ?? 1;
}

function monitorChild(child: RunningChild): MonitoredChild {
  let settled = false;
  const result = child.result.finally(() => {
    settled = true;
  });
  return {
    child,
    result,
    get settled() {
      return settled;
    },
  };
}

function rejectedReasons(results: PromiseSettledResult<void>[]): unknown[] {
  return results
    .filter((result) => result.status === "rejected")
    .map((result) => result.reason);
}

async function stopChild(
  child: MonitoredChild | undefined,
  dependencies: RoonScapeCommandDependencies,
): Promise<void> {
  if (child === undefined || child.settled) {
    return;
  }
  child.stopping ??= stopChildOnce(child, dependencies);
  return child.stopping;
}

async function stopChildOnce(
  child: MonitoredChild,
  dependencies: RoonScapeCommandDependencies,
): Promise<void> {
  child.child.sendSignal("SIGTERM");
  const stopped = await Promise.race([
    child.result.then(() => true),
    dependencies.delay(5_000).then(() => false),
  ]);
  if (!stopped && !child.settled) {
    child.child.sendSignal("SIGKILL");
    await Promise.resolve();
    if (child.settled) {
      return;
    }
    const killed = await Promise.race([
      child.result.then(() => true),
      dependencies.delay(1_000).then(() => false),
    ]);
    if (!killed) {
      throw new Error("A RoonScape child did not report exit after SIGKILL");
    }
  }
}
