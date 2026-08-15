# 07 — Protect the OLED during inactivity

**What to build:** Reduce prolonged bright, static content without obscuring
active music or hiding useful explanations too early. Paused, stopped, and
unavailable presentations enter a restrained inactive treatment after a grace
period, while resumed playback immediately restores the full composition.

**Blocked by:** 04 — Present live Now Playing and artwork; and 05 — Make
playback and progress truthful.

**Status:** ready-for-agent

- [ ] Paused playback freezes progress and initially retains the complete Now
  Playing composition.
- [ ] After a configurable grace period, paused content dims and periodically
  repositions within safe layout bounds.
- [ ] Stopped, pairing-required, disconnected, and output-unavailable states
  explain their condition before receiving the corresponding inactive
  treatment.
- [ ] Returning to Playing immediately restores full luminance and the normal
  Gallery split position.
- [ ] Inactive timing, dimness, and reposition cadence remain configurable on
  the RoonScape Host for later calibration without adding a settings screen.
- [ ] Deterministic tests exercise grace periods, repeated repositioning,
  state changes during inactivity, and immediate active restoration.
