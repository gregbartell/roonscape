import assert from "node:assert/strict";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readlink,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { createConnection } from "node:net";
import test from "node:test";
import { createNativeSession, waitForNativeWindow } from "./native-session.mjs";
import { installFixtureWorktree } from "./native-session-test-fixtures.mjs";
import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcesses,
  stopProcess,
  waitFor,
  waitForProcessExit,
} from "./process-harness.mjs";

const rendererExecutable = path.resolve("target/debug/roonscape-renderer");

test("concurrent Xvfb allocation owns distinct ready displays", async () => {
  const sessions = [];
  try {
    await Promise.all(
      Array.from({ length: 6 }, async (_, index) => {
        const session = await startXvfbDisplay({
          width: 1280 + index * 10,
          height: 720,
        });
        sessions.push({ ...session, width: 1280 + index * 10 });
      }),
    );
    assert.equal(new Set(sessions.map(({ display }) => display)).size, 6);
    for (const { display, width, xvfb } of sessions) {
      const geometry = await runMonitoredProcess("xwininfo", ["-root"], {
        environment: {
          ...process.env,
          DISPLAY: display,
          XAUTHORITY: "/dev/null",
        },
      });
      assert.match(geometry, new RegExp(`Width: ${width}\\b`));
      assertProcessRunning(xvfb, "owned display");
    }
  } finally {
    await stopProcesses(sessions.map(({ xvfb }) => xvfb));
  }
});

test("GTK redirects a second Renderer on the same bus even on a different display", async () => {
  const sentinel = await createNativeSession({ width: 1280, height: 720 });
  let neighbor;
  try {
    const first = sentinel.startProcess(
      rendererExecutable,
      ["--config", sentinel.configurationPath],
      {
        ROONSCAPE_SOCKET: path.join(sentinel.runtimeDirectory, "missing.sock"),
        ROONSCAPE_CAPTURE_VIEWPORT: "1280x720",
      },
    );
    await first.spawned;
    await waitForNativeWindow(first, sentinel.environment, 1280, 720);
    neighbor = await createNativeSession({ width: 1600, height: 900 });
    const second = neighbor.startProcess(
      rendererExecutable,
      ["--config", neighbor.configurationPath],
      {
        DBUS_SESSION_BUS_ADDRESS: sentinel.environment.DBUS_SESSION_BUS_ADDRESS,
        ROONSCAPE_SOCKET: path.join(neighbor.runtimeDirectory, "missing.sock"),
        ROONSCAPE_CAPTURE_VIEWPORT: "1600x900",
      },
    );
    await second.spawned;
    assert.deepEqual(
      await waitForProcessExit(second, { timeoutMilliseconds: 5_000 }),
      [0, null],
    );
    assertProcessRunning(first, "sentinel Renderer");
    await assert.rejects(
      runMonitoredProcess("xwininfo", ["-name", "RoonScape"], {
        environment: neighbor.environment,
        timeoutMilliseconds: 500,
      }),
    );
  } finally {
    await neighbor?.close();
    await sentinel.close();
  }
});

test(
  "independent headless worktrees survive neighboring cancellation and startup failure",
  { timeout: 30_000 },
  async () => {
    await mkdir("/var/tmp/codex/roonscape", { recursive: true });
    const directory = await mkdtemp("/var/tmp/codex/roonscape/task.");
    const sentinel = await createNativeSession({ width: 1280, height: 720 });
    const launchers = [];
    try {
      const sentinelRenderer = sentinel.startProcess(
        rendererExecutable,
        ["--config", sentinel.configurationPath],
        {
          ROONSCAPE_SOCKET: path.join(
            sentinel.runtimeDirectory,
            "missing.sock",
          ),
          ROONSCAPE_CAPTURE_VIEWPORT: "1280x720",
        },
      );
      await sentinelRenderer.spawned;
      const sentinelWindow = await waitForNativeWindow(
        sentinelRenderer,
        sentinel.environment,
        1280,
        720,
      );
      const hostConfig = path.join(directory, "host-config");
      await mkdir(hostConfig);
      await writeFile(
        path.join(hostConfig, "display.json"),
        "conflicting configuration, must remain untouched",
      );
      await writeFile(
        path.join(hostConfig, "authorization.json"),
        "controlled authorization sentinel",
      );
      const conflictingEnvironment = {
        ...process.env,
        ...sentinel.environment,
        HOME: hostConfig,
        XDG_CONFIG_HOME: hostConfig,
        WAYLAND_DISPLAY: "unrelated-wayland",
        GDK_BACKEND: "wayland",
        GDK_SCALE: "2",
        GDK_DPI_SCALE: "2",
        XAUTHORITY: path.join(hostConfig, "missing-authority"),
        ROONSCAPE_DISPLAY_CONFIG: path.join(hostConfig, "display.json"),
        ROONSCAPE_CAPTURE_CONTROL: path.join(
          hostConfig,
          "missing-control.sock",
        ),
        ROONSCAPE_CAPTURE_TYPOGRAPHY: "invalid",
        ROONSCAPE_CAPTURE_VIEWPORT: "invalid",
        ROONSCAPE_FIXTURE: "missing-host-fixture.json",
        ROONSCAPE_FIXTURE_CONTROL: path.join(
          hostConfig,
          "missing-navigation.sock",
        ),
        ROONSCAPE_FIXTURE_AUTO_CLOSE_MS: "1",
        ROONSCAPE_SOCKET: path.join(hostConfig, "unrelated.sock"),
        ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL: path.join(
          hostConfig,
          "missing-paint.sock",
        ),
      };
      const roots = [path.join(directory, "a"), path.join(directory, "b")];
      await Promise.all(roots.map((root) => installFixtureWorktree(root)));
      const ready = await Promise.all(
        roots.map(async (root, index) => {
          const child = startMonitoredProcess(
            process.execPath,
            [
              path.join(root, "scripts/run-fixture.mjs"),
              "--headless",
              "--static",
              "--scenario",
              index === 0 ? "playing" : "paused",
              "--resolution",
              index === 0 ? "1280x720" : "1600x900",
            ],
            { environment: conflictingEnvironment, cwd: root },
          );
          launchers.push(child);
          await child.spawned;
          return waitFor(
            () => {
              const match = child.capturedStandardOutput.match(
                /DISPLAY=([^;]+); runtime=([^\n]+)/,
              );
              assert.ok(match, child.capturedStandardError);
              return {
                child,
                display: match[1],
                runtime: match[2],
                root,
                width: index === 0 ? 1280 : 1600,
              };
            },
            child,
            "headless Fixture Mode readiness",
            { timeoutMilliseconds: 10_000 },
          );
        }),
      );
      assert.notEqual(ready[0].display, ready[1].display);
      assert.notEqual(ready[0].runtime, ready[1].runtime);
      for (const [index, run] of ready.entries()) {
        const snapshot = await readSnapshot(
          path.join(run.runtime, "roonscape.sock"),
        );
        assert.equal(snapshot.revision, index === 0 ? 7 : 8);
        assert.equal(snapshot.playback, index === 0 ? "playing" : "paused");
        const geometry = await runMonitoredProcess(
          "xwininfo",
          ["-name", "RoonScape"],
          {
            environment: {
              ...process.env,
              DISPLAY: run.display,
              XAUTHORITY: "/dev/null",
            },
          },
        );
        assert.match(geometry, new RegExp(`Width: ${run.width}\\b`));
        const childPids = (
          await readFile(
            `/proc/${run.child.pid}/task/${run.child.pid}/children`,
            "utf8",
          )
        )
          .trim()
          .split(/\s+/);
        const executables = await Promise.all(
          childPids.map((pid) => readlink(`/proc/${pid}/exe`)),
        );
        assert.ok(
          executables.includes(
            path.join(run.root, "target/debug/roonscape-renderer"),
          ),
          executables.join("\n"),
        );
      }
      ready[0].child.kill("SIGTERM");
      const [code] = await waitForProcessExit(ready[0].child, {
        timeoutMilliseconds: 7_000,
      });
      assert.equal(code, 130, ready[0].child.capturedStandardError);
      await assert.rejects(access(ready[0].runtime), { code: "ENOENT" });
      assertProcessRunning(ready[1].child, "neighbor Fixture Mode");
      assert.equal(
        (await readSnapshot(path.join(ready[1].runtime, "roonscape.sock")))
          .playback,
        "paused",
      );
      await assert.rejects(
        runMonitoredProcess(
          process.execPath,
          [
            path.join(roots[0], "scripts/run-fixture.mjs"),
            "--headless",
            "--scenario",
            "missing",
          ],
          { environment: conflictingEnvironment },
        ),
        /unknown Fixture Scenario/,
      );
      assertProcessRunning(sentinelRenderer, "unrelated sentinel Renderer");
      const after = await waitForNativeWindow(
        sentinelRenderer,
        sentinel.environment,
        1280,
        720,
      );
      assert.equal(after, sentinelWindow);
      assert.equal(
        await readFile(path.join(hostConfig, "display.json"), "utf8"),
        "conflicting configuration, must remain untouched",
      );
      assert.equal(
        await readFile(path.join(hostConfig, "authorization.json"), "utf8"),
        "controlled authorization sentinel",
      );
    } finally {
      // Send cancellation to the CLI, allowing it to reap its separately owned groups.
      for (const child of launchers)
        if (child.exitCode === null) child.kill("SIGTERM");
      await Promise.all(
        launchers.map((child) =>
          waitForProcessExit(child, { timeoutMilliseconds: 7_000 }),
        ),
      );
      await stopProcesses(launchers);
      await sentinel.close();
      await rm(directory, { recursive: true, force: true });
    }
  },
);

function readSnapshot(socketPath) {
  return new Promise((resolve, reject) => {
    const socket = createConnection(socketPath);
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("snapshot replay timed out"));
    }, 2000);
    let input = "";
    socket.on("data", (chunk) => {
      input += chunk;
      if (!input.includes("\n")) return;
      clearTimeout(timer);
      socket.destroy();
      try {
        resolve(JSON.parse(input.slice(0, input.indexOf("\n"))));
      } catch (error) {
        reject(error);
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

test(
  "startup cancellation escalates an unresponsive Renderer and removes only its runtime",
  { timeout: 15_000 },
  async () => {
    await mkdir("/var/tmp/codex/roonscape", { recursive: true });
    const directory = await mkdtemp("/var/tmp/codex/roonscape/task.");
    let launcher;
    let runtime;
    let childPids = [];
    try {
      await installFixtureWorktree(
        directory,
        `#!/usr/bin/env node
process.on("SIGTERM", () => {});
console.log("stubborn renderer=" + process.pid);
setInterval(() => {}, 1000);
`,
      );
      launcher = startMonitoredProcess(
        process.execPath,
        [path.join(directory, "scripts/run-fixture.mjs"), "--headless"],
        { cwd: directory },
      );
      await launcher.spawned;
      const owned = await waitFor(
        () => {
          const socket = launcher.capturedStandardOutput.match(
            /Fixture publisher listening at (.+)/,
          )?.[1];
          const renderer = launcher.capturedStandardOutput.match(
            /stubborn renderer=(\d+)/,
          )?.[1];
          assert.ok(socket && renderer);
          return { runtime: path.dirname(socket), renderer };
        },
        launcher,
        "unresponsive Renderer startup",
      );
      runtime = owned.runtime;
      childPids = (
        await readFile(
          `/proc/${launcher.pid}/task/${launcher.pid}/children`,
          "utf8",
        )
      )
        .trim()
        .split(/\s+/)
        .map(Number);
      await stopProcess(launcher);
      const [code] = await waitForProcessExit(launcher, {
        timeoutMilliseconds: 7_000,
      });
      assert.equal(code, 130, launcher.capturedStandardError);
      assert.match(launcher.capturedStandardError, /cancelled by SIGTERM/);
      await assert.rejects(access(owned.runtime), { code: "ENOENT" });
      assert.throws(() => process.kill(Number(owned.renderer), 0), {
        code: "ESRCH",
      });
    } finally {
      if (launcher?.exitCode === null) {
        launcher.kill("SIGTERM");
        await waitForProcessExit(launcher, { timeoutMilliseconds: 7_000 });
      }
      await stopProcesses([launcher]);
      for (const pid of childPids) {
        try {
          process.kill(-pid, "SIGKILL");
        } catch (error) {
          assert.equal(error.code, "ESRCH");
        }
      }
      if (runtime) await rm(runtime, { recursive: true, force: true });
      await rm(directory, { recursive: true, force: true });
    }
  },
);
