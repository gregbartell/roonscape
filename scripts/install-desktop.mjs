import {
  constants,
  accessSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { randomUUID } from "node:crypto";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const applicationId = "io.roonscape.Renderer";
const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));

// Desktop Exec arguments have their own quoting rules, separate from shell rules.
function desktopArgument(value) {
  return `"${value
    .replaceAll("%", "%%")
    .replace(/[\\"`$]/g, "\\$&")
    .replaceAll("\\", "\\\\")}"`;
}

function shellArgument(value) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function writeAtomic(filename, contents, mode = 0o644) {
  mkdirSync(path.dirname(filename), { recursive: true });
  const temporary = `${filename}.${randomUUID()}.tmp`;
  try {
    writeFileSync(temporary, contents, { flag: "wx", mode });
    renameSync(temporary, filename);
  } finally {
    rmSync(temporary, { force: true });
  }
}

export function installDesktop({
  root = repositoryRoot,
  environment = process.env,
  nodeExecutable = process.execPath,
  uninstall = false,
} = {}) {
  const dataHome =
    environment.XDG_DATA_HOME ||
    path.join(environment.HOME || homedir(), ".local/share");
  for (const value of [root, dataHome, nodeExecutable]) {
    if (!path.isAbsolute(value) || /[\r\n\t]/.test(value)) {
      throw new Error(
        "Desktop installation paths must be absolute and contain no tabs or line breaks.",
      );
    }
  }
  const entry = path.join(dataHome, "applications", `${applicationId}.desktop`);
  const icon = path.join(
    dataHome,
    "icons/hicolor/scalable/apps",
    `${applicationId}.svg`,
  );
  const launcher = path.join(dataHome, "roonscape/desktop-launch");
  const files = [entry, icon, launcher];
  for (const filename of files) {
    const metadata = lstatSync(filename, { throwIfNoEntry: false });
    if (metadata && !metadata.isFile()) {
      throw new Error(`Refusing to replace a non-file: ${filename}`);
    }
  }
  const existingEntry = existsSync(entry) ? readFileSync(entry, "utf8") : "";
  const owner = existingEntry.match(/^X-RoonScape-Root=(.+)$/m)?.[1];
  const encodedRoot = encodeURIComponent(root);
  if (uninstall) {
    if (!existingEntry && !files.some((filename) => existsSync(filename)))
      return [];
    if (owner !== encodedRoot) {
      throw new Error(
        "This desktop entry belongs to another installation. Uninstall it from that checkout or installation directory.",
      );
    }
    for (const filename of files) rmSync(filename, { force: true });
    return files;
  }
  if (!owner && files.some((filename) => existsSync(filename))) {
    throw new Error(
      "Existing desktop files are not managed by RoonScape; refusing to overwrite them.",
    );
  }

  const bridge = path.join(root, "src/bridge/dist/src/roonscape.js");
  try {
    accessSync(bridge, constants.R_OK);
  } catch {
    throw new Error("Build the RoonScape Bridge first: npm run build");
  }
  try {
    accessSync(
      path.join(root, "target/release/roonscape-renderer"),
      constants.X_OK,
    );
  } catch {
    throw new Error(
      "Build the Renderer first: cargo build --locked --release --package roonscape-renderer",
    );
  }
  accessSync(nodeExecutable, constants.X_OK);
  const iconSource = readFileSync(
    path.join(
      root,
      "src/desktop/icons/hicolor/scalable/apps",
      `${applicationId}.svg`,
    ),
  );
  const template = readFileSync(
    path.join(root, "src/desktop", `${applicationId}.desktop.in`),
    "utf8",
  );
  const desktop = template
    .replace("@EXEC@", () => `/bin/sh ${desktopArgument(launcher)}`)
    .replace("@ROOT@", () => encodedRoot);
  // Capture Node's absolute path: desktop sessions may not inherit a version manager's PATH.
  const command = `#!/bin/sh\nexec ${shellArgument(nodeExecutable)} ${shellArgument(bridge)} "$@"\n`;
  writeAtomic(icon, iconSource);
  writeAtomic(launcher, command, 0o755);
  writeAtomic(entry, desktop);
  return files;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const arguments_ = process.argv.slice(2);
  if (arguments_.length === 1 && arguments_[0] === "--help") {
    process.stdout.write(
      "Usage: node scripts/install-desktop.mjs [--uninstall]\nInstall or remove the current user's RoonScape desktop entry and icon.\nComplete setup in a terminal before launching from the application menu.\n",
    );
  } else if (
    arguments_.length > 1 ||
    (arguments_.length === 1 && arguments_[0] !== "--uninstall")
  ) {
    process.stderr.write(
      "Usage: node scripts/install-desktop.mjs [--uninstall]\n",
    );
    process.exitCode = 2;
  } else {
    try {
      const uninstall = arguments_[0] === "--uninstall";
      const files = installDesktop({ uninstall });
      process.stdout.write(
        `${uninstall ? "Removed" : "Installed"} desktop integration:\n${files.join("\n")}\n`,
      );
    } catch (error) {
      process.stderr.write(`RoonScape: ${error.message}\n`);
      process.exitCode = 1;
    }
  }
}
