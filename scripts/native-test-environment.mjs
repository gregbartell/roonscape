import { constants, accessSync, statSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export function nativeTestFailures(environment = process.env) {
  const requiredExecutables = [
    "Xvfb",
    "xwininfo",
    "xprop",
    "gio",
    "desktop-file-validate",
    "scrot",
    "dbus-daemon",
    "pkg-config",
    "ffmpeg",
    "ffprobe",
  ];
  const failures = requiredExecutables
    .filter((name) => findExecutable(name, environment) === undefined)
    .map((name) => `required executable is unavailable: ${name}`);
  const pkgConfig = findExecutable("pkg-config", environment);
  if (pkgConfig !== undefined) {
    for (const [name, minimum, label] of [
      ["gtk4", "4.6", "GTK 4.6"],
      ["Qt6Quick", "6.2", "Qt Quick 6.2"],
      ["Qt6OpenGL", "6.2", "Qt OpenGL 6.2"],
      ["libjpeg", undefined, "JPEG"],
    ]) {
      const result = spawnSync(
        pkgConfig,
        [
          minimum === undefined ? "--exists" : `--atleast-version=${minimum}`,
          name,
        ],
        { stdio: "ignore", env: environment },
      );
      if (result.error !== undefined || result.status !== 0)
        failures.push(`${label} development files are unavailable`);
    }
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
