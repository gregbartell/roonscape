# 01 — Show a Playing fixture in Gallery split

**What to build:** Provide a development workflow that carries one complete,
versioned Playing snapshot from a TypeScript fixture publisher over the local
process boundary into a Rust/GTK renderer. From the listener's perspective,
launching fixture mode produces a minimal but recognizable Gallery split with
artwork, Title, Artist, Album, Zone, explicit playback state, and determinate
progress.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] One documented command starts the TypeScript fixture publisher and the
  Rust/GTK renderer without requiring a live Roon Server.
- [x] Node.js and Rust toolchain versions are pinned, the selected Node package
  manager is declared, and Cargo and Node dependency lockfiles are committed.
- [x] One repository-level command runs formatting, linting, and automated
  tests for both modules; continuous integration invokes the same checks.
- [x] A language-neutral schema defines a complete snapshot with a schema
  version, monotonic revision, availability, playback, Display Zone, optional
  Now Playing values, progress, and artwork reference.
- [x] TypeScript and Rust both validate and consume the same Playing fixture,
  and each rejects a deliberately invalid fixture.
- [x] The snapshot crosses a private Unix-domain socket; fixture mode opens no
  network listener and sends the current snapshot when the renderer connects.
- [x] The renderer uses GTK 4 and Pango, embeds no browser engine, and presents
  dominant artwork beside a metadata column without exposing Roon Control.
- [x] Automated checks exercise the fixture-to-snapshot and
  snapshot-to-presentation boundaries rather than private implementation
  structure.
