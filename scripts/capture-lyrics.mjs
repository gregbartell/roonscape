import {
  buildLyricMotionCapturePlan,
  createNativeLyricMotionCaptureSessionAdapter,
  executeLyricMotionCapturePlan,
  parseLyricMotionCaptureRequest,
} from "./lyric-motion-capture.mjs";
import { processCancellation } from "./process-harness.mjs";

const request = parseLyricMotionCaptureRequest(process.argv.slice(2));
const cancellation = processCancellation();
try {
  const outputDirectory = await executeLyricMotionCapturePlan(
    buildLyricMotionCapturePlan(request),
    {
      sessionAdapter: createNativeLyricMotionCaptureSessionAdapter({
        signal: cancellation.signal,
      }),
    },
  );
  process.stdout.write(`${outputDirectory}\n`);
} finally {
  cancellation.dispose();
}
