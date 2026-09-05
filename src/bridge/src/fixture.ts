import { startFixturePublisher } from "./fixture-publisher.js";
import { installFixtureModeLifecycle } from "./fixture-lifecycle.js";
import { startFixtureModeSession } from "./fixture-mode-session.js";
import { loadFixtureScenarioCatalog } from "./fixture-scenario-catalog.js";
import { loadSnapshot } from "./snapshot.js";

const socketPath = process.env.ROONSCAPE_SOCKET;

if (socketPath === undefined || socketPath.length === 0) {
  throw new Error("ROONSCAPE_SOCKET must name the private Unix socket");
}

// Socket publication can precede startup completion. Install shutdown handlers
// now so termination during startup waits for the session and closes its sockets.
const fixtureSession = startFixtureSession(socketPath);
installFixtureModeLifecycle({ fixtureSession });
await fixtureSession;

process.stdout.write(`Fixture publisher listening at ${socketPath}\n`);

async function startFixtureSession(socketPath: string) {
  const explicitFixture = process.env.ROONSCAPE_FIXTURE;
  if (explicitFixture !== undefined) {
    const snapshot = await loadSnapshot(explicitFixture);
    return startFixturePublisher(snapshot, socketPath);
  }

  const controlSocketPath = process.env.ROONSCAPE_FIXTURE_CONTROL;
  if (controlSocketPath === undefined || controlSocketPath.length === 0) {
    throw new Error(
      "ROONSCAPE_FIXTURE_CONTROL must name the private Fixture Mode control socket",
    );
  }
  const catalog = await loadFixtureScenarioCatalog(
    process.env.ROONSCAPE_FIXTURE_CATALOG,
  );
  return startFixtureModeSession(catalog, socketPath, controlSocketPath, {
    behavior:
      process.env.ROONSCAPE_STATIC_FIXTURE === "1" ? "static" : "ordinary",
  });
}
