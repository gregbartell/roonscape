import { once } from "node:events";
import { promises as fs } from "node:fs";
import { syncBuiltinESMExports } from "node:module";

const signal = process.env.ROONSCAPE_TEST_STARTUP_SIGNAL;
if (signal !== "SIGINT" && signal !== "SIGTERM") {
  throw new Error("Fixture startup probe requires a termination signal");
}

const chmod = fs.chmod;
fs.chmod = async (file, mode) => {
  await chmod(file, mode);
  if (file !== process.env.ROONSCAPE_TEST_STARTUP_SOCKET) return;

  // Pause a real asynchronous startup operation until the signal is handled.
  // Never install the first listener: that would hide default termination
  // when the application has not installed its own shutdown handlers yet.
  const handled =
    process.listenerCount(signal) > 0
      ? once(process, signal)
      : new Promise<never>(() => {});
  process.stdout.write("Fixture startup paused after opening its socket\n");
  process.kill(process.pid, signal);
  await handled;
};
syncBuiltinESMExports();
