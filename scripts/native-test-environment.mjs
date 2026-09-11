import { constants, accessSync, existsSync, statSync } from "node:fs";
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
      ["libjpeg", undefined, "JPEG"],
      ["xcb", undefined, "XCB"],
      ["xcb-present", undefined, "X Present"],
      ["xcb-randr", undefined, "X RandR"],
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
  return [...failures, ...qtDevelopmentFailures(environment)];
}

export function qtDevelopmentFailures(environment = process.env) {
  const qmake = findExecutable("qmake6", environment);
  if (qmake === undefined)
    return ["required executable is unavailable: qmake6"];
  const result = spawnSync(qmake, ["-query"], {
    encoding: "utf8",
    env: environment,
  });
  if (result.error !== undefined || result.status !== 0)
    return ["Qt installation metadata is unavailable: qmake6 -query failed"];
  const properties = Object.fromEntries(
    result.stdout
      .split("\n")
      .filter((line) => line.includes(":"))
      .map((line) => {
        const separator = line.indexOf(":");
        return [line.slice(0, separator), line.slice(separator + 1).trim()];
      }),
  );
  const version = /^(\d+)\.(\d+)\.\d+$/.exec(properties.QT_VERSION ?? "");
  if (version === null || Number(version[1]) !== 6 || Number(version[2]) < 2)
    return ["Qt 6.2 or newer development files are unavailable"];
  const headers = properties.QT_INSTALL_HEADERS ?? "";
  const libraries = properties.QT_INSTALL_LIBS ?? "";
  if (!path.isAbsolute(headers) || !path.isAbsolute(libraries))
    return [
      "Qt installation metadata is missing absolute header/library paths",
    ];
  return ["Core", "Gui", "Quick", "OpenGL"]
    .filter(
      (module) =>
        !existsSync(path.join(headers, `Qt${module}`, `Qt${module}`)) ||
        !existsSync(path.join(libraries, `libQt6${module}.so`)),
    )
    .map((module) => `Qt ${module} 6.2 development files are unavailable`);
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
