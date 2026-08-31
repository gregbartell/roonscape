import { mkdtemp, readFile, rename, rm } from "node:fs/promises";
import path from "node:path";

export async function publishPresentationCapture({
  finalCapturePath,
  width,
  height,
  produce,
}) {
  const publicationDirectory = await mkdtemp(
    path.join(path.dirname(finalCapturePath), ".roonscape-capture."),
  );
  try {
    const temporaryCapturePath = path.join(publicationDirectory, "capture.png");
    await produce(temporaryCapturePath);
    await validatePngDimensions(temporaryCapturePath, width, height);
    await rename(temporaryCapturePath, finalCapturePath);
  } finally {
    await rm(publicationDirectory, { force: true, recursive: true });
  }
}

async function validatePngDimensions(filePath, expectedWidth, expectedHeight) {
  const header = await readFile(filePath);
  const pngSignature = "89504e470d0a1a0a";
  if (
    header.length < 24 ||
    header.subarray(0, 8).toString("hex") !== pngSignature ||
    header.subarray(12, 16).toString("ascii") !== "IHDR"
  ) {
    throw new Error(`${filePath} is not a PNG capture`);
  }

  const width = header.readUInt32BE(16);
  const height = header.readUInt32BE(20);
  if (width !== expectedWidth || height !== expectedHeight) {
    throw new Error(
      `${filePath} is ${width}x${height}; expected ${expectedWidth}x${expectedHeight}`,
    );
  }
}
