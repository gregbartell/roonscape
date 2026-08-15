# 05 — Derive the complete presentation palette from artwork

**What to build:** Connect every visible field, text role, and accent to the
current artwork so each release owns the complete presentation palette while
remaining readable, including when the artwork produces a light composition.

**Blocked by:** 04 — Restyle complete Now Playing in Gallery split.

**Status:** ready-for-agent

- [ ] Usable artwork derives backgrounds, artwork surroundings, primary and
      secondary text, muted text, progress, and accents through the shared
      presentation path.
- [ ] Light artwork may produce a readable light presentation; palette
      selection does not force every release into a dark theme.
- [ ] The selected prototype artwork produces a result close to its navy,
      coral, cream, and muted visual direction without fixture-only colors or
      artwork-specific branching.
- [ ] Missing or unreadable artwork selects one fixed navy, coral, cream, and
      muted fallback palette derived from the prototype.
- [ ] Primary text maintains at least 7:1 contrast and secondary text and
      accents maintain at least 4.5:1 contrast against their fields.
- [ ] Palette tests cover dark artwork, light artwork, prototype artwork,
      missing artwork, unreadable artwork, and every exported presentation
      color role.
