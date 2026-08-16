# RoonScape

RoonScape is an unattended, read-only presentation of current Roon playback.

> **Status:** The shared contract and fixtures, live Roon availability,
> deterministic Tracked Output selection, live Now Playing metadata and
> artwork, truthful playback progress, bounded metadata layout, and coordinated
> presentation transitions, OLED-safe inactivity treatment, independent IPC
> recovery, and optional local diagnostics are available.

RoonScape observes one configured physical Tracked Output, follows the Tracked
Zone that currently contains it, and presents current artwork, metadata,
playback state, and progress on an attached display. It deliberately provides
no Roon Control, browser interface, or network command surface.

The planned runtime has two independently supervised processes: a small
TypeScript/Node.js bridge using Roon's supported JavaScript extension API and a
native Rust renderer using GTK 4 and Pango. They exchange complete, versioned
state snapshots over a private Unix-domain socket and pass artwork through
bounded local files.

RoonScape is intended for compatible Linux/GTK hosts. Its initial Reference
Deployment is an Intel NUC driving a 4K OLED television, but that machine's
identity and its co-located Roon Server are not product requirements.

## Project documentation

- [Specification](.scratch/roonscape/spec.md)
- [Implementation tickets](.scratch/roonscape/issues/)
- [Domain language](CONTEXT.md)
- [Architecture decisions](docs/adr/)
- [Selected visual direction](docs/design/gallery-split.md)

## Playing fixture

The fixture workflow needs Node.js 24.19.0, npm 11.17.0, Rust 1.97.1, and the
GTK 4 development libraries. Toolchain versions are pinned in `.node-version`,
`package.json`, and `rust-toolchain.toml`.

Install dependencies once with `npm install`, then launch the TypeScript
fixture publisher and native renderer together from a graphical session:

```sh
npm run fixture
```

Fixture mode creates a private temporary Unix-domain socket, sends the shared
Playing snapshot when the renderer connects, and removes its runtime directory
when the renderer exits. Set `ROONSCAPE_WINDOWED=1` before the command when a
window is more convenient than the default fullscreen presentation.

For repeatable native-renderer captures at the three accepted viewports, use
the [Gallery split visual acceptance workflow](docs/visual-acceptance/gallery-split.md).
It covers the complete fixture matrix, typography and diagnostics
representatives, the decision checklist, and the physical-display handoff.

The renderer uses the host-provided Palatino Linotype and Segoe UI families
only when both are available. Otherwise it atomically selects the packaged
Libre Baskerville and IBM Plex Sans fallback pair. The open fallback files and
their license notices live under `renderer/assets/fonts/`; renderer startup
registers them privately without a network request or global font install.

Run every formatter check, linter, typecheck, and automated test used by CI
with:

```sh
npm run check
```

Run the headless local IPC restart exercise on its own with:

```sh
npm run smoke:ipc
```

It starts the native IPC client before the bridge, kills and restarts the
bridge, then restarts the client while the bridge remains live. The check
confirms current-state replay and a truthful Disconnected presentation between
connections.

## Live Roon setup

The bridge registers as `io.roonscape.bridge` and uses Roon's normal extension
authorization flow. Discover the physical outputs visible to Roon from the
RoonScape Host:

```sh
npm run configure -- list
```

On a fresh installation, enable RoonScape under **Settings → Extensions** in a
Roon client while the command waits for Roon. The list includes the internal
Tracked Output ID needed by the host workflow, the Tracked Output name, and its
current Tracked Zone. Save one selection without changing Roon playback:

```sh
npm run configure -- select 'tracked-output-id-from-the-list'
```

OLED protection defaults to a 300-second grace period, 35% opacity, and a new
bounded Gallery split position every 60 seconds. Calibrate those values on the
RoonScape Host without a settings screen:

```sh
npm run configure -- inactivity 300 0.35 60
```

The arguments are grace-period seconds, dimmed opacity greater than zero and
less than one, and reposition-cadence seconds. Display Configuration requires
`trackedOutputId`; the removed `displayOutputId` field is intentionally invalid
and is not migrated. Changing the Tracked Output preserves any saved inactivity
calibration.

Display Configuration is stored at
`$XDG_CONFIG_HOME/roonscape/display.json`, falling back to
`~/.config/roonscape/display.json`. Set `ROONSCAPE_DISPLAY_CONFIG` to choose
another dedicated file. Start or restart the bridge after changing the
selection. Restart both the bridge and renderer after changing inactivity
calibration. The renderer reads inactivity calibration from the same file and
falls back to the defaults when it is absent or invalid.

Start the bridge with a private runtime directory and local socket:

```sh
mkdir -p "$XDG_RUNTIME_DIR/roonscape"
chmod 700 "$XDG_RUNTIME_DIR/roonscape"
export ROONSCAPE_SOCKET="$XDG_RUNTIME_DIR/roonscape/roonscape.sock"
npm run start:bridge
```

Start the renderer with the same `ROONSCAPE_SOCKET` value in the graphical
session. Either process may start first: the renderer keeps its graphical
session open, presents Disconnected without stale Now Playing content, and
retries the local socket until the bridge is available. A replacement bridge
reclaims a stale socket left by an abruptly terminated predecessor, and every
renderer connection receives its current complete snapshot immediately.

The bridge follows the selected physical Tracked Output when Roon groups,
ungroups, or renames its Tracked Zone. If the configuration is absent or
invalid, or Roon removes the selected output, the viewer reports that the
Tracked Output is unavailable instead of following another zone.

Current artwork is requested from Roon as a bounded JPEG derivative and staged
atomically in an `artwork` directory beside the socket. Each complete snapshot
identifies artwork by its presentation revision. Superseded files are removed;
the renderer derives a readable presentation palette from the current file and
uses the fixed navy, coral, and cream fallback when artwork is absent or
unusable. Metadata wraps within fixed line counts, reduces to readable minimum
sizes, and ellipsizes only when it still cannot fit. Visual revision changes
crossfade artwork, metadata, and both layers' full palettes together while
progress-only samples update in place.

Roon authorization is stored separately at
`$XDG_STATE_HOME/roonscape/authorization.json`, falling back to
`~/.local/state/roonscape/authorization.json`. Set
`ROONSCAPE_AUTHORIZATION_FILE` to choose another dedicated file. This state is
independent from Display Configuration. The live bridge observes Roon's Image
and Transport services and provides extension Status; it does not load Browse,
call a Roon Control method, expose a command endpoint, provide a browser UI, or
open a network listener.

## Optional diagnostics

Diagnostics are absent by default. Set `ROONSCAPE_DIAGNOSTICS=1` (or `true`) in
the renderer's host environment to add a compact local overlay reporting its
resident memory, frame timing, artwork dimensions, bridge connection state,
and latest received state revision. Set it to `0` or `false`, or leave it
unset, for the normal viewer-facing presentation. The overlay observes the
same bounded snapshot pipeline and does not modify presentation state or add a
network endpoint.
