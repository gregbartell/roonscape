# 02 — Guide first-time setup into the presentation

**What to build:** Make bare interactive `roonscape` guide an owner with no
Display Configuration through Roon Authorization and Tracked Output selection,
save a safe default configuration, and continue directly into the presentation.

**Blocked by:** 01 — Launch configured RoonScape as one secure session.

**Status:** done

- [x] Bare `roonscape` detects a missing Display Configuration only when the
      terminal is interactive; a noninteractive invocation fails promptly with
      actionable guidance and a nonzero result.
- [x] The wizard explains how to enable RoonScape in an official Roon client,
      waits for Roon Authorization without a deadline, and later presents
      troubleshooting guidance without discarding authorization progress.
- [x] Authorization waiting provides explicit Retry and Quit actions, and
      authorization continues to persist independently from Display
      Configuration.
- [x] The keyboard-driven chooser presents every Tracked Output with its current
      Tracked Zone, reveals internal identity only to disambiguate otherwise
      identical choices, and never invokes Roon Control.
- [x] An empty discovery result offers Refresh and Quit rather than saving an
      empty or guessed Tracked Output.
- [x] First-time setup offers the five-minute grace period, 35 percent dimmed
      opacity, and one-minute reposition cadence as defaults that can be accepted
      together without schema knowledge.
- [x] Completion validates and atomically writes a private Display Configuration
      before launching the same runtime session as an ordinary configured start;
      quitting or failing setup leaves no partial configuration.
- [x] A malformed interactive Display Configuration is reported before repair is
      offered, while a malformed noninteractive configuration fails without
      entering an invisible wizard.
- [x] Command-level tests cover authorization timing and actions, Tracked Output
      selection and disambiguation, empty discovery, cancellation, atomic save,
      and continuation into launch through the owner-facing command seam.

## Comments

### Implemented — 2026-08-15

Implemented in `fe6ad5f`, with review cleanup in `8215f85`. The final Standards
and Spec reviews reported no remaining findings. `npm run typecheck`,
`npm run lint`, and the full `npm test` suite passed.
