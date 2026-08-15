# 04 — Restyle complete Now Playing in Gallery split

**What to build:** Present complete Now Playing content as the selected Gallery
split composition: a deliberate album-sleeve object on the left and a calm,
editorial metadata column on the right, all within one cohesive field.

**Blocked by:** 02 — Publish the selected fixture reference; 03 — Ship
preferred and fallback typography.

**Status:** ready-for-agent

- [ ] Artwork occupies a restrained square field with generous breathing room,
      contained image fit, and prototype-like depth instead of filling an
      opaque left panel.
- [ ] The metadata column presents explicit playback status, dominant Title,
      supporting Artist and Album, and determinate progress in the prototype's
      visual order and hierarchy.
- [ ] Determinate progress retains elapsed and negative remaining time;
      Playing advances locally while Paused and Loading remain frozen.
- [ ] Authoritative Output and Zone names occupy one stable bottom-right row in
      Playing, Paused, and content-bearing Loading presentations.
- [ ] The cohesive field replaces the conspicuous two-panel split without
      adding persistent branding, controls, browser UI, or ambient animation.
- [ ] Layout and typography scale around 3840×2160 while remaining credible
      without letterboxing at 3840×2400 and in the 1600×900 windowed fixture.
- [ ] Fixture and live snapshots render through the same native GTK/Pango view,
      and layout-policy tests assert visible proportions and roles rather than
      private widget nesting.
