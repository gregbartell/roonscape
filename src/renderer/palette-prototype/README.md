# Throwaway palette comparison

Question: how should the palette respond to different artwork while keeping
the same composition? These are manually designed visual candidates, not
outputs of a revised palette algorithm.

For Miami Ultras, the user preferred B (neutral silver). This validates a
visual direction for monochrome artwork; it does not establish universal
palette rules or authorize hardcoded album-specific colors.

Run `python src/renderer/palette-prototype/serve.py`, then open
<http://localhost:8765/?variant=A>.

- A: Current rose, sampled from the original capture.
- B: Neutral silver, with the same background as A.
- C: Warm paper, with warm neutral fields.
- D: Cool steel, with restrained blue-gray fields.

The second study is at <http://localhost:8765/that-kid.html?variant=B>:

- A: Current teal, sampled from the new capture.
- B: Ice + cobalt, light blue fields with a deep blue accent.
- C: Violet + ink, pale violet fields with an indigo accent.
- D: Midnight + cyan, dark blue fields with a cyan accent.

It shows That Kid (feat. Wisely Syndicate), by ytcracker, from Strictly for
My Streamers, frozen at 1:48. No second-study preference has been selected.

Use the floating arrows, A–D buttons, or keyboard arrows. H hides the controls;
H restores them; double-clicking the preview also restores them. R toggles the original capture. The URL records the
selected variant. Color values remain visible below the preview.

This is a static browser reconstruction, not the native Renderer. Artwork is
shown by clipping the unchanged original capture in CSS. Layout, metadata, and
playback position are fixed across variants. Native gradient dithering, font
rasterization, transitions, and palette extraction are not reproduced. The
original capture button provides the actual display for reference.

Keep this prototype out of production. Once a direction is selected, preserve
the study on a throwaway branch and record the verdict with any implementation
issue. Implement and verify any product change separately.

IBM Plex Sans is provided under the SIL Open Font License; see `OFL.txt`.
