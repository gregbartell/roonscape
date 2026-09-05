import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  assertProcessRunning,
  runMonitoredProcess,
  startMonitoredProcess,
  startXvfbDisplay,
  stopProcesses,
  waitFor,
} from "./process-harness.mjs";

// Verification owns a bus as well as a display: GTK application registration
// is scoped to D-Bus, not DISPLAY. Do not change the Live Mode application ID.
export async function createNativeSession({
  width,
  height,
  environment = process.env,
  signal,
} = {}) {
  const runtimeDirectory = await mkdtemp(path.join(tmpdir(), "rs-native."));
  const configurationPath = path.join(runtimeDirectory, "display.json");
  const infrastructureProcesses = [];
  const applicationProcesses = [];
  const isolated = { ...environment };
  for (const name of [
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "SESSION_MANAGER",
    "DBUS_SESSION_BUS_ADDRESS",
    "DBUS_SESSION_BUS_PID",
    "DBUS_STARTER_ADDRESS",
    "DBUS_STARTER_BUS_TYPE",
    "AT_SPI_BUS_ADDRESS",
    "GTK_MODULES",
    "GTK_PATH",
    "ROONSCAPE_DISPLAY_CONFIG",
    "ROONSCAPE_CAPTURE_CONTROL",
    "ROONSCAPE_CAPTURE_VIEWPORT",
    "ROONSCAPE_CAPTURE_TYPOGRAPHY",
    "ROONSCAPE_FIXTURE_AUTO_CLOSE_MS",
    "ROONSCAPE_FIXTURE_CONTROL",
    "ROONSCAPE_FIXTURE",
    "ROONSCAPE_FIXTURE_CATALOG",
    "ROONSCAPE_SOCKET",
    "ROONSCAPE_WINDOWED",
    "ROONSCAPE_STATIC_FIXTURE",
    "ROONSCAPE_DIAGNOSTICS",
    "ROONSCAPE_TEST_FIRST_REVEALED_PAINT_CONTROL",
  ])
    delete isolated[name];

  try {
    for (const name of [
      "HOME",
      "XDG_CONFIG_HOME",
      "XDG_CACHE_HOME",
      "XDG_DATA_HOME",
      "XDG_STATE_HOME",
      "XDG_RUNTIME_DIR",
    ]) {
      isolated[name] = path.join(runtimeDirectory, name.toLowerCase());
      await mkdir(isolated[name], { mode: 0o700 });
    }
    Object.assign(isolated, {
      GDK_BACKEND: "x11",
      GDK_SCALE: "1",
      GDK_DPI_SCALE: "1",
      NO_AT_BRIDGE: "1",
      GTK_A11Y: "none",
      GTK_USE_PORTAL: "0",
      GSETTINGS_BACKEND: "memory",
    });
    await writeFile(
      configurationPath,
      JSON.stringify({
        trackedOutputId: "native-verification",
        inactivity: {
          gracePeriodSeconds: 3600,
          dimmedOpacity: 0.35,
          repositionCadenceSeconds: 60,
        },
      }),
      { mode: 0o600 },
    );
    // No service directories: this bus cannot activate desktop services from
    // the host session or leave unmonitored activation children behind.
    const busConfiguration = path.join(runtimeDirectory, "bus.conf");
    await writeFile(
      busConfiguration,
      `<busconfig><type>session</type><listen>unix:tmpdir=${runtimeDirectory}</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>`,
      { mode: 0o600 },
    );
    const bus = startMonitoredProcess(
      "dbus-daemon",
      ["--nofork", "--print-address=1", "--config-file", busConfiguration],
      { environment: isolated },
    );
    infrastructureProcesses.push(bus);
    await bus.spawned;
    isolated.DBUS_SESSION_BUS_ADDRESS = await waitFor(
      () => {
        const address = bus.capturedStandardOutput.trim();
        if (!address.startsWith("unix:"))
          throw new Error("private session bus has not reported its address");
        return address;
      },
      bus,
      "private native session bus",
      { signal },
    );
    const { display, xvfb } = await startXvfbDisplay({
      width,
      height,
      environment: isolated,
      signal,
    });
    infrastructureProcesses.push(xvfb);
    isolated.DISPLAY = display;
    return {
      runtimeDirectory,
      configurationPath,
      environment: isolated,
      startProcess(command, arguments_, overrides = {}) {
        signal?.throwIfAborted();
        for (const child of infrastructureProcesses)
          assertProcessRunning(child, "native session process");
        const child = startMonitoredProcess(command, arguments_, {
          environment: { ...isolated, ...overrides },
        });
        applicationProcesses.push(child);
        return child;
      },
      async close(failure) {
        // Stop application clients before their display and bus.
        try {
          await stopProcesses(applicationProcesses, { failure, signal });
        } finally {
          await stopProcesses(infrastructureProcesses, { failure, signal });
          await rm(runtimeDirectory, { recursive: true, force: true });
        }
      },
    };
  } catch (error) {
    await stopProcesses(infrastructureProcesses, { failure: error, signal });
    await rm(runtimeDirectory, { recursive: true, force: true });
    throw error;
  }
}

export function waitForNativeWindow(
  renderer,
  environment,
  width,
  height,
  { signal } = {},
) {
  return waitFor(
    async () => {
      const output = await runMonitoredProcess(
        "xwininfo",
        ["-name", "RoonScape", "-int"],
        {
          environment,
          signal,
          timeoutMilliseconds: 1000,
        },
      );
      if (
        !new RegExp(`Width: ${width}\\b`).test(output) ||
        !new RegExp(`Height: ${height}\\b`).test(output) ||
        !output.includes("Map State: IsViewable")
      ) {
        throw new Error(
          `native RoonScape window has not reached a mapped ${width}x${height} viewport\n${output}`,
        );
      }
      return output;
    },
    renderer,
    `native RoonScape window at ${width}x${height}`,
    { signal, timeoutMilliseconds: 5000 },
  );
}
