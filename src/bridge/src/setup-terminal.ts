import {
  clearScreenDown,
  createInterface,
  emitKeypressEvents,
  type Key,
} from "node:readline";
import type { ReadStream } from "node:tty";

import type { SetupKey } from "./first-time-setup.js";

const saveCursorPosition = "\u001b7";
const restoreCursorPosition = "\u001b8";

export function terminalIsInteractive(
  input: ReadStream = process.stdin,
  output: NodeJS.WriteStream = process.stdout,
): boolean {
  return Boolean(input.isTTY && output.isTTY);
}

export function writeSetupLines(
  lines: readonly string[],
  replacePrevious: boolean,
  output: NodeJS.WritableStream = process.stdout,
): void {
  if (replacePrevious) {
    output.write(restoreCursorPosition);
    clearScreenDown(output);
  } else {
    output.write(saveCursorPosition);
  }
  output.write(`${lines.join("\n")}\n`);
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

    const wasRaw = input.isRaw;
    emitKeypressEvents(input);
    input.setRawMode(true);
    input.resume();

    let settled = false;
    const cleanup = (): void => {
      input.off("keypress", handleKeypress);
      signal.removeEventListener("abort", handleAbort);
      input.setRawMode(wasRaw);
      input.pause();
    };
    const finish = (key: SetupKey): void => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      resolve(key);
    };
    const handleAbort = (): void => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      reject(abortError());
    };
    const handleKeypress = (_character: string, key: Key): void => {
      if (key.ctrl && key.name === "c") {
        handleAbort();
      } else if (key.name === "up") {
        finish("up");
      } else if (key.name === "down") {
        finish("down");
      } else if (key.name === "return" || key.name === "enter") {
        finish("enter");
      } else if (key.name === "c") {
        finish("customize");
      }
    };

    input.on("keypress", handleKeypress);
    signal.addEventListener("abort", handleAbort, { once: true });
  });
}

export function readSetupValue(
  prompt: string,
  initialValue: string,
  signal: AbortSignal,
  input: ReadStream = process.stdin,
  output: NodeJS.WriteStream = process.stdout,
): Promise<string> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(abortError());
      return;
    }

    const terminal = createInterface({ input, output, terminal: true });
    let settled = false;
    const cleanup = (): void => {
      terminal.off("SIGINT", handleInterrupt);
      signal.removeEventListener("abort", handleAbort);
      terminal.close();
    };
    const finish = (value: string): void => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      resolve(value);
    };
    const cancel = (): void => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      reject(abortError());
    };
    const handleInterrupt = (): void => cancel();
    const handleAbort = (): void => cancel();

    terminal.on("SIGINT", handleInterrupt);
    signal.addEventListener("abort", handleAbort, { once: true });
    terminal.question(`${prompt} `, finish);
    terminal.write(initialValue);
  });
}

function abortError(): Error {
  return new DOMException("Setup cancelled", "AbortError");
}
