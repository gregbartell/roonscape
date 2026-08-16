# 04 — Build and verify a relocatable Linux release

**What to build:** Give an owner one relocatable Linux archive that runs the
finished `roonscape` command without installing source toolchains or building
either runtime process.

**Blocked by:** 03 — Reconfigure Display Configuration safely.

**Status:** ready-for-agent

- [ ] One reproducible source command builds a versioned x86-64 archive and
      checksum for glibc-based Linux systems with an Ubuntu 22.04-era baseline
      and GTK 4.6 or newer as the only pre-existing application runtime
      dependency.
- [ ] The archive contains an executable top-level `roonscape` wrapper, the
      private pinned Node runtime, compiled command and bridge JavaScript,
      production JavaScript dependencies, the native renderer, schemas, fonts,
      licenses, and every required runtime asset.
- [ ] The extracted archive runs as `./roonscape` from an arbitrary location
      without installation or root access and resolves all bundled components
      relative to itself rather than the source checkout.
- [ ] Replacing the release directory leaves Display Configuration and Roon
      Authorization intact in their persistent XDG locations.
- [ ] Source development exposes `npm run setup`, `npm start`, and
      `npm run package` through the same production command behavior while
      preserving focused bridge, fixture, check, and test workflows.
- [ ] The packaging check unpacks the archive, verifies its contents, executable
      permissions, and checksum, then runs `./roonscape --help` and
      `./roonscape --version` with system Node, npm, Cargo, and Rust excluded
      from command discovery.
- [ ] Packaging verification does not claim a clean-host GTK or live Roon
      session; established renderer, fixture, snapshot, and IPC checks remain
      responsible for their existing behavior.
- [ ] The archive and packaging command make no compatibility promise for ARM64,
      musl, non-Linux systems, older GTK releases, or native package formats.
