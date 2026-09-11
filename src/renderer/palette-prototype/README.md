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
My Streamers, frozen at 1:48. The user selected D (midnight + cyan).
This supports exploring dark blue fields with cyan accents for this bright
blue cover; it does not establish that every bright cover should be dark.

The third study is at <http://localhost:8765/dont-come-to-la.html?variant=B>:

- A: Current crimson, sampled from the capture.
- B: Crimson + oxblood, the same gradient and semantic colors as A with an
  oxblood four-sided artwork frame and offset plate.
- C: Lacquer + porcelain, richer red fields, porcelain text, and a garnet frame.
- D: Parchment + burgundy, light warm fields and dark burgundy type and frame.

The user rejected the first set of third-study alternatives (charcoal +
scarlet, oxblood + chalk, and ink + ice). Those remain in the previous branch
commit. The replacement set retains A and explicitly explores the requested
current-gradient/oxblood-border combination as B. Artwork border and plate
colors are independent of status and progress accent in this study.

It shows Don't Come To LA, from YG's Still Brazy (Deluxe), paused at 0:01.
The full artist credit is retained. Paused status has its own muted accent,
shown alongside the other color values. No third-study preference is selected.

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
