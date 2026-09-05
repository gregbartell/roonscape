import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { parsePresentationCaptureRequest } from "./presentation-capture-options.mjs";
import { planPresentationCaptures } from "./presentation-capture-planning.mjs";
import { installPresentationCaptureFixtures } from "./presentation-capture-test-fixtures.mjs";

test("preflight resolves the complete request and aggregates applicable failures", async () => {
  const workingDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-planning-test."),
  );
  await writeFile(path.join(workingDirectory, "captures"), "not a directory");
  const request = parsePresentationCaptureRequest([
    "--scenario",
    "playing",
    "--output",
    "captures",
  ]);

  try {
    await assert.rejects(
      planPresentationCaptures(request, {
        workingDirectory,
        environment: { ...process.env, PATH: "" },
      }),
      (error) => {
        assert.match(error.message, /required executable is unavailable: Xvfb/);
        assert.match(
          error.message,
          /required executable is unavailable: xwininfo/,
        );
        assert.match(
          error.message,
          /required executable is unavailable: scrot/,
        );
        assert.match(error.message, /capture output is unavailable:/);
        return true;
      },
    );
  } finally {
    await rm(workingDirectory, { force: true, recursive: true });
  }
});

test("preflights and groups the complete maintained capture plan", async () => {
  const workingDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-profile-planning-test."),
  );
  const environment = await captureEnvironment(workingDirectory);

  try {
    const plan = await planPresentationCaptures(
      parsePresentationCaptureRequest([
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
      ]),
      { workingDirectory, environment },
    );

    assert.equal(plan.captures.length, 259);
    assert.equal(plan.sessions.length, 28);
    assert.equal(plan.sessions.flat().length, plan.captures.length);
    assert.ok(
      plan.sessions.every((captures) =>
        captures.every(
          (capture) =>
            capture.viewport === captures[0].viewport &&
            capture.typography === captures[0].typography &&
            capture.diagnostics === captures[0].diagnostics,
        ),
      ),
    );
    assert.equal(plan.incompleteSetName, "Visual-acceptance profile");
  } finally {
    await rm(workingDirectory, { force: true, recursive: true });
  }
});

test("preflights custom artwork only for compatible Fixture Scenarios", async () => {
  const workingDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-artwork-planning-test."),
  );
  const environment = await captureEnvironment(workingDirectory);
  const artworkPath = path.join(workingDirectory, "Maintainer Cover.svg");
  const artwork = "maintainer artwork";
  await writeFile(artworkPath, artwork);

  try {
    await assert.rejects(
      planPresentationCaptures(
        parsePresentationCaptureRequest([
          "--scenario",
          "idle",
          "--artwork",
          artworkPath,
        ]),
        { workingDirectory, environment },
      ),
      /--artwork is incompatible with Fixture Scenario idle/,
    );

    const plan = await planPresentationCaptures(
      parsePresentationCaptureRequest([
        "--all",
        "--artwork",
        artworkPath,
        "--resolution",
        "1280x720",
      ]),
      { workingDirectory, environment },
    );
    const playing = plan.captures.find(
      ({ scenario }) => scenario === "playing",
    );
    const idle = plan.captures.find(({ scenario }) => scenario === "idle");
    const artworkHash = createHash("sha256")
      .update(artwork)
      .digest("hex")
      .slice(0, 12);

    assert.equal(plan.captures.length, 24);
    assert.equal(plan.sessions.length, 1);
    assert.deepEqual(
      plan.captures
        .filter(({ customArtwork }) => customArtwork !== undefined)
        .map(({ scenario }) => scenario),
      [
        "playing",
        "paused",
        "lyrics-one-line",
        "lyrics-two-line",
        "lyrics-three-line",
        "lyrics-four-lines",
        "lyrics-blank-cue",
        "lyrics-long-masthead",
        "loading-with-content",
        "missing-metadata",
        "missing-artist",
        "missing-album",
        "long-metadata",
        "extreme-metadata",
        "indeterminate-progress",
      ],
    );
    assert.equal(
      playing.fileName,
      `1280x720--playing--maintainer-cover-svg--${artworkHash}.png`,
    );
    assert.equal(playing.customArtwork.contents.toString(), artwork);
    assert.equal(idle.customArtwork, undefined);
    assert.equal(idle.fileName, "1280x720--idle.png");
  } finally {
    await rm(workingDirectory, { force: true, recursive: true });
  }
});

test("rejects unusable custom artwork before Renderer work", async () => {
  const workingDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-unusable-artwork-planning-test."),
  );
  const environment = await captureEnvironment(workingDirectory);
  const emptyArtwork = path.join(workingDirectory, "empty.png");
  const directoryArtwork = path.join(workingDirectory, "directory.png");
  await writeFile(emptyArtwork, "");
  await mkdir(directoryArtwork);

  try {
    for (const [artwork, diagnostic] of [
      [path.join(workingDirectory, "missing.png"), /does not exist/],
      [emptyArtwork, /is empty/],
      [directoryArtwork, /is not a file/],
    ]) {
      await assert.rejects(
        planPresentationCaptures(
          parsePresentationCaptureRequest([
            "--scenario",
            "playing",
            "--artwork",
            artwork,
          ]),
          { workingDirectory, environment },
        ),
        diagnostic,
      );
    }
  } finally {
    await rm(workingDirectory, { force: true, recursive: true });
  }
});

test("rejects every destination collision before Renderer work", async () => {
  const workingDirectory = await mkdtemp(
    path.join(tmpdir(), "roonscape-destination-planning-test."),
  );
  const environment = await captureEnvironment(workingDirectory);
  const outputDirectory = path.join(workingDirectory, "captures");
  await mkdir(outputDirectory);
  await writeFile(path.join(outputDirectory, "1280x720--playing.png"), "old");
  await writeFile(path.join(outputDirectory, "1920x1080--playing.png"), "old");
  const request = parsePresentationCaptureRequest([
    "--scenario",
    "playing",
    "--output",
    outputDirectory,
    "--resolution",
    "1280x720",
    "--resolution",
    "1920x1080",
  ]);

  try {
    await assert.rejects(
      planPresentationCaptures(request, { workingDirectory, environment }),
      (error) => {
        assert.match(error.message, /destination files already exist:/);
        assert.match(error.message, /1280x720--playing\.png/);
        assert.match(error.message, /1920x1080--playing\.png/);
        return true;
      },
    );

    const firstDestination = path.join(
      outputDirectory,
      "1280x720--playing.png",
    );
    await rm(firstDestination);
    await mkdir(firstDestination);
    await assert.rejects(
      planPresentationCaptures(
        { ...request, overwrite: true },
        { workingDirectory, environment },
      ),
      /destination is not a replaceable file:.*1280x720--playing\.png/,
    );
  } finally {
    await rm(workingDirectory, { force: true, recursive: true });
  }
});

async function captureEnvironment(workingDirectory) {
  const binDirectory = path.join(workingDirectory, "bin");
  await installPresentationCaptureFixtures(binDirectory);
  return {
    ...process.env,
    PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
  };
}
