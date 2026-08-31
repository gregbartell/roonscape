import { startFixturePublisher } from "./fixture-publisher.js";
import { installFixtureModeLifecycle } from "./fixture-lifecycle.js";
import { startFixtureModeSession } from "./fixture-mode-session.js";
import { loadFixtureScenarioCatalog } from "./fixture-scenario-catalog.js";
import { loadSnapshot } from "./snapshot.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

const explicitFixture = process.env.ROONSCAPE_FIXTURE;
let fixtureSession;
if (explicitFixture === undefined) {
  const controlSocketPath = process.env.ROONSCAPE_FIXTURE_CONTROL;
  if (controlSocketPath === undefined || controlSocketPath.length === 0) {
    throw new Error(
      "ROONSCAPE_FIXTURE_CONTROL must name the private Fixture Mode control socket",
    );
  }
  const catalog = await loadFixtureScenarioCatalog(
    process.env.ROONSCAPE_FIXTURE_CATALOG,
  );
  fixtureSession = await startFixtureModeSession(
    catalog,
    socketPath,
    controlSocketPath,
    {
      behavior:
        process.env.ROONSCAPE_STATIC_FIXTURE === "1" ? "static" : "ordinary",
    },
  );
} else {
  const snapshot = await loadSnapshot(explicitFixture);
  fixtureSession = await startFixturePublisher(snapshot, socketPath);
}

process.stdout.write(`Fixture publisher listening at ${socketPath}\n`);

installFixtureModeLifecycle({ fixtureSession });
