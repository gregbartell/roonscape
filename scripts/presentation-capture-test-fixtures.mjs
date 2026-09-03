import { chmod, copyFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const fixtureDirectory = fileURLToPath(
  new URL("test-fixtures/presentation-capture", import.meta.url),
);

export async function installPresentationCaptureFixtures(binDirectory) {
  await mkdir(binDirectory, { recursive: true });
  const renderer = path.join(binDirectory, "roonscape-renderer");
  await Promise.all(
    [
      ["Xvfb", "Xvfb"],
      ["xwininfo", "xwininfo"],
      ["scrot", "scrot"],
      ["renderer.mjs", "roonscape-renderer"],
    ].map(async ([sourceName, executableName]) => {
      const destination = path.join(binDirectory, executableName);
      await copyFile(path.join(fixtureDirectory, sourceName), destination);
      await chmod(destination, 0o755);
    }),
  );
  return { renderer };
}

export function presentationCapturePngHeader(width, height) {
  const header = Buffer.alloc(33);
  Buffer.from("89504e470d0a1a0a", "hex").copy(header);
  header.writeUInt32BE(13, 8);
  Buffer.from("IHDR").copy(header, 12);
  header.writeUInt32BE(width, 16);
  header.writeUInt32BE(height, 20);
  header[24] = 8;
  header[25] = 2;
  return header;
}
