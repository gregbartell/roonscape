#!/usr/bin/env node

import { createHash } from "node:crypto";
import { once } from "node:events";
import { existsSync } from "node:fs";
import { appendFile, readFile } from "node:fs/promises";
import { createConnection } from "node:net";
import { createInterface } from "node:readline";

const log = process.env.ROONSCAPE_CAPTURE_TEST_PROCESS_LOG;
const style = process.env.ROONSCAPE_CAPTURE_TEST_LOG_STYLE ?? "general";
const connection = createConnection(process.env.ROONSCAPE_CAPTURE_CONTROL);

process.once("SIGTERM", async () => {
  if (style === "focused") {
    await appendFile(log, "renderer-stopped\n");
  }
  process.exit(0);
});

await once(connection, "connect");
for await (const line of createInterface({ input: connection })) {
  const selection = JSON.parse(line);
  if (
    selection.type !== "select" ||
    selection.revision !== selection.snapshot.revision
  ) {
    process.exit(2);
  }

  if (style === "focused") {
    const observedArtworkHash =
      selection.snapshot.artwork === null
        ? null
        : createHash("sha256")
            .update(await readFile(selection.snapshot.artwork.path))
            .digest("hex")
            .slice(0, 12);
    await appendFile(
      log,
      `selection|${JSON.stringify({ ...selection, observedArtworkHash })}\npainted|${Date.now()}\n`,
    );
  } else {
    const artwork = selection.snapshot.artwork?.path ?? "none";
    await appendFile(
      log,
      `selection|${selection.scenario}|${selection.revision}|${artwork}\n`,
    );
  }

  if (
    existsSync(process.env.ROONSCAPE_CAPTURE_TEST_FAILURE_MARKER ?? "") &&
    selection.scenario === "loading-with-content"
  ) {
    process.exit(9);
  }

  if (style !== "focused") {
    await appendFile(
      log,
      `painted|${selection.scenario}|${selection.revision}\n`,
    );
  }
  connection.write(
    `${JSON.stringify({
      type: "painted",
      scenario: selection.scenario,
      revision: selection.revision,
    })}\n`,
  );
}

if (style === "focused") {
  await appendFile(log, "renderer-stopped\n");
}
