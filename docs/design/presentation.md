# Presentation design

RoonScape uses an asymmetric, full-field composition that gives artwork and
Now Playing metadata distinct roles without making either side feel like a
separate panel. The composition scales fluidly across landscape displays at
1280x720 or larger. Portrait displays and smaller landscape viewports are not
part of the supported presentation range.

## Composition and hierarchy

The artwork field occupies the larger left side of the presentation. It has
enough surrounding space for the artwork to read as an album sleeve and for
its shadow to remain inside the display. The narrower right side is a dedicated
metadata column ordered as Presentation Status, Title, Artist, Album, progress
or indeterminate activity, and the Output and Zone identity row.

Presentation Status begins a responsive inset below the imaginary square
artwork field's top edge. The complete Output and Zone identity row ends the
identical inset above its bottom edge. These symmetric anchors use the reserved
square, not the viewport or the visible bounds of supplied artwork. They remain
stable for square, non-square, missing, and unusable artwork in the Now Playing
composition. Available Full-field Presentations share only the established
bottom-right identity anchor; their Presentation Status uses the independent
Full-field geometry described below.

Title is the dominant text. Artist and Album step down in size, followed by
Presentation Status, progress or activity copy, and identity labels. Missing
metadata closes up cleanly instead of leaving placeholders. Long values use
the first fitting preferred, reduced, or minimum readable font tier, then
ellipsize at the end when content still exceeds bounds of five Title lines,
three Artist lines, and three Album lines. Output and Zone names remain on one
ellipsize only as a defensive fallback. Each identity label shares one text
baseline with its name, and content length does not move or resize the stable
row. Metadata uses no scrolling, marquee motion, or pagination and never
shrinks below the established readable font floors.

The composition uses the complete landscape field without letterboxing.
Artwork and metadata keep their relative emphasis on ordinary, tall, wide,
and high-resolution displays rather than treating one viewport as canonical.
The peer acceptance set is exactly 1280×720, 1600×900, 1600×1200, 1920×1200,
2560×1080, 3840×2160, and 3840×2400; all seven viewports carry equal design
authority.

## Artwork and palette

Artwork composition always reserves the same imaginary square field. Supplied
artwork is shown completely within that square: square images fill it, while
non-square images are centered and contained without cropping. The unused area
around a supplied non-square image is transparent. Its visible surface,
one-pixel border, and shadow follow the contained image rectangle rather than
revealing the reserved square. Square supplied artwork keeps the same framed
appearance. When Now Playing metadata exists without usable artwork, a
restrained square field preserves the composition without inventing an icon or
label.

Usable artwork supplies the color basis for the complete presentation,
including its background, artwork field, metadata field, text, accent,
progress, and diagnostics roles. Both dark and light results are valid when
the selected roles remain readable. Presentations without usable artwork use
a fixed navy, coral, and cream palette with the same role hierarchy.

Presentation Status uses the artwork-derived accent without assigning fixed
hues to playback or availability conditions. Playing uses the full accent with
a restrained glow, Paused uses a muted and desaturated form of the accent, and
Starting uses the full accent. The fixed no-art palette supplies the same
roles when artwork is unavailable.

## Presentation Status

Every Now Playing and full-field presentation uses one compact circular
symbol container beside a bold uppercase Presentation Status label. Playing,
Paused, Starting, and Idle use the approved play triangle, pause bars,
segmented ring with center point, and rounded square silhouettes. Pairing
required uses interlocking chain links, Disconnected uses crossed Wi-Fi arcs,
and Output unavailable uses a speaker followed by an `X`.

Presentation Status contains only its symbol and label. Elapsed time,
held-time copy, preparation copy, and other secondary detail do not appear in
the status row; determinate time remains in the progress area.

Playing without meaningful duration uses that progress area for a compact
activity treatment instead of fabricating a timeline. Seven rounded vertical
bars use symmetrical reference heights of 30%, 70%, 100%, 48%, 100%, 70%, and
30%, followed by `Audio active` and `Timing unavailable` on separate lines.
The waveform uses the current accent and the timing explanation uses muted
text. This treatment is independent of artwork availability: supplied artwork
remains present, while missing artwork retains the established quiet field.

## Typography

Now Playing typography is selected by role. Title uses bold, upright Sitka
Display when that family is installed on the RoonScape Host and packaged Libre
Baskerville otherwise. Artist, Album, Presentation Status, progress and
activity copy, timing, and identities always use packaged IBM Plex Sans,
independently of Title-face availability.

Full-field Presentation retains its existing atomic selection: Palatino
Linotype with Segoe UI when both host-provided families are available, and
packaged Libre Baskerville with IBM Plex Sans otherwise. Diagnostics retains
the same utility-family selection. The packaged faces require neither a
network request nor a global installation. Every selected family remains the
first member of a Pango family stack so ordinary glyph fallback stays
available for missing characters.

## Motion and inactivity

Motion is restrained to information that changes over time or protects the
display. Determinate progress advances in place while Playing and remains
frozen while Paused. Every presentation change, including availability loss
and disconnection, crossfades artwork, metadata, and the full palette as one
coordinated layer.

Paused, Idle, and unavailable presentations retain their normal appearance
during the configured inactivity grace period, then dim and move periodically
through a bounded offset sequence. The layout reserves the movement envelope,
keeping every position inside the available field. Playing and Starting remain
at full opacity and their normal position. This OLED-safe behavior is a
product capability configured through Display Configuration.

Only the complete Starting ring rotates, using a 1.8-second linear revolution;
all other Presentation Status symbols remain static. The indeterminate
activity waveform is separate from Presentation Status and scales its bars
toward 28% on an approximately 1.1-second alternating ease-in-out cycle with
staggered phases. The platform's reduced-animation preference leaves Starting
and the waveform on stable, meaningful frames without removing the activity or
timing copy.

## Full-field states

States without a useful artwork-and-metadata composition use the entire field
for a concise editorial message with a vertical accent bar. The grammar is
Presentation Status, then a heading with the viewer's takeaway or action, then
an explanation only when essential. Each heading and each present explanation
occupies one complete line across the supported landscape range.

| Condition | Presentation Status | Heading | Explanation |
| --- | --- | --- | --- |
| Idle | `IDLE` | `Nothing is playing` | none |
| Starting without content | `STARTING` | `Preparing playback` | none |
| Awaiting Roon Authorization | `PAIRING REQUIRED` | `Enable RoonScape` | `In a Roon client, open Settings → Extensions and enable RoonScape.` |
| Disconnected | `DISCONNECTED` | `Waiting for Roon` | `Check Roon Server and the network.` |
| Output unavailable | `OUTPUT UNAVAILABLE` | `Check the selected output` | `Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.` |
| Playing without content | `PLAYING` | `Now Playing details unavailable` | none |
| Paused without content | `PAUSED` | `Now Playing details unavailable` | none |

Available states show the Tracked Output and current Tracked Zone at one
stable bottom-right position, under the viewer-facing labels **Output** and
**Zone**. The row stays in that position across Now Playing and available
Full-field Presentations. Awaiting Roon Authorization, disconnected, and
output-unavailable states omit the row so potentially stale identities are
never shown.

The accent bar and copy form a centered composition that occupies 60% of the
layout viewport. The accent is its stable left edge. Presentation Status, the
heading, and any explanation are left-aligned to one shared text edge after the
existing responsive accent inset. The composition remains independent of the
bottom-right identity row.

The heading occupies a fixed-height slot whose center is the viewport's
vertical midpoint. Presentation Status occupies one stable slot above it,
using the established responsive status-to-heading spacing. An optional
explanation occupies a fixed slot below the heading using the established
responsive heading-to-explanation spacing. The accent begins at the top of the
Presentation Status slot and ends after the heading slot when no explanation
is present. An explanation extends only the accent's bottom edge through its
slot; it does not move Presentation Status, the heading, or identities.

Each heading and explanation starts at its preferred responsive size and is
fitted independently after allocation. Only a line that would otherwise
ellipsize shrinks, choosing the largest size that completes the current
approved copy on one line. The fixed slots retain their preferred-typography
geometry while fitted glyphs remain centered within them, so copy length and
font fitting do not move any anchor. End ellipsis remains a defensive widget
fallback, but no approved Full-field line is ellipsized. A change to approved
Full-field copy requires renewed layout and visual fit review; this design does
not establish a permanent minimum font size for future copy.

Fixture Mode includes both Playing without content and Paused without content
so the shared content-unavailable behavior can be inspected in each active
playback condition.

Persistent product branding and unrelated controls are absent from the
viewer-facing presentation.
