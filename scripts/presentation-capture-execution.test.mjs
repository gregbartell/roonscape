import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { executePresentationCapturePlan } from "./presentation-capture-execution.mjs";
import { installPresentationCaptureFixtures } from "./presentation-capture-test-fixtures.mjs";
import { planPresentationCaptures } from "./presentation-capture-planning.mjs";

test("executes the complete maintained plan through in-memory sessions", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "roonscape-execution-test."),
  );
  const binDirectory = path.join(directory, "bin");
  await installPresentationCaptureFixtures(binDirectory);
  const plan = await planPresentationCaptures(
    {
      all: false,
      artwork: undefined,
      listScenarios: false,
      output: path.join(directory, "captures"),
      overwrite: false,
      profile: "visual-acceptance",
      resolutions: [],
      scenario: undefined,
    },
    {
      environment: {
        ...process.env,
        PATH: `${binDirectory}${path.delimiter}${process.env.PATH ?? ""}`,
      },
    },
  );
  const observedSessions = [];
  const started = [];
  const published = [];
  const sessionAdapter = {
    async execute(captures, observer) {
      observedSessions.push(captures);
      for (const capture of captures) {
        observer.captureStarted(capture);
        observer.capturePublished(capture.finalCapturePath);
      }
    },
  };

  try {
    const completedPaths = await executePresentationCapturePlan(plan, {
      sessionAdapter,
      onCaptureStarted: ({ scenario, viewport }) =>
        started.push(`${viewport}:${scenario}`),
      onCapturePublished: (capturePath) => published.push(capturePath),
    });

    assert.equal(plan.captures.length, 259);
    assert.deepEqual(observedSessions, plan.sessions);
    const executionOrder = plan.sessions.flat();
    assert.deepEqual(
      started,
      executionOrder.map(({ scenario, viewport }) => `${viewport}:${scenario}`),
    );
    assert.deepEqual(
      completedPaths,
      executionOrder.map(({ finalCapturePath }) => finalCapturePath),
    );
    assert.deepEqual(published, completedPaths);

    const processFailure = new Error("Renderer exited with status 9");
    let captureIndex = 0;
    const failingSessionAdapter = {
      async execute(captures, observer) {
        for (const capture of captures) {
          observer.captureStarted(capture);
          if (captureIndex === 100) {
            throw processFailure;
          }
          captureIndex += 1;
          observer.capturePublished(capture.finalCapturePath);
        }
      },
    };
    await assert.rejects(
      executePresentationCapturePlan(plan, {
        sessionAdapter: failingSessionAdapter,
      }),
      (error) => {
        assert.equal(error.cause, processFailure);
        assert.deepEqual(
          error.completedPaths,
          executionOrder
            .slice(0, 100)
            .map(({ finalCapturePath }) => finalCapturePath),
        );
        assert.match(
          error.message,
          /Visual-acceptance profile is incomplete \(100\/259 captures completed\)/,
        );
        return true;
      },
    );
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

test("reports an incomplete all-scenario execution", async () => {
  const cause = new Error("renderer exited with status 9");
  const captures = [
    capture("playing", "/captures/playing.png"),
    capture("paused", "/captures/paused.png"),
    capture("idle", "/captures/idle.png"),
  ];
  const sessionAdapter = {
    async execute(sessionCaptures, observer) {
      for (const capture of sessionCaptures) {
        observer.captureStarted(capture);
        if (capture.scenario === "paused") {
          throw cause;
        }
        observer.capturePublished(capture.finalCapturePath);
      }
    },
  };

  const plan = {
    captures,
    sessions: [captures.slice(0, 2), captures.slice(2)],
    incompleteSetName: "All-scenario capture",
  };
  await assert.rejects(
    executePresentationCapturePlan(plan, { sessionAdapter }),
    (error) => {
      assert.equal(error.cause, cause);
      assert.deepEqual(error.completedPaths, ["/captures/playing.png"]);
      assert.match(
        error.message,
        /All-scenario capture is incomplete \(1\/3 captures completed\)/,
      );
      assert.match(error.message, /Fixture Scenario paused at 1280x720/);
      assert.match(error.message, /- \/captures\/playing\.png/);
      assert.match(error.message, /renderer exited with status 9/);
      return true;
    },
  );
});

test("identifies the first capture when a session fails during startup", async () => {
  const firstCapture = capture("playing", "/captures/playing.png");

  await assert.rejects(
    executePresentationCapturePlan(
      { captures: [firstCapture], sessions: [[firstCapture]] },
      {
        sessionAdapter: {
          async execute() {
            throw new Error("could not start Renderer");
          },
        },
      },
    ),
    /Fixture Scenario playing at 1280x720/,
  );
});

function capture(scenario, finalCapturePath) {
  return {
    scenario,
    viewport: "1280x720",
    finalCapturePath,
  };
}
