# 10 — Tune and accept the Reference Deployment

**What to build:** Calibrate and exercise the installed Reference Deployment—an
Intel NUC RoonScape Host with about 4 GiB RAM and an attached 4K OLED—until
Gallery split is pleasant from normal viewing distance and RoonScape behaves
like an unattended personal appliance during ordinary use.

**Blocked by:** 09 — Run RoonScape unattended on a Linux host.

**Status:** ready-for-agent

- [ ] Final fonts, line bounds, minimum text sizes, and glyph fallbacks keep
  ordinary, missing, and extreme metadata legible at 3840×2160 from normal
  television-viewing distance.
- [ ] Artwork-derived palettes, neutral fallback, crossfade, inactive dimness,
  grace period, and reposition cadence are calibrated by looking at the
  physical OLED.
- [ ] Xorg uses the standard modesetting driver instead of the obsolete Intel
  driver configuration from the RoonScape Host's prior graphical setup.
- [ ] The display uses 4K60 RGB/4:4:4 when supported and otherwise favors sharp
  4K30 RGB/4:4:4 over 4K60 chroma subsampling.
- [ ] Reference Deployment smoke checks cover playing, paused, loading, stopped,
  missing artwork, pairing required, disconnected, output unavailable, long
  metadata, grouping changes, and one restart of each process.
- [ ] Turning the television off or selecting another input does not stop
  RoonScape from remaining current for the next return to the display input.
- [ ] A normal-use glance at memory, CPU, swap, and frame timing finds no
  obvious interference with co-located Roon Server; no hard resource gate, long
  soak, or exhaustive television-state matrix is added.
- [ ] The tested hardware and required deployment overrides are documented
  without embedding a hostname or making Roon Server co-location a runtime
  requirement.
- [ ] Other compatible Linux/GTK RoonScape Hosts are described as unverified
  rather than unsupported.
- [ ] The accepted setup reaches current Now Playing after boot without routine
  intervention and remains pleasant and responsive during normal personal use.
