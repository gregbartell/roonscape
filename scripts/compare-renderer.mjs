import path from "node:path";

function options(arguments_) {
  const values = {};
  for (let index = 0; index < arguments_.length; index += 2) {
    const key = arguments_[index];
    if (
      ![
        "--baseline",
        "--candidate",
        "--profile",
        "--workloads",
        "--output",
        "--resolution",
        "--repeats",
        "--warmup-seconds",
        "--measurement-seconds",
      ].includes(key)
    )
      throw new Error(`unknown option: ${key}`);
    const value = arguments_[index + 1];
    if (!value || value.startsWith("--") || values[key])
      throw new Error(`missing or duplicate value: ${key}`);
    values[key] = value;
  }
  if (!values["--baseline"] || !values["--candidate"])
    throw new Error(
      "--baseline and --candidate are required (ref:REVISION or worktree:PATH)",
    );
  const profile = values["--profile"] ?? "default";
  if (!["default", "thorough", "smoke"].includes(profile))
    throw new Error("unknown profile");
  const resolution = values["--resolution"] ?? "1280x720";
  if (!/^\d+x\d+$/.test(resolution)) throw new Error("invalid resolution");
  const [width, height] = resolution.split("x").map(Number);
  if (width < 1280 || height < 720 || width <= height || width > 8192)
    throw new Error(
      "resolution must be landscape, at least 1280x720, at most 8192 wide",
    );
  const overrides = {};
  for (const [flag, key, min, max, multiplier] of [
    ["--repeats", "repeats", 1, 20, 1],
    ["--warmup-seconds", "warmupMs", 0.1, 60, 1000],
    ["--measurement-seconds", "measurementMs", 0.2, 300, 1000],
  ]) {
    if (values[flag] === undefined) continue;
    const value = Number(values[flag]);
    if (
      !Number.isFinite(value) ||
      value < min ||
      value > max ||
      (key === "repeats" && !Number.isInteger(value))
    )
      throw new Error(`invalid ${flag}`);
    overrides[key] = value * multiplier;
  }
  return {
    overrides,
    baseline: values["--baseline"],
    candidate: values["--candidate"],
    profile,
    workloads: values["--workloads"]?.split(","),
    output: values["--output"] ? path.resolve(values["--output"]) : undefined,
    width,
    height,
  };
}
let selected;
try {
  selected = options(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 2;
}
if (selected) {
  const { compare } = await import("./renderer-comparison.mjs");
  process.exitCode = await compare(selected);
}
