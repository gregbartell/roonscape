import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Render documentation graphics from the approved SVG; does not build a release.
const root = fileURLToPath(new URL("../", import.meta.url));
const outputDirectory = path.join(root, "docs/branding");
const source = readFileSync(
  path.join(root, "src/renderer/assets/roonscape.svg"),
  "utf8",
);
const scratchRoot = "/var/tmp/codex/roonscape";
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(path.join(scratchRoot, "task."));
mkdirSync(outputDirectory, { recursive: true });

try {
  // Register only the bundled fonts for this process, without host installation.
  const fontConfig = path.join(scratch, "fonts.conf");
  const xmlEscape = (value) =>
    value.replaceAll("&", "&amp;").replaceAll("<", "&lt;");
  writeFileSync(
    fontConfig,
    `<?xml version="1.0"?>
<fontconfig>
  <dir>${xmlEscape(path.join(root, "src/renderer/assets/fonts"))}</dir>
  <cachedir>${xmlEscape(path.join(scratch, "font-cache"))}</cachedir>
</fontconfig>`,
  );

  function render(name, svg, width, height) {
    const inputFile = path.join(scratch, "render.svg");
    writeFileSync(inputFile, svg);
    const result = spawnSync(
      "rsvg-convert",
      [
        "--width",
        String(width),
        "--height",
        String(height),
        "--output",
        path.join(outputDirectory, name),
        inputFile,
      ],
      {
        encoding: "utf8",
        timeout: 30_000,
        env: { ...process.env, FONTCONFIG_FILE: fontConfig },
      },
    );
    if (result.error || result.status !== 0) {
      throw new Error(
        `Could not render ${name}; install rsvg-convert (librsvg). ${result.error?.message ?? result.stderr}`,
      );
    }
    process.stdout.write(`Wrote docs/branding/${name}\n`);
  }

  for (const [name, color] of [
    ["black", "#000000"],
    ["white", "#ffffff"],
  ]) {
    for (const size of [256, 1024]) {
      render(
        `roonscape-${name}-${size}.png`,
        source.replaceAll("currentColor", color),
        size,
        size,
      );
    }
  }

  const icon = (x, y, size) =>
    source
      .replace(
        "<svg ",
        `<svg x="${x}" y="${y}" width="${size}" height="${size}" `,
      )
      .replaceAll("currentColor", "#203443");

  render(
    "roonscape-avatar-512.png",
    `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
  <rect width="512" height="512" fill="#e4edf3"/>
  ${icon(52, 64, 384)}
</svg>`,
    512,
    512,
  );

  render(
    "roonscape-share-1280x640.png",
    `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="640" viewBox="0 0 1280 640">
  <rect width="1280" height="640" fill="#e4edf3"/>
  ${icon(76, 156, 320)}
  <text x="472" y="275" font-family="Libre Baskerville" font-weight="700" font-size="72" fill="#203443">RoonScape</text>
  <g font-family="IBM Plex Sans" fill="#203443">
    <text x="476" y="346" font-size="32">Your music, on display.</text>
    <text x="476" y="398" font-size="24">An unattended Now Playing display for Roon.</text>
    <text x="476" y="548" font-size="20" fill="#526878">github.com/gregbartell/roonscape</text>
  </g>
</svg>`,
    1280,
    640,
  );
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
