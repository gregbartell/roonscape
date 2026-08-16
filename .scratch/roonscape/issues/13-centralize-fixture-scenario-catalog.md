# 13 — Centralize the Fixture Scenario catalog

**What to build:** Give ordinary Fixture Mode and repeatable visual-acceptance
captures one shared, ordered catalog of the 18 accepted Fixture Scenarios.
Ordinary Fixture Mode should validate the complete catalog before starting and
open predictably at Playing, while the existing explicit single-fixture
override retains its current behavior.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] One catalog defines Playing, Paused, Loading with content, Loading without
      content, Idle, pairing required, disconnected, output unavailable,
      Playing without content, missing metadata, missing Artist, missing Album,
      missing artwork, long metadata, extreme metadata, indeterminate progress,
      non-square artwork, and light artwork in that order.
- [x] Ordinary Fixture Mode loads and validates all 18 catalog entries before
      becoming available, starts at Playing, and does not persist a selection
      between sessions.
- [x] A missing or invalid catalog entry fails Fixture Mode clearly rather than
      starting with a partial catalog.
- [x] The visual-acceptance capture plan derives its matrix scenarios and order
      from the shared catalog without changing its accepted viewports,
      representative captures, or output naming.
- [x] Explicit `ROONSCAPE_FIXTURE` selection continues to load only the named
      fixture and does not require or alter the ordinary navigation catalog.
- [x] Automated checks observe ordinary startup, explicit single-fixture
      startup, catalog validation failure, and the unchanged capture plan at
      their existing public seams rather than asserting private helpers.
