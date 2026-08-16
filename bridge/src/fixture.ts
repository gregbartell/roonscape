import { startFixturePublisher } from "./fixture-publisher.js";
import { loadFixtureScenarioCatalog } from "./fixture-scenario-catalog.js";
import { loadSnapshot, type PresentationSnapshot } from "./snapshot.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

const explicitFixture = process.env.ROONSCAPE_FIXTURE;
let snapshot: PresentationSnapshot;
if (explicitFixture === undefined) {
  const catalog = await loadFixtureScenarioCatalog(
    process.env.ROONSCAPE_FIXTURE_CATALOG,
  );
  const playing = catalog[0];
  if (playing === undefined) {
    throw new Error("Fixture Scenario catalog is unexpectedly empty");
  }
  snapshot = playing.snapshot;
} else {
  snapshot = await loadSnapshot(explicitFixture);
}
const publisher = await startFixturePublisher(snapshot, socketPath);

process.stdout.write(`Fixture publisher listening at ${socketPath}\n`);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.once(signal, () => {
    void publisher.close().finally(() => process.exit(0));
  });
}
