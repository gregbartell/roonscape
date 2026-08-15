# RoonScape

RoonScape is an unattended, read-only presentation of current Roon playback.

> **Status:** Design is complete; implementation has not started.

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
