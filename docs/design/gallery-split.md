# Gallery Split

Variant A, **Gallery split**, was selected as RoonScape's visual direction on
2026-08-14. The complete five-variant throwaway prototype is preserved on the
`prototype/visual-variants` branch; it is prior art, not production source.

Preserve its asymmetric album-sleeve composition and dedicated metadata
column, with these corrections:

- The Display Output is internal configuration and does not appear onscreen.
- Present the Display Zone name under the viewer-facing label **Zone**.
- Derive the entire presentation palette, including text, from the current
  album artwork; the prototype's fixed palette is illustrative only. Select
  readable combinations from that palette, but do not constrain primary text
  to a permanent product color unless testing proves full recoloring too
  garish.
- Wrap and reduce long metadata within firm line and minimum-size bounds; do
  not use a perpetual marquee.
- On the OLED television, keep the paused composition briefly, then dim and
  periodically reposition it. Calibrate the dim level on the physical screen
  rather than declaring an untested luminance in the prototype.
- Crossfade artwork, typography, and the derived palette together on track
  changes; keep all other motion limited to progress and OLED protection.
- Preserve the editorial serif Title/Album treatment paired with clean sans
  serif utility text, subject to final legibility and glyph-coverage testing.
- Do not place persistent RoonScape branding on the television.

The final typography, overflow behavior, transitions, and OLED-safe inactive
states remain implementation details. Do not promote the prototype directly
into the native renderer.
