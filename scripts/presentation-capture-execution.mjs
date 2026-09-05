export async function executePresentationCapturePlan(
  plan,
  {
    sessionAdapter,
    onCaptureStarted = () => {},
    onCapturePublished = () => {},
  },
) {
  const completedPaths = [];
  let activeCapture;

  try {
    for (const sessionCaptures of plan.sessions) {
      [activeCapture] = sessionCaptures;
      await sessionAdapter.execute(sessionCaptures, {
        captureStarted(capture) {
          activeCapture = capture;
          onCaptureStarted(capture);
        },
        capturePublished(capturePath) {
          completedPaths.push(capturePath);
          onCapturePublished(capturePath);
        },
      });
    }
    return completedPaths;
  } catch (error) {
    const completed =
      completedPaths.length === 0
        ? "none"
        : completedPaths.map((capturePath) => `- ${capturePath}`).join("\n");
    const active =
      activeCapture === undefined
        ? "before the first Fixture Scenario started"
        : `while capturing Fixture Scenario ${activeCapture.scenario} at ${activeCapture.viewport}`;
    const setName = plan.incompleteSetName ?? "Presentation Capture";
    const executionError = new Error(
      `${setName} is incomplete (${completedPaths.length}/${plan.captures.length} captures completed) ${active}.\nCompleted captures:\n${completed}\nFailure: ${errorMessage(error)}`,
      { cause: error },
    );
    executionError.completedPaths = [...completedPaths];
    throw executionError;
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
