import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

export function installQtFixture(directory, version = "6.2.4") {
  const headers = path.join(directory, "qt/include");
  const libraries = path.join(directory, "qt/lib");
  mkdirSync(libraries, { recursive: true });
  for (const module of ["Core", "Gui", "Quick", "OpenGL"]) {
    mkdirSync(path.join(headers, `Qt${module}`), { recursive: true });
    writeFileSync(path.join(headers, `Qt${module}`, `Qt${module}`), "");
    writeFileSync(path.join(libraries, `libQt6${module}.so`), "");
  }
  const executable = path.join(directory, "qmake6");
  writeFileSync(
    executable,
    `#!${process.execPath}\nconsole.log(${JSON.stringify(`QT_VERSION:${version}\nQT_INSTALL_HEADERS:${headers}\nQT_INSTALL_LIBS:${libraries}`)});\n`,
  );
  chmodSync(executable, 0o755);
}
