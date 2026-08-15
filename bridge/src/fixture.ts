import { startFixturePublisher } from "./fixture-publisher.js";
import { loadSnapshot } from "./snapshot.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

const snapshot = await loadSnapshot(
  process.env.ROONSCAPE_FIXTURE ?? "fixtures/playing.json",
);
const publisher = await startFixturePublisher(snapshot, socketPath);

process.stdout.write(`Fixture publisher listening at ${socketPath}\n`);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.once(signal, () => {
    void publisher.close().finally(() => process.exit(0));
  });
}
