import { listFixtureScenarios } from "./presentation-captures.mjs";
import { parsePresentationCaptureRequest } from "./presentation-capture-options.mjs";
import { planPresentationCaptures } from "./presentation-capture-planning.mjs";
import { publishPresentationCapture } from "./presentation-capture-publication.mjs";
import { runControlledRendererSession } from "./presentation-capture-renderer.mjs";

const request = parsePresentationCaptureRequest(process.argv.slice(2));

if (request.listScenarios) {
  process.stdout.write(
    listFixtureScenarios()
      .map(({ scenario, label }) => `${scenario}\t${label}\n`)
      .join(""),
  );
} else {
  await capturePlannedPresentations(await planPresentationCaptures(request));
}

async function capturePlannedPresentations(plan) {
  const completedPaths = [];
  try {
    for (const sessionCaptures of plan.sessions) {
      await runControlledRendererSession(sessionCaptures, {
        publishCapture: publishPresentationCapture,
        onCaptureStarted: ({ scenario, viewport }) => {
          process.stderr.write(
            `Capturing Fixture Scenario ${scenario} at ${viewport}\n`,
          );
        },
        onCapturePublished: (capturePath) => {
          completedPaths.push(capturePath);
          process.stdout.write(`${capturePath}\n`);
        },
      });
    }
  } catch (error) {
    if (plan.incompleteSetName === undefined) {
      throw error;
    }
    const completed =
      completedPaths.length === 0
        ? "none"
        : completedPaths.map((capturePath) => `- ${capturePath}`).join("\n");
    throw new Error(
      `${plan.incompleteSetName} is incomplete (${completedPaths.length}/${plan.captures.length} captures completed).\nCompleted captures:\n${completed}\nFailure: ${errorMessage(error)}`,
      { cause: error },
    );
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
