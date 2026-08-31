import type { FixtureModeSession } from "./fixture-mode-session.js";
import {
  installProcessLifecycle,
  type ProcessLifecycleEnvironment,
} from "./process-lifecycle.js";

interface FixtureModeLifecycleOptions extends ProcessLifecycleEnvironment {
  fixtureSession: FixtureModeSession;
}

export function installFixtureModeLifecycle({
  fixtureSession,
  once,
  reportError,
  exit,
}: FixtureModeLifecycleOptions): void {
  installProcessLifecycle({
    cleanup: () => fixtureSession.close(),
    failureMessage: "Could not stop Fixture Mode",
    once,
    reportError,
    exit,
  });
}
