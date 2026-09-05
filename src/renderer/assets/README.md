# RoonScape icon

[`icon.svg`](../../../assets/icon.svg) is the approved **Groove R, open groove**
design. Its larger opening preserves the returning groove at small sizes.
This file is the canonical source for application and documentation icons.

The SVG has a 64×64 viewBox, a transparent background, and no external
resources. It uses `currentColor`: black by default, or the surrounding text
color when inlined. For standalone colored or reversed exports, replace
`currentColor` with the desired ink color in a derived copy. Preserve the
source geometry and aspect ratio.

The README uses derived PNGs for light and dark themes. See
[`docs/branding`](../../../docs/branding/README.md) for reusable exports,
share graphics, and regeneration instructions.

The Renderer loads the derived desktop icon from `src/desktop/icons`, using
the name `io.roonscape.Renderer` to match its application ID and desktop entry.
This derivative places the approved mark on a pale background so it remains
visible on light and dark desktop surfaces. The packaging recipe includes
the desktop assets and per-user installer; it does not install them globally.
