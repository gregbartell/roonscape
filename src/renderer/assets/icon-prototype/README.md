# RoonScape icon prototype

The user selected **Groove R** from the first round. This throwaway refinement
asks which inset groove best preserves its character at small sizes. The outer
R silhouette is identical in all three variations. The exact refinement is
still undecided; this round does not install a production icon.

Run `npm run prototype:icons`, then open
<http://localhost:4173/icon-prototype/?variant=original>.

- `original`: the unchanged `groove.svg`, with a 3-unit returning stroke.
- `open`: `groove-open.svg`, with a larger opening and a 4-unit returning
  stroke. At 16 CSS px, the inner gap grows from 0.75 to 1.5 CSS px.
- `simple`: `groove-simple.svg`, with the original opening and one 4-unit
  rounded stroke, removing the small inner loop.

The page shows all three together with 16, 24, 32 and 48 CSS px samples on
light and dark backgrounds, plus a larger inspection of the selected icon.
Use the bottom switcher or left/right keys. Each variation also includes
16–64 px samples, light/dark surfaces, a launcher tile, and a wordmark.
The color toggle and selected variant are reflected in the URL. Download
exports the selected SVG using the displayed ink color.

These SVGs use a 64×64 viewBox, transparent backgrounds, and `currentColor`.
They contain no fonts, raster images, filters, or external resources.
The standalone source SVGs default to black and can inherit color when inlined.

This native GTK project has no browser route to host the comparison, so the
page lives beside Renderer assets and uses a dedicated local static server.
Release packaging only copies the fonts from this assets directory; the
prototype and its switcher are not part of the production Renderer.

Keep this exploration on `t3code/prototype-svg-icon-ideas`. The first round is
captured in commit `fe1a92a`; its SVG sources remain available here. After
refinement selection, record the verdict and implement the chosen icon
separately. No production icon is replaced by this prototype.
