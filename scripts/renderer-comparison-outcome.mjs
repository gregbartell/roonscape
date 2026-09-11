export function failureOutcome(error, signal) {
  if (signal.aborted) return { status: "cancelled", exitCode: 130 };
  if (error.unsupported) return { status: "unavailable", exitCode: 4 };
  if (error.invalidEvidence) return { status: "invalid-evidence", exitCode: 2 };
  if (error.behaviorFailure) return { status: "behavior-failed", exitCode: 3 };
  return { status: "execution-failed", exitCode: 1 };
}
