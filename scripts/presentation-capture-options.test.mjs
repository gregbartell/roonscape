import assert from "node:assert/strict";
import test from "node:test";

import { parsePresentationCaptureRequest } from "./presentation-capture-options.mjs";

test("parses a focused Presentation Capture request", () => {
  assert.deepEqual(
    parsePresentationCaptureRequest([
      "--scenario",
      "playing",
      "--artwork",
      "cover.png",
      "--resolution",
      "1280x720",
      "--resolution",
      "1920x1080",
      "--output",
      "captures",
      "--overwrite",
    ]),
    {
      all: false,
      artwork: "cover.png",
      listScenarios: false,
      output: "captures",
      overwrite: true,
      profile: undefined,
      resolutions: [
        { width: 1280, height: 720, viewport: "1280x720" },
        { width: 1920, height: 1080, viewport: "1920x1080" },
      ],
      scenario: "playing",
    },
  );
});

test("reports an invalid Presentation Capture option", () => {
  assert.throws(
    () => parsePresentationCaptureRequest(["--scenario", "playing", "--wat"]),
    /unknown capture option: --wat/,
  );
});
