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

The runtime uses a small TypeScript/Node.js bridge for Roon's supported
JavaScript extension API and a native Rust renderer built with GTK 4 and Pango.
One foreground RoonScape command supervises them as a single session. They
exchange complete, versioned state snapshots over a private Unix-domain socket
and pass artwork through bounded local files.

RoonScape is intended for compatible Linux/GTK hosts. Its initial Reference
Deployment is an Intel NUC driving a 4K OLED television, but that machine's
identity and its co-located Roon Server are not product requirements.

## Project documentation

- [Specification](.scratch/roonscape/spec.md)
- [Implementation tickets](.scratch/roonscape/issues/)
- [Domain language](CONTEXT.md)
- [Architecture decisions](docs/adr/)
- [Presentation design](docs/design/presentation.md)

## Install a Linux release

Download the versioned archive and matching `.sha256` file from the repository's
[Releases](../../releases) page. Set `VERSION` to the version you downloaded,
then verify and extract it anywhere your user can write:

```sh
VERSION=0.1.0
sha256sum --check "roonscape-${VERSION}-linux-x64.tar.gz.sha256"
tar --extract --gzip --file "roonscape-${VERSION}-linux-x64.tar.gz"
cd "roonscape-${VERSION}-linux-x64"
```

The release is a relocatable directory, not an installer. It includes the
required Node.js runtime, production JavaScript dependencies, and precompiled
renderer, so running it does not require Node, npm, Rust, Cargo, a compiler, or
GTK development libraries. The RoonScape Host must provide the GTK 4.6 or newer
runtime.

The initial release target is x86-64 Linux with glibc and an Ubuntu 22.04-era
compatibility baseline. ARM64, musl-based Linux, older GTK releases, other
operating systems, and other Linux distributions or graphical arrangements are
unverified; the archive does not imply a broad distribution support guarantee.

## First-time and normal use

Start RoonScape from a graphical session:

```sh
./roonscape
```

On first use, the terminal wizard waits for Roon Authorization, guides you to
enable RoonScape under **Settings → Extensions** in a Roon client, lets you
choose a Tracked Output, and offers OLED inactivity settings. Completing the
wizard writes Display Configuration and continues directly into the
presentation. Later invocations with valid configuration launch immediately.

Use the explicit setup command to change the Tracked Output or OLED settings.
It saves the completed Display Configuration and exits without launching the
presentation:

```sh
./roonscape --setup
```

A prepared Display Configuration can be selected noninteractively with
`./roonscape --config PATH`. Use `./roonscape --help` to see the complete public
command surface and `./roonscape --version` to identify the extracted release.

Display Configuration is stored at
`$XDG_CONFIG_HOME/roonscape/display.json`, falling back to
`~/.config/roonscape/display.json`. Roon Authorization is independent state at
`$XDG_STATE_HOME/roonscape/authorization.json`, falling back to
`~/.local/state/roonscape/authorization.json`.

## Update a release

Stop the foreground command, download and verify the new archive, and extract
it as a new release directory. Point your normal launch command at that new
directory and start it; after verifying the update, the old release directory
can be removed. Display Configuration and Roon Authorization remain in their
XDG locations outside both release directories. RoonScape has no automatic
updater and does not modify an existing release directory in place.

## Launch RoonScape unattended

`roonscape` remains in the foreground and supervises its bridge and renderer as
one runtime session. It does not daemonize, restart itself, know about `startx`,
or require a particular service manager. An owner may launch or supervise that
one command with the boot integration appropriate to the RoonScape Host.

The Reference Deployment uses a guarded tty1 `startx` graphical session to
launch the command. Its deployment profile—not the RoonScape runtime—owns any
failure-recovery policy and must respect a successful intentional exit.

## Source development

Source workflows need Node.js 24.19.0, npm 11.17.0, Rust 1.97.1, and the GTK 4
development libraries. Toolchain versions are pinned in `.node-version`,
`package.json`, and `rust-toolchain.toml`. Install dependencies once:

```sh
npm install
```

Use the production-equivalent commands for ordinary setup, launch, and
packaging:

```sh
npm run setup
npm start
npm run package
```

`npm run setup` builds and runs explicit reconfiguration, then exits. `npm
start` builds and launches one foreground session. `npm run package` is the
sole release-packaging implementation and writes the versioned archive and
checksum under `release/`.

For focused presentation work, `npm run fixture` launches the TypeScript
fixture publisher and native renderer together. An ordinary Fixture Mode
session starts at Playing. With the renderer window focused, Right selects the
next Fixture Scenario and Left selects the previous one; both directions wrap,
and holding an arrow advances only once until it is released. The terminal
names the initial and subsequently selected Fixture Scenarios without adding
anything onscreen.

The catalog order is Playing, Paused, Loading with content, Loading without
content, Idle, pairing required, disconnected, output unavailable, Playing
without content, missing metadata, missing Artist, missing Album, missing
artwork, long metadata, extreme metadata, indeterminate progress, non-square
artwork, and light artwork. Set `ROONSCAPE_WINDOWED=1` when a window is more
convenient than fullscreen. Setting `ROONSCAPE_FIXTURE` retains the focused
single-fixture workflow and intentionally disables arrow navigation.

For focused bridge work, `npm run start:bridge` remains available when
`ROONSCAPE_SOCKET` names a developer-managed private socket. The lower-level
`npm run configure -- ...` commands remain available for discovery and
configuration diagnostics, but are not the ordinary owner setup path.

For repeatable native-renderer captures at the five representative viewports,
use the [presentation visual-acceptance workflow](docs/visual-acceptance/presentation.md).
It covers the complete Fixture Scenario matrix, typography and diagnostics
representatives, and the decision checklist.

The renderer uses the host-provided Palatino Linotype and Segoe UI families
only when both are available. Otherwise it atomically selects the packaged
Libre Baskerville and IBM Plex Sans fallback pair. The open fallback files and
their license notices live under `src/renderer/assets/fonts/`; renderer startup
registers them privately without a network request or global font install.

Run every formatter check, linter, typecheck, and automated test used by CI with
`npm run check`. Run the focused headless IPC restart exercise with `npm run
smoke:ipc`.

## Publish a release

Set the intended version in `package.json` and `package-lock.json`, commit it,
and run `npm run check` and `npm run package` locally. Create and push a tag
whose exact name is `v<package-version>`. For example, package version `0.1.0`
must use tag `v0.1.0`.

The tag-triggered workflow rejects a mismatched tag before packaging. For a
matching tag it invokes `npm run package` and creates a normal GitHub Release
containing that command's versioned archive and SHA-256 checksum. The workflow
does not maintain a second staging or checksum implementation.

## Runtime details

The launcher privately owns the socket, singleton lock, runtime cleanup, and
both child lifetimes. The bridge follows the selected physical Tracked Output
when Roon groups, ungroups, or renames its Tracked Zone. If Roon removes that
output, the viewer reports that the Tracked Output is unavailable instead of
following another zone.

Current artwork is requested from Roon as a bounded JPEG derivative and staged
atomically in an `artwork` directory beside the socket. Each complete snapshot
identifies artwork by its presentation revision. Superseded files are removed;
the renderer derives a readable presentation palette from the current file and
uses the fixed navy, coral, and cream fallback when artwork is absent or
unusable. Metadata wraps within fixed line counts, reduces to readable minimum
sizes, and ellipsizes only when it still cannot fit. Visual revision changes
crossfade artwork, metadata, and both layers' full palettes together while
progress-only samples update in place.

Roon Authorization is stored separately at
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
