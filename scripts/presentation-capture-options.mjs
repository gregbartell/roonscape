import { presentationCaptureResolution } from "./presentation-captures.mjs";

const maximumCaptureDimension = 32_767;

export function parsePresentationCaptureRequest(arguments_) {
  const parsed = {
    all: false,
    artwork: undefined,
    listScenarios: false,
    output: undefined,
    overwrite: false,
    profile: undefined,
    resolutions: [],
    scenario: undefined,
  };

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    switch (argument) {
      case "--all":
        rejectDuplicateOption(parsed.all, argument);
        parsed.all = true;
        break;
      case "--artwork":
        rejectDuplicateOption(parsed.artwork !== undefined, argument);
        parsed.artwork = requiredValue(arguments_, ++index, argument);
        break;
      case "--list":
        throw new Error("--list was removed; use --list-scenarios");
      case "--list-scenarios":
        rejectDuplicateOption(parsed.listScenarios, argument);
        parsed.listScenarios = true;
        break;
      case "--output":
        rejectDuplicateOption(parsed.output !== undefined, argument);
        parsed.output = requiredValue(arguments_, ++index, argument);
        break;
      case "--profile":
        rejectDuplicateOption(parsed.profile !== undefined, argument);
        parsed.profile = requiredValue(arguments_, ++index, argument);
        break;
      case "--scenario":
        rejectDuplicateOption(parsed.scenario !== undefined, argument);
        parsed.scenario = requiredValue(arguments_, ++index, argument);
        break;
      case "--resolution":
        parsed.resolutions.push(
          parseResolution(requiredValue(arguments_, ++index, argument)),
        );
        break;
      case "--overwrite":
        rejectDuplicateOption(parsed.overwrite, argument);
        parsed.overwrite = true;
        break;
      case "--only":
        throw new Error("--only was removed; use --scenario");
      case "--viewport":
        throw new Error("--viewport was removed; use --resolution");
      case "--settle-ms":
        throw new Error(
          "--settle-ms was removed; Presentation Captures now wait for painted-frame readiness",
        );
      default:
        throw new Error(`unknown capture option: ${argument}`);
    }
  }

  validateRequest(parsed, arguments_.length);
  return parsed;
}

function validateRequest(parsed, argumentCount) {
  if (parsed.listScenarios && argumentCount !== 1) {
    throw new Error("--list-scenarios cannot be combined with capture options");
  }
  if (parsed.profile !== undefined && parsed.profile !== "visual-acceptance") {
    throw new Error(`unknown capture profile: ${parsed.profile}`);
  }
  if (parsed.profile === "visual-acceptance") {
    const incompatibleOption = [
      [parsed.scenario !== undefined, "--scenario"],
      [parsed.all, "--all"],
      [parsed.artwork !== undefined, "--artwork"],
      [parsed.resolutions.length > 0, "--resolution"],
    ].find(([present]) => present)?.[1];
    if (incompatibleOption !== undefined) {
      throw new Error(
        `--profile visual-acceptance cannot be combined with ${incompatibleOption}`,
      );
    }
    if (parsed.output === undefined) {
      throw new Error("--profile visual-acceptance requires --output");
    }
  }
  if (parsed.all && parsed.scenario !== undefined) {
    throw new Error("--all and --scenario cannot be combined");
  }
  if (
    parsed.scenario === undefined &&
    !parsed.all &&
    parsed.profile === undefined &&
    !parsed.listScenarios
  ) {
    throw new Error(
      "a Presentation Capture selector is required: use --scenario, --all, or --profile visual-acceptance",
    );
  }
}

function parseResolution(value) {
  const match = value.match(/^(\d+)x(\d+)$/);
  if (match === null) {
    throw new Error("--resolution must use WIDTHxHEIGHT");
  }
  const width = Number(match[1]);
  const height = Number(match[2]);
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0
  ) {
    throw new Error("--resolution dimensions must be positive safe integers");
  }
  if (width < 1280 || height < 720) {
    throw new Error("--resolution must be at least 1280x720");
  }
  if (width <= height) {
    throw new Error("--resolution must be landscape");
  }
  if (width > maximumCaptureDimension || height > maximumCaptureDimension) {
    throw new Error(
      `--resolution exceeds the supported maximum of ${maximumCaptureDimension}`,
    );
  }
  return presentationCaptureResolution(width, height);
}

function rejectDuplicateOption(duplicate, option) {
  if (duplicate) {
    throw new Error(`duplicate capture option: ${option}`);
  }
}

function requiredValue(arguments_, index, option) {
  const value = arguments_[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}
