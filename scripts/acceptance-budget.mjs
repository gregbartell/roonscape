// Work shares one monotonic deadline. Cleanup uses its own bounds after dispose.
export function createAcceptanceBudget({
  timeoutMilliseconds = 30 * 60_000,
  signal,
} = {}) {
  const deadline = performance.now() + timeoutMilliseconds;
  const controller = new AbortController();
  const failure = new Error("acceptance work budget exhausted");
  const timer = setTimeout(
    () => controller.abort(failure),
    timeoutMilliseconds,
  );
  const combinedSignal = signal
    ? AbortSignal.any([signal, controller.signal])
    : controller.signal;

  return {
    timeoutMilliseconds,
    signal: combinedSignal,
    waitOptions(phaseMilliseconds = Infinity) {
      const remaining = Math.max(0, deadline - performance.now());
      if (remaining === 0) controller.abort(failure);
      combinedSignal.throwIfAborted();
      return {
        signal: combinedSignal,
        timeoutMilliseconds: Math.min(phaseMilliseconds, remaining),
      };
    },
    dispose() {
      clearTimeout(timer);
    },
  };
}
