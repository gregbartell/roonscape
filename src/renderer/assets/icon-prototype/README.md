# RoonScape icon prototype

Throwaway exploration: which visual metaphor identifies RoonScape at both
launcher and small icon sizes? No direction has been selected.

Run `npm run prototype:icons`, then open
<http://localhost:4173/icon-prototype/?variant=A>.

- `A`: Record horizon — record grooves rising above a landscape.
- `B`: Listening window — Album artwork and details inside a display.
- `C`: Groove R — a compact initial with an inset groove.
- `D`: Soundscape — a waveform that doubles as a landscape.

The page shows all four together, with a larger inspection of the selected
icon. Use the bottom switcher or left/right keys. Each direction includes
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

Keep this exploration on `t3code/prototype-svg-icon-ideas`. After selection,
record the verdict and implement the chosen icon separately. No production
icon is replaced by this prototype.
