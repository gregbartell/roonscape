export type TerminationSignal = "SIGINT" | "SIGTERM";

export interface ProcessLifecycleEnvironment {
  once?: (
    signal: TerminationSignal,
    handler: () => void | Promise<void>,
  ) => void;
  reportError?: (message: string) => void | Promise<void>;
  exit?: (code: number) => void;
}

interface ProcessLifecycleOptions extends ProcessLifecycleEnvironment {
  cleanup(): Promise<void>;
  failureMessage: string;
}

export function installProcessLifecycle({
  cleanup,
  failureMessage,
  once = (signal, handler) => process.once(signal, handler),
  reportError = writeStandardError,
  exit = (code) => process.exit(code),
}: ProcessLifecycleOptions): void {
  for (const signal of ["SIGINT", "SIGTERM"] as const) {
    once(signal, async () => {
      try {
        await cleanup();
      } catch (error) {
        for (const failure of cleanupFailures(error)) {
          await reportError(`${failureMessage}: ${errorMessage(failure)}`);
        }
        exit(1);
        return;
      }
      exit(0);
    });
  }
}

function cleanupFailures(error: unknown): unknown[] {
  return error instanceof AggregateError
    ? error.errors.flatMap(cleanupFailures)
    : [error];
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function writeStandardError(message: string): Promise<void> {
  return new Promise((resolve, reject) => {
    process.stderr.write(`${message}\n`, (error) => {
      if (error === undefined || error === null) {
        resolve();
      } else {
        reject(error);
      }
    });
  });
}
