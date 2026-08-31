export async function attemptAllCleanup(
  failureMessage: string,
  cleanupSteps: ReadonlyArray<() => void | Promise<void>>,
): Promise<void> {
  const results = await Promise.allSettled(
    cleanupSteps.map((cleanup) => Promise.resolve().then(cleanup)),
  );
  const failures = results.flatMap((result) =>
    result.status === "rejected" ? [result.reason] : [],
  );
  if (failures.length > 0) {
    throw new AggregateError(failures, failureMessage);
  }
}
