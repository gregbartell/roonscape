import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { createNativeSession, waitForNativeWindow } from "./native-session.mjs";
import {
  assertProcessRunning,
  waitFor,
  waitForProcessExit,
  processFailure,
} from "./process-harness.mjs";
import { contentEvidence } from "./renderer-comparison-content.mjs";
import {
  readPhysicalEvidence,
  probePhysical,
} from "./physical-presentation-run.mjs";
import { resourceMetrics } from "./renderer-comparison-report.mjs";
import { failureOutcome } from "./renderer-comparison-outcome.mjs";

export async function measureRenderer(
  binary,
  workload,
  options,
  run,
  output,
  signal,
) {
  let session,
    renderer,
    sampler,
    connection,
    socketError,
    controlServer,
    control;
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
      if (!["cleanup", "finishing"].includes(run.phase))
        socketError = new Error("Renderer snapshot connection closed");
    });
    socket.on("data", () => {}); // Optional observations never gate publication.
  });
  let failure;
  const started = performance.now();
  const diagnostic = run.kind === "content" || run.kind === "physical";
  try {
    session = await createNativeSession({
      width: options.width,
      height: options.height,
      signal,
      ...(options.physical && {
        physicalDisplay: {
          display: options.physical.display,
          authority: options.physicalTools.authority,
        },
      }),
      environment: {
        PATH: process.env.PATH,
        LANG: "C.UTF-8",
        ...(options.physical
          ? { QT_XCB_GL_INTEGRATION: "xcb_glx" }
          : { LIBGL_ALWAYS_SOFTWARE: "1" }),
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
    if (diagnostic) {
      run.evidence = `content-${run.order}.jsonl`;
      const controlPath = path.join(session.runtimeDirectory, "content.sock");
      controlServer = createServer((socket) => {
        control = socket;
        socket.on("error", (error) => {
          socketError = error;
        });
      });
      await new Promise((resolve, reject) => {
        controlServer.once("error", reject);
        controlServer.listen(controlPath, resolve);
      });
      environment.ROONSCAPE_CONTENT_EVIDENCE = path.join(output, run.evidence);
      environment.ROONSCAPE_CONTENT_EVIDENCE_CONTROL = controlPath;
    }
    if (run.kind === "physical") {
      run.presentationEvidence = `presentation-${run.order}.jsonl`;
      run.animationEvidence = `animation-${run.order}.jsonl`;
      environment.LD_PRELOAD = options.physicalTools.collector.path;
      environment.ROONSCAPE_PRESENT_EVIDENCE = path.join(
        output,
        run.presentationEvidence,
      );
      environment.ROONSCAPE_ANIMATION_EVIDENCE = path.join(
        output,
        run.animationEvidence,
      );
    }
    if (options.physical)
      run.capabilities = await probePhysical(
        options.physicalTools.probe.path,
        options,
        signal,
      );
    renderer = session.startProcess(
      binary,
      ["--config", session.configurationPath],
      environment,
    );
    await renderer.spawned;
    run.process = {
      pid: renderer.pid,
      sessionDirectory: session.runtimeDirectory,
      display: session.environment.DISPLAY,
      ownsDisplay: !options.physical,
    };
    await waitFor(
      () => {
        if (!connection) throw new Error("waiting for snapshot connection");
      },
      renderer,
      "snapshot connection",
      { signal },
    );
    if (controlServer)
      await waitFor(
        () => {
          if (!control) throw new Error("waiting for diagnostic control");
        },
        renderer,
        "content evidence control",
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
      const actualMs = performance.now() - origin;
      if (socketError || connection.destroyed)
        throw socketError ?? new Error("Renderer disconnected");
      // Never await drain or an acknowledgement: excessive backlog invalidates evidence.
      const publishedMicros = Number(process.hrtime.bigint() / 1000n);
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
        contentId: entry.contentId,
        artwork: snapshot.artwork,
        asset: entry.asset,
        publishedMicros,
        plannedMs: entry.atMs,
        actualMs,
      });
    }
    publish(workload.phases.warmup.entries[0], performance.now(), Date.now());
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
      const entries = workload.phases[phase].entries;
      run.windows ??= {};
      run.windows[phase] = {
        startMicros: Number(process.hrtime.bigint() / 1000n),
        durationMs: duration,
      };
      let index = 0;
      while (performance.now() - origin < duration) {
        signal.throwIfAborted();
        assertProcessRunning(renderer, "Renderer");
        assertProcessRunning(sampler, "resource sampler");
        if (socketError) throw socketError;
        while (
          index < entries.length &&
          entries[index].atMs <= performance.now() - origin
        )
          publish(entries[index++], origin, epoch);
        const next = Math.min(duration, entries[index]?.atMs ?? duration);
        await delay(
          Math.max(1, Math.min(10, next - (performance.now() - origin))),
          undefined,
          { signal },
        );
      }
      run[`${phase}Ms`] = performance.now() - origin;
      run.windows[phase].endMicros = Number(process.hrtime.bigint() / 1000n);
      const plannedCount = entries.length;
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
    if (diagnostic) {
      run.phase = "drain";
      const drainStarted = performance.now();
      await delay(options.drainMs ?? 1000, undefined, { signal });
      run.drainMs = performance.now() - drainStarted;
      run.phase = "finishing";
      control.write("F");
      const [code, exitSignal] = await waitForProcessExit(renderer, {
        signal,
        timeoutMilliseconds: 5000,
      });
      // A valid footer distinguishes explicitly lost evidence from process failure.
      try {
        run.content = await contentEvidence(
          path.join(output, run.evidence),
          run,
        );
      } catch (error) {
        run.content = { status: "invalid-evidence", reason: error.message };
        if (
          code !== 0 &&
          !/overflow|(?:Content|Animation) evidence/i.test(
            renderer.capturedStandardError,
          )
        )
          throw processFailure(
            "diagnostic Renderer",
            renderer,
            code,
            exitSignal,
          );
        throw error;
      }
      if (code !== 0) {
        const error = processFailure(
          "diagnostic Renderer",
          renderer,
          code,
          exitSignal,
        );
        if (
          /(?:Content|Animation) evidence/i.test(renderer.capturedStandardError)
        )
          error.invalidEvidence = true;
        throw error;
      }
      if (run.kind === "physical") {
        run.physical = await readPhysicalEvidence(output, run, options);
        const after = await probePhysical(
          options.physicalTools.probe.path,
          options,
          signal,
        );
        if (JSON.stringify(after) !== JSON.stringify(run.capabilities))
          throw Object.assign(
            new Error("physical display conditions changed during measurement"),
            { invalidEvidence: true },
          );
        if (run.physical.status !== "complete")
          throw Object.assign(
            new Error(run.physical.reasons.join("; ")),
            run.physical.status === "unavailable"
              ? { unsupported: true }
              : { invalidEvidence: true },
          );
        if (run.physical.contract.status === "failed")
          throw Object.assign(
            new Error("requested physical cadence contract failed"),
            { behaviorFailure: true },
          );
      }
      if (run.content.failures.length)
        throw Object.assign(new Error(run.content.failures.join("; ")), {
          behaviorFailure: true,
        });
    }
    run.status = "complete";
  } catch (error) {
    failure = error;
    run.status = failureOutcome(error, signal).status;
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
      control?.destroy();
      try {
        if (session) await session.close(failure);
      } finally {
        if (server.listening)
          await new Promise((resolve) => server.close(resolve));
        if (controlServer?.listening)
          await new Promise((resolve) => controlServer.close(resolve));
      }
    }
  }
}
