import { constants } from "node:fs";
import { copyFile, mkdir, symlink, writeFile, chmod } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

// Independent executable paths and launcher roots, sharing read-only source
// assets. Reflinks avoid duplicating the large native debug executable on disk.
export async function installFixtureWorktree(directory, rendererSource) {
  await mkdir(path.join(directory, "scripts"), { recursive: true });
  await symlink(path.join(repositoryRoot, "src"), path.join(directory, "src"));
  for (const name of [
    "run-fixture.mjs",
    "native-session.mjs",
    "process-harness.mjs",
  ]) {
    await copyFile(
      path.join(repositoryRoot, "scripts", name),
      path.join(directory, "scripts", name),
    );
  }
  for (const profile of ["debug", "release"]) {
    const executable = path.join(
      directory,
      "target",
      profile,
      "roonscape-renderer",
    );
    await mkdir(path.dirname(executable), { recursive: true });
    if (rendererSource === undefined) {
      await copyFile(
        path.join(repositoryRoot, "target/debug/roonscape-renderer"),
        executable,
        constants.COPYFILE_FICLONE,
      );
    } else {
      await writeFile(executable, rendererSource);
      await chmod(executable, 0o755);
    }
  }
  return path.join(directory, "target/debug/roonscape-renderer");
}
