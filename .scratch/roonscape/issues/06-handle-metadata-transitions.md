# 06 — Handle metadata and track transitions gracefully

**What to build:** Make Gallery split remain composed across incomplete,
unusually long, and rapidly changing Now Playing content. Metadata stays
legible without scrolling, and track changes feel coherent because artwork,
text, and palette transition as one presentation.

**Blocked by:** 04 — Present live Now Playing and artwork; and 05 — Make
playback and progress truthful.

**Status:** ready-for-agent

- [ ] Missing Artist or Album values remain absent without invented labels,
  placeholders, or broken spacing.
- [ ] Long metadata wraps and reduces within firm line and minimum-size bounds,
  then ellipsizes only extreme cases; no perpetual marquee is introduced.
- [ ] Title and Album use an editorial serif treatment while Artist, Zone,
  state, and timing use a clean sans-serif treatment with usable fallback
  glyph coverage.
- [ ] Artwork, Title, Artist, Album, and the full derived palette crossfade
  together when presentation revision changes.
- [ ] Transition state retains at most current artwork and one outgoing artwork
  and uses a small derivative for palette analysis.
- [ ] Apart from progress and later OLED protection, the completed presentation
  contains no perpetual ambient animation.
- [ ] Fixture-driven checks cover missing lines, very long metadata, rapid
  revision changes, and bounded transition cleanup.
