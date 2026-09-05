import { nativeTestFailures } from "./native-test-environment.mjs";

const failures = nativeTestFailures();
if (failures.length > 0) {
  process.stderr.write(
    `Native test environment is unavailable:\n${failures
      .map((failure) => `- ${failure}`)
      .join("\n")}\n`,
  );
  process.exitCode = 1;
} else {
  process.stdout.write("Native test environment is available\n");
}
