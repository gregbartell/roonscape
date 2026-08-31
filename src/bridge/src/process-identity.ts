import { readFileSync } from "node:fs";

export type ProcessIdentityObservation =
  { status: "absent" } | { status: "observed"; processStartTimeTicks: string };

export type ObserveProcessIdentity = (
  processId: number,
) => ProcessIdentityObservation;

export function observeLinuxProcessIdentity(
  processId: number,
): ProcessIdentityObservation {
  let stat: string;
  try {
    stat = readFileSync(`/proc/${processId}/stat`, "utf8");
  } catch (error) {
    if (errorCode(error) === "ENOENT") {
      return { status: "absent" };
    }
    throw error;
  }
  return {
    status: "observed",
    processStartTimeTicks: parseLinuxProcessStartTimeTicks(stat),
  };
}

export function parseLinuxProcessStartTimeTicks(stat: string): string {
  const processNameEnd = stat.lastIndexOf(")");
  if (processNameEnd < 0) {
    throw new Error("Linux process stat is missing its process name boundary");
  }

  const fieldsAfterProcessName = stat
    .slice(processNameEnd + 1)
    .trim()
    .split(/\s+/);
  const processStartTimeTicks = fieldsAfterProcessName[19];
  if (
    processStartTimeTicks === undefined ||
    !/^\d+$/.test(processStartTimeTicks)
  ) {
    throw new Error("Linux process stat has an invalid process start time");
  }
  return processStartTimeTicks;
}

function errorCode(error: unknown): string | undefined {
  return error instanceof Error && "code" in error
    ? (error as NodeJS.ErrnoException).code
    : undefined;
}
