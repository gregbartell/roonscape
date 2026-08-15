# 12 — Prevent artwork flicker on same-track Roon updates

**What to build:** Keep the current artwork visually stable when Roon updates
the Display Zone without changing the artwork identity. Timing refreshes,
pause and resume changes, volume changes, and other non-artwork updates must
not make the artwork disappear and return or trigger unnecessary presentation
transitions.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [x] A Display Zone update that retains the same Roon artwork identity also
      retains the current artwork reference without publishing an intermediate
      missing-artwork snapshot.
- [x] Same-track timing refreshes do not request the same artwork again or
      trigger an artwork transition.
- [x] Pausing and resuming update playback and progress truthfully while
      retaining current artwork without flicker.
- [x] Volume-only and other non-presented Display Zone changes do not refresh
      artwork or disturb the current presentation.
- [x] A genuine artwork identity change still requests the new artwork once,
      publishes it under a new presentation revision, and follows the normal
      bounded transition and cleanup behavior.
- [x] Regression tests exercise timing, pause and resume, and volume-triggered
      source events at the bridge-to-renderer state seam and assert both the
      snapshot sequence and artwork-request count.
