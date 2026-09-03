import { listFixtureScenarios } from "./presentation-captures.mjs";
import { executePresentationCapturePlan } from "./presentation-capture-execution.mjs";
import { parsePresentationCaptureRequest } from "./presentation-capture-options.mjs";
import { planPresentationCaptures } from "./presentation-capture-planning.mjs";
import { publishPresentationCapture } from "./presentation-capture-publication.mjs";
import { createControlledRendererSessionAdapter } from "./presentation-capture-renderer.mjs";

const request = parsePresentationCaptureRequest(process.argv.slice(2));

if (request.listScenarios) {
  process.stdout.write(
    listFixtureScenarios()
      .map(({ scenario, label }) => `${scenario}\t${label}\n`)
      .join(""),
  );
} else {
  await executePresentationCapturePlan(
    await planPresentationCaptures(request),
    {
      sessionAdapter: createControlledRendererSessionAdapter({
        publishCapture: publishPresentationCapture,
      }),
      onCaptureStarted: ({ scenario, viewport }) => {
        process.stderr.write(
          `Capturing Fixture Scenario ${scenario} at ${viewport}\n`,
        );
      },
      onCapturePublished: (capturePath) => {
        process.stdout.write(`${capturePath}\n`);
      },
    },
  );
}
