import { spawn } from "node:child_process";

import type { RunningChild } from "./roonscape-command.js";

export function launchChildProcess(
  executable: string,
  arguments_: string[],
  environment: NodeJS.ProcessEnv,
): RunningChild {
  const child = spawn(executable, arguments_, {
    env: environment,
    stdio: "inherit",
  });
  const result = new Promise<{
    exitCode: number | null;
    signal: NodeJS.Signals | null;
  }>((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (exitCode, signal) => resolve({ exitCode, signal }));
  });

  return {
    result,
    sendSignal: (signal) => {
      child.kill(signal);
    },
  };
}
