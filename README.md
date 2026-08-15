# RoonScape

RoonScape is an unattended, read-only presentation of current Roon playback.

> **Status:** The shared contract, fixture workflow, live Roon availability,
> deterministic Display Output selection, and live Now Playing metadata and
> artwork are available.

RoonScape observes one configured physical Display Output, follows the Display
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

Run every formatter check, linter, typecheck, and automated test used by CI
with:

```sh
npm run check
```

## Live Roon setup

The bridge registers as `io.roonscape.bridge` and uses Roon's normal extension
authorization flow. Discover the physical outputs visible to Roon from the
RoonScape Host:

```sh
npm run configure -- list
```

On a fresh installation, enable RoonScape under **Settings → Extensions** in a
Roon client while the command waits for Roon. The list includes the internal
output ID needed by the host workflow, the Display Output name, and its current
Display Zone. Save one selection without changing Roon playback:

```sh
npm run configure -- select 'display-output-id-from-the-list'
```

Display Configuration is stored at
`$XDG_CONFIG_HOME/roonscape/display.json`, falling back to
`~/.config/roonscape/display.json`. Set `ROONSCAPE_DISPLAY_CONFIG` to choose
another dedicated file. Start or restart the bridge after changing the
selection.

Start the bridge with a private runtime directory and local socket:

```sh
mkdir -p "$XDG_RUNTIME_DIR/roonscape"
chmod 700 "$XDG_RUNTIME_DIR/roonscape"
export ROONSCAPE_SOCKET="$XDG_RUNTIME_DIR/roonscape/roonscape.sock"
npm run start:bridge
```

Start the renderer with the same `ROONSCAPE_SOCKET` value in the graphical
session. The bridge follows the selected physical Display Output when Roon
groups, ungroups, or renames its Display Zone. If the configuration is absent
or invalid, or Roon removes the selected output, the viewer reports that the
Display Output is unavailable instead of following another zone.

Current artwork is requested from Roon as a bounded JPEG derivative and staged
atomically in an `artwork` directory beside the socket. Each complete snapshot
identifies artwork by its presentation revision. Superseded files are removed;
the renderer derives a readable presentation palette from the current file and
uses a neutral palette when no artwork is present.

Roon authorization is stored separately at
`$XDG_STATE_HOME/roonscape/authorization.json`, falling back to
`~/.local/state/roonscape/authorization.json`. Set
`ROONSCAPE_AUTHORIZATION_FILE` to choose another dedicated file. This state is
independent from Display Configuration. The live bridge observes Roon's Image
and Transport services and provides extension Status; it does not load Browse,
call a Roon Control method, expose a command endpoint, provide a browser UI, or
open a network listener.
