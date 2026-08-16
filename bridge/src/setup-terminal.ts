import { emitKeypressEvents, type Key } from "node:readline";
import type { ReadStream } from "node:tty";

import type { SetupKey } from "./first-time-setup.js";

export function terminalIsInteractive(
  input: ReadStream = process.stdin,
  output: NodeJS.WriteStream = process.stdout,
): boolean {
  return Boolean(input.isTTY && output.isTTY);
}

export function readSetupKey(
  signal: AbortSignal,
  input: ReadStream = process.stdin,
): Promise<SetupKey> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(abortError());
      return;
    }

    const wasPaused = input.isPaused();
    const wasRaw = input.isRaw;
    emitKeypressEvents(input);
    input.setRawMode(true);
    input.resume();

    const cleanup = (): void => {
      input.off("keypress", handleKeypress);
      signal.removeEventListener("abort", handleAbort);
      input.setRawMode(wasRaw);
      if (wasPaused) {
        input.pause();
      }
    };
    const finish = (key: SetupKey): void => {
      cleanup();
      resolve(key);
    };
    const handleAbort = (): void => {
      cleanup();
      reject(abortError());
    };
    const handleKeypress = (_character: string, key: Key): void => {
      if (key.ctrl && key.name === "c") {
        finish("quit");
      } else if (key.name === "up") {
        finish("up");
      } else if (key.name === "down") {
        finish("down");
      } else if (key.name === "return" || key.name === "enter") {
        finish("enter");
      } else if (key.name === "r") {
        finish("retry");
      } else if (key.name === "q") {
        finish("quit");
      }
    };

    input.on("keypress", handleKeypress);
    signal.addEventListener("abort", handleAbort, { once: true });
  });
}

function abortError(): Error {
  return new DOMException("Setup key read cancelled", "AbortError");
}
