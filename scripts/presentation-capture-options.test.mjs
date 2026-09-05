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

test("requires exactly one compatible Presentation Capture selector", () => {
  const invalidRequests = [
    [[], /a Presentation Capture selector is required/],
    [["--output", "captures"], /a Presentation Capture selector is required/],
    [["--list"], /--list was removed; use --list-scenarios/],
    [["--only", "playing"], /--only was removed; use --scenario/],
    [["--viewport", "1280x720"], /--viewport was removed; use --resolution/],
    [
      ["--settle-ms", "0"],
      /Presentation Captures now wait for painted-frame readiness/,
    ],
    [
      ["--list-scenarios", "--overwrite"],
      /--list-scenarios cannot be combined with capture options/,
    ],
    [
      ["--all", "--scenario", "playing"],
      /--all and --scenario cannot be combined/,
    ],
  ];

  for (const [arguments_, diagnostic] of invalidRequests) {
    assert.throws(
      () => parsePresentationCaptureRequest(arguments_),
      diagnostic,
      arguments_.join(" "),
    );
  }
});

test("rejects invalid resolutions and duplicate scalar options", () => {
  const invalidRequests = [
    [["--scenario", "playing", "--resolution", "wide"], /WIDTHxHEIGHT/],
    [["--scenario", "playing", "--resolution", "0x720"], /positive/],
    [["--scenario", "playing", "--resolution", "1280x719"], /at least/],
    [["--scenario", "playing", "--resolution", "1280x1280"], /landscape/],
    [["--scenario", "playing", "--resolution", "32768x720"], /maximum/],
    [
      ["--scenario", "playing", "--artwork", "a", "--artwork", "b"],
      /duplicate capture option: --artwork/,
    ],
  ];

  for (const [arguments_, diagnostic] of invalidRequests) {
    assert.throws(
      () => parsePresentationCaptureRequest(arguments_),
      diagnostic,
      arguments_.join(" "),
    );
  }
});

test("validates the visual-acceptance profile without launching a command", () => {
  const invalidRequests = [
    [["--profile", "visual-acceptance"], /requires --output/],
    [
      [
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
        "--scenario",
        "playing",
      ],
      /cannot be combined with --scenario/,
    ],
    [
      [
        "--profile",
        "visual-acceptance",
        "--output",
        "captures",
        "--resolution",
        "1920x1080",
      ],
      /cannot be combined with --resolution/,
    ],
    [
      ["--profile", "brief", "--output", "captures"],
      /unknown capture profile: brief/,
    ],
  ];

  for (const [arguments_, diagnostic] of invalidRequests) {
    assert.throws(
      () => parsePresentationCaptureRequest(arguments_),
      diagnostic,
      arguments_.join(" "),
    );
  }
});
