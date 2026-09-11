import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { createNativeSession, waitForNativeWindow } from "./native-session.mjs";
import { assertProcessRunning, waitFor } from "./process-harness.mjs";
import { resourceMetrics } from "./renderer-comparison-report.mjs";

export async function measureRenderer(
  binary,
  workload,
  options,
  run,
  output,
  signal,
) {
  let session, renderer, sampler, connection, socketError;
  const server = createServer((socket) => {
    if (connection) {
      socket.destroy();
      return;
    }
    connection = socket;
    socket.on("error", (error) => {
      socketError = error;
    });
    socket.on("close", () => {
      if (run.phase !== "cleanup")
        socketError = new Error("Renderer snapshot connection closed");
    });
    socket.on("data", () => {}); // Optional observations never gate publication.
  });
  let failure;
  const started = performance.now();
  try {
    session = await createNativeSession({
      width: options.width,
      height: options.height,
      signal,
      environment: {
        PATH: process.env.PATH,
        LANG: "C.UTF-8",
        LIBGL_ALWAYS_SOFTWARE: "1",
      },
    });
    const configuration = JSON.parse(
      await readFile(session.configurationPath, "utf8"),
    );
    configuration.inactivity.gracePeriodSeconds = 1;
    configuration.inactivity.repositionCadenceSeconds = 2;
    await writeFile(session.configurationPath, JSON.stringify(configuration));
    const gtk = path.join(session.environment.XDG_CONFIG_HOME, "gtk-4.0");
    await mkdir(gtk);
    await writeFile(
      path.join(gtk, "settings.ini"),
      `[Settings]\ngtk-enable-animations=${workload.reducedAnimation ? "false" : "true"}\n`,
    );
    const socketPath = path.join(session.runtimeDirectory, "snapshots.sock");
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(socketPath, resolve);
    });
    const environment = {
      ROONSCAPE_SOCKET: socketPath,
      ROONSCAPE_CAPTURE_VIEWPORT: `${options.width}x${options.height}`,
      ROONSCAPE_CAPTURE_REDUCED_ANIMATION: workload.reducedAnimation
        ? "1"
        : "0",
      ...(workload.static ? { ROONSCAPE_STATIC_FIXTURE: "1" } : {}),
    };
    renderer = session.startProcess(
      binary,
      ["--config", session.configurationPath],
      environment,
    );
    await renderer.spawned;
    await waitFor(
      () => {
        if (!connection) throw new Error("waiting for snapshot connection");
      },
      renderer,
      "snapshot connection",
      { signal },
    );
    let revision = 0;
    function publish(entry, origin, epoch) {
      const snapshot = structuredClone(entry.snapshot);
      snapshot.revision = ++revision;
      if (snapshot.timing?.position)
        snapshot.timing.position.sampledAt = new Date(
          epoch + entry.atMs,
        ).toISOString();
      if (snapshot.artwork?.path)
        snapshot.artwork.path = path.join(
          output,
          "content",
          path.basename(snapshot.artwork.path),
        );
      const actualMs = performance.now() - origin;
      if (socketError || connection.destroyed)
        throw socketError ?? new Error("Renderer disconnected");
      // Never await drain or an acknowledgement: excessive backlog invalidates evidence.
      if (!connection.write(JSON.stringify(snapshot) + "\n"))
        throw Object.assign(
          new Error(
            "publication backpressure: workload could not be delivered on schedule",
          ),
          { invalidEvidence: true },
        );
      run.publications.push({
        phase: run.phase,
        revision,
        plannedMs: entry.atMs,
        actualMs,
      });
    }
    publish(workload.schedule[0], performance.now(), Date.now());
    await waitForNativeWindow(
      renderer,
      { ...session.environment, ...environment },
      options.width,
      options.height,
      { signal },
    );
    run.startupMs = performance.now() - started;
    sampler = session.startProcess("python3", [
      fileURLToPath(new URL("renderer-resource-sampler.py", import.meta.url)),
      String(renderer.pid),
      String(options.intervalMs),
    ]);
    await sampler.spawned;
    const sampleLines = createInterface({ input: sampler.stdout });
    const samples = [];
    sampleLines.on("line", (line) => {
      try {
        const sample = JSON.parse(line);
        if (
          !Number.isFinite(sample.elapsedMs) ||
          !["cpuSeconds", "rssBytes"].every(
            (key) =>
              sample[key] === null ||
              (Number.isFinite(sample[key]) && sample[key] >= 0),
          )
        )
          throw new Error("malformed resource sample");
        const observed = {
          ...sample,
          receivedMs: performance.now() - started,
          phase: run.phase,
        };
        samples.push(observed);
        if (run.phase === "measurement") run.samples.push(observed);
      } catch (error) {
        socketError = Object.assign(error, { invalidEvidence: true });
      }
    });
    await waitFor(
      () => {
        if (!samples.length) throw new Error("waiting for sampler");
      },
      sampler,
      "resource sampler",
      { signal },
    );
    for (const [phase, duration] of [
      ["warmup", options.warmupMs],
      ["measurement", options.measurementMs],
    ]) {
      run.phase = phase;
      const origin = performance.now(),
        epoch = Date.now();
      let cycle = 0,
        index = 0;
      while (performance.now() - origin < duration) {
        signal.throwIfAborted();
        assertProcessRunning(renderer, "Renderer");
        assertProcessRunning(sampler, "resource sampler");
        if (socketError) throw socketError;
        let entry = workload.schedule[index];
        while (
          cycle * workload.cycleMs + entry.atMs < duration &&
          cycle * workload.cycleMs + entry.atMs <= performance.now() - origin
        ) {
          publish(
            { ...entry, atMs: cycle * workload.cycleMs + entry.atMs },
            origin,
            epoch,
          );
          index++;
          if (index === workload.schedule.length) {
            index = 0;
            cycle++;
          }
          entry = workload.schedule[index];
        }
        const next = Math.min(duration, cycle * workload.cycleMs + entry.atMs);
        await delay(
          Math.max(1, Math.min(10, next - (performance.now() - origin))),
          undefined,
          { signal },
        );
      }
      run[`${phase}Ms`] = performance.now() - origin;
      const plannedCount = Array.from(
        { length: Math.ceil(duration / workload.cycleMs) },
        (_, cycleIndex) =>
          workload.schedule.filter(
            (entry) => cycleIndex * workload.cycleMs + entry.atMs < duration,
          ).length,
      ).reduce((a, b) => a + b, 0);
      const delivered = run.publications.filter(
        (publication) => publication.phase === phase,
      );
      if (
        delivered.length !== plannedCount ||
        delivered.some(
          (publication) => publication.actualMs - publication.plannedMs > 50,
        )
      )
        throw Object.assign(
          new Error("publication schedule missed its 50 ms delivery tolerance"),
          { invalidEvidence: true },
        );
    }
    run.phase = "finished";
    signal.throwIfAborted();
    assertProcessRunning(renderer, "Renderer");
    assertProcessRunning(sampler, "resource sampler");
    if (socketError) throw socketError;
    run.metrics = resourceMetrics(run.samples);
    if (
      !run.metrics ||
      ["cpuSeconds", "peakRssBytes"].some((key) => run.metrics[key] === null) ||
      run.metrics.intervalsMs.some((value) => value <= 0)
    )
      throw Object.assign(
        new Error("unavailable or invalid resource evidence"),
        { invalidEvidence: true },
      );
    run.status = "complete";
  } catch (error) {
    failure = error;
    run.status = signal.aborted
      ? "cancelled"
      : error.invalidEvidence
        ? "invalid-evidence"
        : "execution-failed";
    run.error = error.message;
    throw error;
  } finally {
    run.phase = "cleanup";
    run.totalMs = performance.now() - started;
    run.log = `run-${run.order}.log`;
    try {
      await writeFile(
        path.join(output, run.log),
        [
          renderer?.capturedStandardOutput,
          renderer?.capturedStandardError,
          sampler?.capturedStandardError,
        ]
          .filter(Boolean)
          .join("\n"),
      );
    } finally {
      connection?.destroy();
      try {
        if (session) await session.close(failure);
      } finally {
        if (server.listening)
          await new Promise((resolve) => server.close(resolve));
      }
    }
  }
}
