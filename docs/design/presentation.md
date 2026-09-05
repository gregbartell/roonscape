# Presentation design

RoonScape uses an asymmetric, full-field composition that gives artwork and
Now Playing metadata distinct roles without making either side feel like a
separate panel. The composition scales fluidly across landscape displays at
1280x720 or larger. Portrait displays and smaller landscape viewports are not
part of the supported presentation range.

## Composition and hierarchy

The artwork field occupies the larger left side of the Now Playing
presentation. It has enough surrounding space for the artwork to read as an
album sleeve and for its shadow to remain inside the display. The narrower
right side is one information rail with three stable vertical regions: compact
Presentation Status, a vertically centered Title/Artist/Album group, and a
unified footer containing progress or indeterminate activity followed by the
Output and Zone identity row.

The imaginary square artwork field is the lesser of 84% of viewport height and
56% of viewport width, then is centered vertically. The continuous width cap
protects the information area on 4:3 displays without introducing a named
breakpoint or reducing artwork on wider displays.

Every information role begins on one strict left rail; no Title, credit,
status, progress, timing, activity, or identity content overhangs the gutter,
print plate, or artwork. On ultrawide displays only the Title, Artist, and Album
group is capped to a width of approximately 72% of viewport height. That
musical measure remains on the strict rail, while Presentation Status,
progress or activity, timing, and identities retain the complete utility
width.

Presentation Status begins at a fixed responsive inset below the imaginary
square artwork field's top edge. The footer sits low in the rail but is
optically raised from the square's bottom edge. These anchors use the reserved
square, not the viewport or the visible bounds of supplied artwork, and leave
the entire composition inside the reserved inactivity-movement field. They
remain stable for square, non-square, missing, and unusable artwork. Available
Full-field Presentations use independent status-and-copy geometry and a
bottom-right identity anchor.

Title is the dominant text. Short single-line Titles use the calm preferred
scale shared by other fitting Titles.
Longer Titles use deterministic balanced wrapping at word boundaries, without
language parsing or content-specific breaks, and avoid a very short final line
when a better-fitting arrangement exists. The complete metadata group is
vertically centered in the middle rail region with a small upward optical
correction. Artist and Album form a close credit group beneath Title, with
Artist stronger than Album and a calibrated Title-to-credit gap. Missing
metadata closes up cleanly instead of leaving placeholders.

Preferred metadata sizes derive primarily from viewport height and then fit to
both the musical metadata measure and the vertical space between status and
footer. The planner chooses the first fitting preferred, reduced, or minimum
Title tier for the complete Title/Artist/Album group. If that group still does
not fit, one deterministic compact-credit density preserves readable Artist
and Album floors. Content ellipsizes at the end when it still exceeds bounds of
five Title lines, three Artist lines, and three Album lines. Metadata uses no
scrolling, marquee motion, pagination, or content-specific exceptions.

Progress or activity uses the complete rail width at the top of the unified
footer. The identity row follows at a responsive gap and contains two compact
inline phrases separated by a small muted dot: `OUTPUT <Tracked Output>` and
`ZONE <Tracked Zone>`. Each semibold uppercase label uses slight positive
tracking and shares one baseline with its name. The two phrases receive bounded
shares of the row, so names ellipsize independently without moving the
separator or footer. Presentation Status, timing, and activity copy derive
their sizes from viewport height while preserving readable floors. Identity
size also respects a viewport-width ceiling so ordinary names remain complete
on tall displays. At
3840×2160, Artist is approximately 68 px, Album 56 px, Presentation Status
58 px, elapsed and remaining time and both activity lines 56 px, and identity
names 52 px. Identity labels remain subordinate at approximately 46 px, while
the muted separator scales to approximately 10 px. Full-field Presentation
uses its own status and identity sizes, with television-scale caps aligned to
the same utility hierarchy.

The composition uses the complete landscape field without letterboxing.
Artwork and metadata keep their relative emphasis on ordinary, tall, wide,
and high-resolution displays rather than treating one viewport as canonical.
The peer acceptance set is exactly 1280×720, 1600×900, 1600×1200, 1920×1200,
2560×1080, 3840×2160, and 3840×2400; all seven viewports carry equal design
authority.

## Synchronized lyric composition

When the Lyric Feed supplies a relevant timed cue, the Now Playing composition
temporarily gives the current lyric the central role. The artwork remains the
same persistent object but yields space: its square becomes the lesser of 68%
of viewport height and 42% of viewport width. Presentation Status and the
unified footer retain their vertical anchors and travel with the information rail. A compact Title/Artist
masthead replaces the ordinary Title/Artist/Album group; Album is omitted in
this composition.

The current cue is the room-scale focal point on the information rail. The
nearest nonblank previous cue uses muted text as memory, while the nearest
nonblank next cue uses secondary text as anticipation. A settled Intentional
Blank keeps these contextual neighbors around an empty focal position, even
after a tall cue. Leading blanks are ignored and all-blank timelines retain
ordinary Now Playing. Short blanks preserve advance promotion without a forced
empty dwell. When the current cue occupies three rendered lines, the previous cue is
omitted; at four or more lines both neighbors are omitted. The current tier is
never reduced merely to retain neighbors, and defensive overflow is limited to
four lines with an end ellipsis.

Same-identity lyric entry and exit animate persistent artwork and information
rail geometry in place. Ordinary metadata relinquishes ownership to the compact
masthead and reel without duplicating artwork, Presentation Status, or footer.
Preparation starts before the first nonblank cue's advance promotion; its
arrival may overlap the geometry movement. Internal gaps and blanks retain one
continuous composition interval, ending after the hold following the final
timeline entry, including trailing blanks.

Natural Cue Handoffs use Reel Lift: compact cues promote from Next Cue to focus
while the outgoing focal cue becomes Previous Cue. If either cue occupies three
or four Pango-rendered lines, the outgoing cue departs upward at focal size under
the reel's clip, and the incoming cue takes a shorter path. Available memory
returns as the incoming cue settles. Position, scale, semantic color, and weight
transfer together without a missing-focus interval. External seeks and timeline
revisions install destination-relative cue state directly, while boundary
crossings still animate composition geometry. Interrupted composition movement
retargets from its current geometry; interrupted handoffs prioritize the newest
cue and never queue skipped lyrics. The
platform's reduced-animation preference and deterministic Presentation
Capture behavior suppress this motion while preserving the complete lyric
hierarchy.

## Artwork and palette

Artwork composition always reserves the same imaginary square field. Supplied
artwork is shown completely within that square: square images fill it, while
non-square images are centered and contained without cropping. The unused area
around a supplied non-square image is transparent. Its visible surface,
responsive one-to-two-pixel border, and shadow follow the contained image
rectangle rather than revealing the reserved square. Square supplied artwork
keeps the same framed appearance. When Now Playing metadata exists without
usable artwork, a restrained square field preserves the composition without
inventing an icon or label.

One solid accent print plate sits behind the visible decorated artwork
rectangle at a deliberately misregistered responsive down-and-right offset. At
3840×2160 the offset is approximately 24 px right and 16 px down. Supplied
non-square artwork receives a matching non-square plate centered on the same
visible bounds before the offset is applied; the imaginary square reservation
and every information-rail anchor remain unchanged. Missing or unusable artwork
keeps a square plate behind its quiet square field. The plate stays fully
opaque, flat, crisp, square-cornered, and unblurred. The responsive keyline and
quieter artwork-surface shadow remain attached to the visible artwork
rectangle; the shadow does not move behind the combined artwork-and-plate
stack. No additional plates, registration marks, grain, rotation, or depth
effects are part of the presentation.

Usable artwork supplies the color basis for the complete presentation,
including its background, artwork field, metadata field, text, accent,
progress, and diagnostics roles. Both dark and light results are valid when
the selected roles remain readable. Presentations without usable artwork use
a fixed navy, coral, and cream palette with the same role hierarchy.

The artwork-derived gradient uses stable geometry across revisions: the
artwork field holds through approximately the first fifth, transitions near
the middle, and reaches the metadata field at the far edge on an angle near
112 degrees. A luminance ceiling compresses only light results that would
otherwise approach a room-filling near-white field. The calibrated reduction
is approximately 8–12% at the bright end and preserves the artwork's hue and
chroma; it does not neutralize dark or teal-heavy palettes.

Artwork-derived secondary and muted text target at least 7:1 contrast against
the background and metadata fields used by the information rail. Their hue
and relative emphasis remain derived from the artwork. The fixed no-art
palette retains its established role colors.

Determinate progress uses a field-relative neutral track rather than a text
role. The track remains approximately 1.5–2:1 against the local metadata field,
while the full artwork-derived accent fill differs from the track by at least
3:1. This direct relationship is guaranteed independently of either color's
contrast with the surrounding field.

Presentation Status uses the artwork-derived accent without assigning fixed
hues to playback or availability conditions. Playing and Starting use the
full accent without a glow or halo; Paused uses a muted and desaturated form
of the accent. The fixed no-art palette supplies the same roles when artwork
is unavailable. Accent emphasis is reserved for the print plate, active
Presentation Status, determinate progress fill, and indeterminate activity.

## Presentation Status

Now Playing uses a compact, circle-free Presentation Status. Every approved
symbol occupies the same fixed cell beside a bold uppercase label, so the
label begins at one stable position as playback changes. There is no border,
filled circle, glow, halo, or secondary detail. Full-field Presentation uses a
circular status treatment. Playing, Paused, Starting, and Idle use a play
triangle, pause bars, segmented ring with center point, and rounded square
silhouettes. Pairing required uses interlocking chain links, Disconnected uses
crossed Wi-Fi arcs, and Output unavailable uses a speaker followed by an `X`.

Presentation Status contains only its symbol and label. Elapsed time,
held-time copy, preparation copy, and other secondary detail do not appear in
the status row; determinate time remains in the progress area.

Determinate progress remains a minimal linear track and fill without scrub,
transport, hover, or other control affordances. It spans the footer utility
width, with elapsed and remaining timing beneath it in tabular numerals so
updates do not shift their alignment. The played segment is heavier than the
remaining track, and the remaining track is vertically centered behind it.
Both use square ends with an abrupt square transition at the current position.
At 3840×2160, the played segment is approximately 12 px high, the remaining
track is approximately 6 px high, and the gap between progress or activity and
identities is approximately 40 px; these values scale responsively across peer
viewports.

Playing without meaningful duration uses the same footer role for a compact
activity treatment instead of fabricating a timeline. Seven rounded vertical
bars use symmetrical reference heights of 30%, 70%, 100%, 48%, 100%, 70%, and
30%, followed by `Audio active` and `Timing unavailable` on separate lines.
The waveform uses the current accent and the timing explanation uses muted
text. This treatment is independent of artwork availability: supplied artwork
remains present, while missing artwork uses the quiet field.

## Typography

Now Playing typography is selected by role. Title uses bold, upright Sitka
Display with normal tracking when that family is installed on the RoonScape
Host and packaged Libre Baskerville otherwise. Artist and Album use normal
upright IBM Plex Sans, with semibold Artist and regular Album. Presentation
Status, progress and activity copy, timing, and identities also use packaged
IBM Plex Sans, independently of Title-face availability.

Now Playing Presentation Status, timing, and identities request IBM Plex's
`wdth=96` variation through Pango. When the active face does not expose that
axis, Pango ignores the request and leaves the text at normal width; RoonScape
never applies synthetic geometric compression.

Full-field Presentation atomically selects Palatino Linotype with Segoe UI
when both host-provided families are available, and packaged Libre Baskerville
with IBM Plex Sans otherwise. Diagnostics uses the utility-family selection.
The packaged faces require neither a network request nor a global
installation. Every selected family remains the first member of a Pango
family stack so ordinary glyph fallback stays available for missing
characters.

## Motion and inactivity

Motion is restrained to information that changes over time or protects the
display. Determinate progress advances in place while Playing and remains
frozen while Paused. A playback-only change that preserves the current
composition updates Presentation Status and progress in place. A composition
change crossfades artwork, metadata, identities, and the full palette as one
coordinated layer, including when Now Playing and playback change together.
Availability loss and disconnection also replace the composition through that
crossfade. Composition identity is determined from the resolved content and
artwork reference, never inferred from playback state, so a Paused update with
changed Now Playing content cannot be mistaken for a playback-only update.

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

| Condition                   | Presentation Status  | Heading                           | Explanation                                                                                             |
| --------------------------- | -------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Idle                        | `IDLE`               | `Nothing is playing`              | none                                                                                                    |
| Starting without content    | `STARTING`           | `Preparing playback`              | none                                                                                                    |
| Awaiting Roon Authorization | `PAIRING REQUIRED`   | `Enable RoonScape`                | `In a Roon client, open Settings → Extensions and enable RoonScape.`                                    |
| Disconnected                | `DISCONNECTED`       | `Waiting for Roon`                | `Check Roon Server and the network.`                                                                    |
| Output unavailable          | `OUTPUT UNAVAILABLE` | `Check the selected output`       | `Open RoonScape setup to choose another Tracked Output, or make the selected output available in Roon.` |
| Playing without content     | `PLAYING`            | `Now Playing details unavailable` | none                                                                                                    |
| Paused without content      | `PAUSED`             | `Now Playing details unavailable` | none                                                                                                    |

Available states show the Tracked Output and current Tracked Zone at one
stable bottom-right position, under the viewer-facing labels **Output** and
**Zone**. This is the Full-field anchor; Now Playing places the row in its
raised unified footer. Output unavailable uses the same Full-field anchor for
the persisted Tracked Output name alone, without a separator or Zone phrase,
because no current Tracked Zone is authoritative. A legacy Display
Configuration without a persisted name omits the row. Awaiting Roon
Authorization and disconnected omit both identities because neither recovery
action depends on the configured output.

The accent bar and copy form a centered composition that occupies 60% of the
layout viewport. The accent is its stable left edge. Presentation Status, the
heading, and any explanation are left-aligned to one shared text edge after the
responsive accent inset. The composition remains independent of the
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
