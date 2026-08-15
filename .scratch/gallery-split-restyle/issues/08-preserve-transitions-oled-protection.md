# 08 — Preserve transitions and OLED protection

**What to build:** Make the restyled presentation behave as calmly and safely
as the existing renderer during track, state, palette, and inactivity changes,
without stale content, resource growth, or new perpetual motion.

**Blocked by:** 06 — Restyle trackless states as full-field presentations; 07
— Keep Gallery split truthful under imperfect content.

**Status:** ready-for-agent

- [ ] Changes to availability, playback, identities, metadata, progress
      presence, artwork, or palette replace the presentation through one
      coordinated crossfade of artwork, text, and the complete color system.
- [ ] Progress-value-only updates remain in place and same-track Roon updates
      do not refresh artwork, flicker, or trigger unnecessary transitions.
- [ ] Disconnection and unavailable snapshots clear stale Now Playing content
      immediately rather than preserving it for visual continuity.
- [ ] Rapid revisions retain only the current and one outgoing presentation,
      then release the outgoing view and artwork after the bounded transition.
- [ ] Paused, Idle, and unavailable states preserve configured grace, dimming,
      and periodic repositioning; Loading remains active and Playing restores
      full opacity and normal position immediately.
- [ ] The larger artwork field, shadows, full-field copy, footer, and
      diagnostics remain within safe bounds at every configured OLED offset.
- [ ] Existing transition and inactivity tests are extended across Gallery
      split, full-field, light-palette, and missing-content presentations, with
      no new motion beyond progress, crossfade, and OLED protection.
