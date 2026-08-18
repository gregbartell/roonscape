# Now Playing refresh direction record

Status: historical visual-direction evidence, not a production specification.
The implemented behavior is authoritative in the
[presentation design](../presentation.md), and its review workflow is in
[presentation visual acceptance](../../visual-acceptance/presentation.md).

## Selected direction

The Humanist Editorial direction combines an artwork-left Reverse Poster
hierarchy with a continuous artwork-derived Chromatic Matte and one restrained
offset print plate. All information stays on the strict rail. The responsive
artwork cap protects 4:3 breathing room, and the ultrawide measure applies only
to Title, Artist, and Album.

Title prefers host-provided Sitka Display Bold and falls back to packaged
Libre Baskerville. Packaged IBM Plex Sans supplies every Now Playing supporting
role independently of the Title choice. The selected refinement also retains
the current artwork-surface shadow, balanced Title wrapping, the restrained
light matte, glow-free Presentation Status, and the small muted identity
separator.

## Prototype evidence

The throwaway browser prototype and its controlled captures remain on branch
`prototype/visual-refresh` as non-production evidence. Its `rail` variant is
the selected information geometry. The `gutter` variant, where musical
metadata overhangs the gutter, and the `sleeve` variant, where Title crosses
the print plate and artwork, are rejected comparisons.

The prototype's floating comparison bar, scenario selector, query parameters,
palette toggle, original-artwork toggle, and plate toggle exist only to compare
directions. They are not viewer-facing controls and must not be promoted into
the native GTK 4/Pango renderer. The reference captures define intent rather
than pixel-golden output.

The broader composition and typography studies remain archived on branch
`prototype/now-playing-visual-study-archive` at commit
`aa79845cbe86c133f330576d7f93b0f420a0e1de`.
