import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

export async function withTaskDirectory<T>(
  run: (taskDirectory: string) => Promise<T>,
): Promise<T> {
  const taskDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-bridge-test."),
  );
  try {
    return await run(taskDirectory);
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
}
