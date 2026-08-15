# RoonScape

RoonScape is an unattended, read-only presentation of current Roon playback.

> **Status:** The shared contract and Playing fixture workflow are available;
> live Roon integration and the remaining presentation states are still under
> development.

RoonScape observes one configured physical Roon output, follows the Roon zone
that currently contains it, and presents current artwork, metadata, playback
state, and progress on an attached display. It deliberately provides no Roon
Control, browser interface, or network command surface.

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
