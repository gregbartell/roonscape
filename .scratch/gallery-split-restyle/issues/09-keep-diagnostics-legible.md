# 09 — Keep diagnostics legible on every palette

**What to build:** Preserve the opt-in troubleshooting surface as a quiet,
readable overlay that works with the restyled light and dark presentations
without becoming part of the normal unattended display.

**Blocked by:** 05 — Derive the complete presentation palette from artwork.

**Status:** ready-for-agent

- [ ] Diagnostics remain absent by default and appear only through the existing
      explicit environment opt-in.
- [ ] The overlay retains memory, frame time, artwork dimensions, connection
      state, and presentation revision information.
- [ ] Its text, field, and border remain readable over both light and dark
      artwork-derived palettes and the fixed no-art fallback.
- [ ] The overlay remains unobtrusive and does not displace Gallery split,
      full-field copy, the Output/Zone row, or OLED-safe bounds.
- [ ] Automated checks cover opt-in parsing, complete diagnostic content, and
      adaptive style roles without asserting incidental GTK widget structure.
