# 14 — Navigate Fixture Scenarios with arrows

**What to build:** Let a person inspecting an ordinary Fixture Mode session use
Left and Right in the focused renderer window to cycle backward and forward
through all 18 Fixture Scenarios. Navigation should preserve the real
presentation path and transitions, remain deterministic across repeated and
rapid selections, and have no effect in Live Mode or an explicit single-fixture
session.

**Blocked by:** 13 — Centralize the Fixture Scenario catalog.

**Status:** done

- [x] Ordinary Fixture Mode activates an explicit private navigation capability
      without making the existing presentation socket bidirectional, changing
      the presentation snapshot schema, or adding a network command surface.
- [x] With the renderer window focused, Right selects the next Fixture Scenario
      and wraps from light artwork to Playing; Left selects the previous Fixture
      Scenario and wraps from Playing to light artwork.
- [x] Every selection publishes the latest complete snapshot through the normal
      presentation path, uses the normal immediate-replacement or crossfade
      behavior, and retains no navigation event history.
- [x] Fixture Mode assigns monotonic session revisions and re-anchors Playing at
      its reference progress position whenever Playing is selected.
- [x] Every selection begins a fresh inactivity grace period without changing
      Live Mode progress or inactivity behavior.
- [x] Holding an arrow advances once until that key is released; distinct rapid
      presses remain responsive, and the latest deliberate selection wins.
- [x] The initial and subsequently selected Fixture Scenario names are logged
      in the terminal without adding an onscreen label, hint, or control.
- [x] Left and Right remain inert in Live Mode, in explicit
      `ROONSCAPE_FIXTURE` sessions, and when the renderer window lacks focus.
- [x] Escape, window-close behavior, windowed and fullscreen presentation,
      launcher cleanup, and the coordinated Fixture Mode process lifecycle
      remain unchanged.
- [x] Integration checks send Previous and Next through the Fixture Mode control
      boundary and observe the resulting complete snapshots, covering all 18
      scenarios, both wraparound directions, monotonic revisions, progress
      re-anchoring, and rapid navigation.
- [x] Toolkit-independent renderer checks cover Left and Right mapping,
      held-repeat suppression, fresh inactivity timing, Fixture Mode gating,
      inert Live Mode behavior, and unchanged Escape handling without asserting
      GTK internals.
- [x] Process-level launcher checks cover ordinary navigation activation and
      preservation of explicit single-fixture behavior.
- [x] A manual GTK smoke check covers focused navigation in windowed and
      fullscreen Fixture Mode, real crossfades, terminal labels, rapid
      selections, inactivity reset, and the absence of onscreen fixture
      controls; no pixel-golden or screenshot-difference gate is added.
- [x] Fixture Mode documentation explains the arrow controls, catalog order,
      terminal labels, and explicit single-fixture exception using canonical
      project vocabulary.

## Comments

### Native GTK smoke — 2026-08-16

- Exercised ordinary windowed Fixture Mode under Xvfb and actual 1920×1080
  fullscreen Fixture Mode under Xvfb with bspwm, using focused X key events.
  Right, Left, held-repeat suppression, rapid presses across all 18 Fixture
  Scenarios, terminal labels, Escape, and coordinated cleanup behaved as
  specified.
- Visually inspected midpoint and settled native captures for a real crossfade
  between light and non-square artwork. No Fixture Mode label, hint, or control
  appeared onscreen.
- With a one-second test inactivity calibration, Paused dimmed and repositioned,
  a new Paused selection restored the full presentation for a fresh grace
  period, and the presentation dimmed again after that period expired.
