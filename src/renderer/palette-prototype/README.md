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
- B: Crimson + parchment, parchment backing plate and progress fill, with
  a muted parchment Paused status following the existing accent-mixing rule.
- C: Espresso + copper, warm brown-black fields with ivory text and copper accents.
- D: Aubergine + lilac, deep purple fields with pale lilac text and accents.

The user rejected the first set of third-study alternatives and requested
further color options. The subsequent framed variants misunderstood their
intent: only the backing plate color was in scope, not artwork styling.
All variants now retain A's fixed geometry, 2 px keyline, shadow, and backing
plate offset of 24 px right and 16 px down. B uses parchment (#e0d2c2) for both the backing plate and progress fill.
At the user’s request, Paused now follows the existing muted-accent rule:
45% accent plus 55% muted text, producing #d3b4b0. Title, supporting text,
footer, track, and gradient retain their existing roles and colors. C and D
are new color-only alternatives. Earlier versions remain in the branch history.
Backing plate color remains independent of status and progress accent.

It shows Don't Come To LA, from YG's Still Brazy (Deluxe), paused at 0:01.
The full artist credit is retained. Paused status has its own muted accent,
shown alongside the other color values. The user selected B (crimson +
parchment) after coordinating the backing plate, progress fill, and muted
Paused status. This preserves the current red gradient and text hierarchy.

The fourth study is at <http://localhost:8765/campaign.html?variant=B>:

- A: Current copper, sampled from the capture.
- B: Navy + parchment, blue fields and warm neutral accents.
- C: Petrol + cream, teal fields and cream accents.
- D: Paper + royal blue, light warm fields and blue accents.

It shows Campaign (feat. Future), by Ty Dolla $ign / Future, frozen at 0:16
in the Synchronized Lyric Composition. The smaller artwork, compact masthead,
Lyric Reel, active cue, and footer positions are shared across all variants.
The active cue uses primary text; surrounding cues use secondary text.
Playing, progress fill, and backing plate share the accent color. Only colors
vary. The user selected D (paper + royal blue), a light palette.

The fifth study is at <http://localhost:8765/kings.html?variant=B>:

- A: Current ochre, the captured dark baseline.
- B: Aged paper + teal, a light warm field and deep teal accents.
- C: Ivory + carmine, a light ivory field and red accents.
- D: Butter + violet, a light yellow field and violet accents.

It shows Kings, credited to Steely Dan / Elliott Randall, from Can't Buy A
Thrill, frozen at 0:52. All options keep the captured layout and styling.
Backing plate, Playing, and progress share the accent. The user selected
D (butter + violet), a light palette.

The sixth study is at <http://localhost:8765/one.html?variant=B>:

- A: Current olive-gold, the captured light baseline.
- B: Limestone + forest, pale stone fields and a green accent.
- C: Marble + graphite, cool gray fields and a blue-gray accent.
- D: Sage + slate, pale green fields with dark slate text and green accents.

It shows One (Remastered), by Metallica, from ...And Justice for All
(Remastered), frozen at 0:35. All variants keep the captured layout and
styling. Backing plate, Playing, and progress share the accent. No selection yet.

## Selection balance

Count distinct artwork studies, not revisions or individual variants. Count
dark/light by the selected presentation palette, not source artwork brightness.
The user wants equal numbers of dark and light selected examples.

| Study | Selected option | Tone |
| --- | --- | --- |
| Miami Ultras / Warlord | B — Neutral silver | Dark |
| That Kid / Strictly for My Streamers | D — Midnight + cyan | Dark |
| Don't Come To LA / Still Brazy | B — Crimson + parchment | Dark |
| Campaign | D — Paper + royal blue | Light |
| Kings / Can't Buy A Thrill | D — Butter + violet | Light |
| One / ...And Justice for All | Pending | Pending |

Five selected studies: three dark, two light. Six comparisons including the
pending One study. A light selection in the sixth study would give three of
each; all four sixth-study candidates are light. The user expects this may
be the final artwork, but has not selected its palette yet.

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
