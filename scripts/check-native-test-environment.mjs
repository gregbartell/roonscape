import { constants, accessSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const requiredExecutables = ["Xvfb", "xwininfo", "scrot", "pkg-config"];
const executablePaths = new Map(
  requiredExecutables.map((name) => [name, findExecutable(name)]),
);
const failures = requiredExecutables
  .filter((name) => executablePaths.get(name) === undefined)
  .map((name) => `required executable is unavailable: ${name}`);

const pkgConfig = executablePaths.get("pkg-config");
if (pkgConfig !== undefined) {
  const gtk = spawnSync(pkgConfig, ["--atleast-version=4.6", "gtk4"], {
    stdio: "ignore",
  });
  if (gtk.error !== undefined || gtk.status !== 0) {
    failures.push("GTK 4.6 development files are unavailable");
  }
}

if (failures.length > 0) {
  process.stderr.write(
    `Native test environment is unavailable:\n${failures
      .map((failure) => `- ${failure}`)
      .join("\n")}\n`,
  );
  process.exitCode = 1;
} else {
  process.stdout.write("Native test environment is available\n");
}

function findExecutable(name) {
  for (const directory of (process.env.PATH ?? "").split(path.delimiter)) {
    if (directory.length === 0) continue;
    const candidate = path.join(directory, name);
    try {
      accessSync(candidate, constants.X_OK);
      return candidate;
    } catch (error) {
      if (error?.code !== "ENOENT" && error?.code !== "EACCES") throw error;
    }
  }
  return undefined;
}
