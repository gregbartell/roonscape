import { readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";
import { runMonitoredProcess } from "./process-harness.mjs";
import { digest } from "./renderer-comparison-sources.mjs";
import { summarizePhysicalPresentation } from "./physical-presentation.mjs";

export async function preparePhysical(options, output, signal) {
  const build = async (source, name, flags) => {
    const file = fileURLToPath(new URL(source, import.meta.url));
    const executable = path.join(output, name);
    const log = await runMonitoredProcess(
      "c++",
      [
        "-std=c++17",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        file,
        "-o",
        executable,
        ...flags,
      ],
      { signal, timeoutMilliseconds: 60000 },
    );
    await writeFile(`${executable}.log`, log);
    return {
      path: executable,
      sourceDigest: digest(await readFile(file)),
      binaryDigest: digest(await readFile(executable)),
    };
  };
  const probe = await build("presentation-probe.cpp", "presentation-probe", [
    "-lxcb",
    "-lxcb-randr",
  ]);
  const collector = await build(
    "presentation-collector.cpp",
    "presentation-collector.so",
    ["-shared", "-fPIC", "-lxcb", "-lxcb-present", "-ldl", "-pthread"],
  );
  const authority =
    process.env.XAUTHORITY ?? path.join(os.homedir(), ".Xauthority");
  options.physicalAuthority = authority;
  const capabilities = await probePhysical(probe.path, options, signal);
  return {
    probe,
    collector,
    authority,
    capabilities,
    compiler: (
      await runMonitoredProcess("c++", ["--version"], { signal })
    ).trim(),
    policy:
      "Explicit existing physical display; private home/configuration and bus; no display modes or power policy changed. Scanout flips required. Fixed refresh only; variable-refresh execution must be disabled by the operator before selection.",
    limitations: [
      "Present scanout completion is not optical verification",
      "Only the Qt xcb / DRI3 PresentPixmap path is decoded; missing interposition cannot establish delivery",
      "Clean physical resource passes and instrumented passes are separate; external load and repeat order can affect overhead estimates",
    ],
  };
}
export async function probePhysical(probe, options, signal) {
  try {
    return JSON.parse(
      await runMonitoredProcess(
        probe,
        [
          options.physical.output,
          String(options.width),
          String(options.height),
        ],
        {
          signal,
          timeoutMilliseconds: 5000,
          environment: {
            PATH: process.env.PATH,
            DISPLAY: options.physical.display,
            XAUTHORITY: options.physicalAuthority ?? process.env.XAUTHORITY,
          },
        },
      ),
    );
  } catch (error) {
    throw Object.assign(
      new Error(`physical capability unavailable: ${error.message}`),
      { unsupported: true },
    );
  }
}
export async function readPhysicalEvidence(output, run, options) {
  const read = async (file) => {
    if ((await stat(file)).size > 128 * 1024 * 1024)
      throw new Error("physical evidence exceeds 128 MiB read bound");
    return (await readFile(file, "utf8"))
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
  };
  try {
    const [trace, animation, draws] = await Promise.all(
      [run.presentationEvidence, run.animationEvidence, run.evidence].map(
        (file) => read(path.join(output, file)),
      ),
    );
    const start = animation[0],
      end = animation.at(-1);
    if (
      start?.event !== "start" ||
      start.version !== 1 ||
      start.clock !== "CLOCK_MONOTONIC" ||
      end?.event !== "end" ||
      end.complete !== true ||
      end.lost !== 0 ||
      end.records !== animation.length - 2
    )
      throw new Error(
        "animation evidence is incomplete, overflowed, or malformed",
      );
    return summarizePhysicalPresentation({
      trace,
      animation,
      draws,
      publications: run.content.publications,
      window: run.windows.measurement,
      maxMissedRefreshes: options.physical.maxMissedRefreshes,
    });
  } catch (error) {
    throw Object.assign(
      new Error(`invalid physical evidence: ${error.message}`),
      { invalidEvidence: true },
    );
  }
}
