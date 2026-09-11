import { options } from "./compare-renderer.mjs";

try {
  const args = process.argv.slice(2),
    physical = {},
    remaining = [];
  for (let i = 0; i < args.length; i += 2) {
    if (
      ["--display", "--physical-output", "--max-missed-refreshes"].includes(
        args[i],
      )
    ) {
      if (
        !args[i + 1] ||
        args[i + 1].startsWith("--") ||
        physical[args[i]] !== undefined
      )
        throw new Error(`missing or duplicate value: ${args[i]}`);
      physical[args[i]] = args[i + 1];
    } else remaining.push(args[i], args[i + 1]);
  }
  if (!physical["--display"] || !physical["--physical-output"])
    throw new Error(
      "--display and --physical-output are required; physical execution is never inferred from DISPLAY",
    );
  const maximum =
    physical["--max-missed-refreshes"] === undefined
      ? null
      : Number(physical["--max-missed-refreshes"]);
  if (maximum !== null && (!Number.isSafeInteger(maximum) || maximum < 0))
    throw new Error("invalid --max-missed-refreshes");
  const selected = options(remaining);
  selected.physical = {
    display: physical["--display"],
    output: physical["--physical-output"],
    maxMissedRefreshes: maximum,
  };
  const { compare } = await import("./renderer-comparison.mjs");
  process.exitCode = await compare(selected);
} catch (error) {
  console.error(error.message);
  process.exitCode = 2;
}
