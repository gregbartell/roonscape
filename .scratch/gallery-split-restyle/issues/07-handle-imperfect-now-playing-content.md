# 07 — Keep Gallery split truthful under imperfect content

**What to build:** Keep the Gallery split stable, readable, and truthful when
real Roon data is missing, unusually long, indeterminate, or shaped unlike the
reference fixture.

**Blocked by:** 05 — Derive the complete presentation palette from artwork.

**Status:** ready-for-agent

- [ ] Metadata without usable artwork retains the Gallery split with a quiet
      square artwork field and no placeholder label or icon.
- [ ] Non-square artwork is displayed completely and centered within the
      square field without cropping or changing the composition geometry.
- [ ] Missing Artist or Album values disappear cleanly without placeholders or
      dead spacing.
- [ ] Long values wrap and reduce through viewport-scaled readable sizes before
      ellipsizing at maximums of three Title lines, two Artist lines, and two
      Album lines; no marquee or indefinite shrinking is introduced.
- [ ] Ordinary Output and Zone names remain comfortably within the footer,
      while unexpectedly long names defensively ellipsize on one line without
      moving the row.
- [ ] Indeterminate or live content omits the complete timeline, while valid
      determinate content retains both elapsed and negative remaining time.
- [ ] Shared edge-case fixtures and layout-policy tests cover missing fields,
      missing and non-square artwork, long and extreme metadata, long
      identities, and determinate versus indeterminate progress.
