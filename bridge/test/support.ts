import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";

const scratchRoot = "/tmp/codex/roonscape";

export async function withTaskDirectory<T>(
  run: (taskDirectory: string) => Promise<T>,
): Promise<T> {
  await mkdir(scratchRoot, { recursive: true });
  const taskDirectory = await mkdtemp(path.join(scratchRoot, "task."));
  try {
    return await run(taskDirectory);
  } finally {
    await rm(taskDirectory, { recursive: true });
  }
}
