import { mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { findExecutable } from "./native-test-environment.mjs";
import {
  processCancellation,
  startMonitoredProcess,
  stopProcess,
  waitForProcessExit,
} from "./process-harness.mjs";

const cancellation = processCancellation();
let child;
try {
  console.log(
    "Live Capture Session helper tests: deterministic logic and synthetic media only; passing does not verify an actual Live Capture Session.",
  );
  for (const name of ["ffmpeg", "ffprobe"]) {
    if (!findExecutable(name))
      throw new Error(`${name} is required; see docs/development.md`);
  }
  await mkdir("/var/tmp/codex/roonscape", { recursive: true });
  child = startMonitoredProcess(process.execPath, [
    "--test",
    fileURLToPath(
      new URL(
        "../.agents/skills/capture-live-session/scripts/live-capture-session.test.mjs",
        import.meta.url,
      ),
    ),
  ]);
  child.stdout.pipe(process.stdout);
  child.stderr.pipe(process.stderr);
  await child.spawned;
  const [code] = await waitForProcessExit(child, {
    signal: cancellation.signal,
  });
  process.exitCode = code ?? 1;
} catch (error) {
  console.error(error.message);
  process.exitCode = cancellation.signal.aborted ? 130 : 1;
} finally {
  await stopProcess(child);
  cancellation.dispose();
}
