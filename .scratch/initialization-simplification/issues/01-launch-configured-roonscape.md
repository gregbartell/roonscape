# 01 — Launch configured RoonScape as one secure session

**What to build:** Give an owner with a valid Display Configuration one
foreground `roonscape` command that starts the bridge and renderer as a secure,
single-owner runtime session and reports the session's real outcome.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Bare `roonscape` and the source `npm start` workflow launch immediately
      when the selected Display Configuration is valid, without exposing the
      bridge, renderer, socket, or process boundary to the owner.
- [x] `--config PATH` takes precedence over the standard XDG Display
      Configuration location, and the removed Display Configuration environment
      override is rejected rather than retained as a second public override.
- [x] Roon Authorization defaults to the XDG state location independently of
      Display Configuration, and a valid Display Configuration without existing
      authorization launches into the pairing-required presentation.
- [x] `--help` and `--version` describe the complete public command surface;
      unsupported arguments fail with actionable usage and a nonzero result.
- [x] The launcher creates private runtime state beneath the owner's XDG runtime
      directory, or a validated `/run/user/<uid>` equivalent, and fails with
      remediation rather than falling back to persistent storage or predictable
      temporary paths.
- [x] A live RoonScape session excludes a second invocation, while stale runtime
      artifacts are reclaimed only after their former owner is known to be gone.
- [x] The bridge and renderer form one runtime session: either child ending stops
      its peer, launcher termination stops both, and a child that does not stop
      within five seconds is forcibly terminated.
- [x] Normal renderer closure returns success, while a crash, signal, or nonzero
      child result remains observable through the launcher result; runtime state
      is cleaned up in every completed shutdown path.
- [x] Focused bridge and fixture workflows remain available, and command-level
      tests cover configuration selection, runtime ownership, process ordering,
      signals, result propagation, and cleanup through injected system adapters.

## Comments

### Implemented — 2026-08-15

Implemented in `4596f87`, with review fixes in `e424edd`, `9df857f`, and
`638359e`. The final Standards and Spec reviews reported no findings, and
`npm run check` passed.
