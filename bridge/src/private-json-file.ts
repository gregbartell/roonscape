import { randomUUID } from "node:crypto";
import { mkdirSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";

export function writePrivateJsonFile(
  file: string,
  value: unknown,
  failureMessage: string,
): void {
  const directory = path.dirname(file);
  mkdirSync(directory, { mode: 0o700, recursive: true });
  const temporaryFile = path.join(
    directory,
    `.${path.basename(file)}.${randomUUID()}.tmp`,
  );

  try {
    writeFileSync(temporaryFile, `${JSON.stringify(value, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
      mode: 0o600,
    });
    renameSync(temporaryFile, file);
  } catch (error) {
    try {
      unlinkSync(temporaryFile);
    } catch (cleanupError) {
      if (!isMissingFile(cleanupError)) {
        throw new AggregateError([error, cleanupError], failureMessage, {
          cause: cleanupError,
        });
      }
    }

    throw error;
  }
}

export function isMissingFile(error: unknown): boolean {
  return (
    error instanceof Error &&
    "code" in error &&
    (error as NodeJS.ErrnoException).code === "ENOENT"
  );
}
