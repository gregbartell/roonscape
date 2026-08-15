# 10 — Generate repeatable visual acceptance captures

**What to build:** Give maintainers a repeatable way to inspect and compare the
complete native restyle at the agreed development and deployment dimensions,
with a decision-focused checklist rather than brittle screenshot equality.

**Blocked by:** 08 — Preserve transitions and OLED protection; 09 — Keep
diagnostics legible on every palette.

**Status:** ready-for-agent

- [ ] One documented workflow generates native-renderer captures at
      3840×2160, 3840×2400, and 1600×900 without introducing a browser engine
      into the runtime.
- [ ] The capture matrix covers Playing, Paused, Loading with and without
      content, Idle, every unavailable state, missing metadata, missing
      artwork, long and extreme metadata, determinate and indeterminate
      progress, and non-square artwork.
- [ ] Representative captures cover the preferred and forced fallback font
      pairs plus dark, light, and fixed no-art palettes.
- [ ] The checklist compares composition, negative space, hierarchy, artwork
      fit, palette, footer position, full-field grammar, transition boundaries,
      and diagnostics with the selected prototype and specification.
- [ ] Automated coverage remains focused on fixture content, layout policy,
      palette contrast, and preserved behavior; no pixel-golden or screenshot
      difference gate is added.
- [ ] The handoff points to the existing human Reference Deployment acceptance
      work for physical 4K OLED legibility, luminance, transition, and
      burn-in-protection judgment rather than claiming those checks were
      automated.
