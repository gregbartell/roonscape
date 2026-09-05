import { constants, accessSync, statSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export function nativeTestFailures(environment = process.env) {
  const requiredExecutables = [
    "Xvfb",
    "xwininfo",
    "scrot",
    "dbus-daemon",
    "pkg-config",
  ];
  const failures = requiredExecutables
    .filter((name) => findExecutable(name, environment) === undefined)
    .map((name) => `required executable is unavailable: ${name}`);
  const pkgConfig = findExecutable("pkg-config", environment);
  if (pkgConfig !== undefined) {
    const gtk = spawnSync(pkgConfig, ["--atleast-version=4.6", "gtk4"], {
      stdio: "ignore",
      env: environment,
    });
    if (gtk.error !== undefined || gtk.status !== 0)
      failures.push("GTK 4.6 development files are unavailable");
  }
  return failures;
}

export function findExecutable(name, environment = process.env) {
  for (const directory of (environment.PATH ?? "").split(path.delimiter)) {
    if (directory.length === 0) continue;
    const candidate = path.join(directory, name);
    try {
      accessSync(candidate, constants.X_OK);
      if (statSync(candidate).isFile()) return candidate;
    } catch (error) {
      if (!["ENOENT", "EACCES", "ENOTDIR"].includes(error?.code)) throw error;
    }
  }
  return undefined;
}
