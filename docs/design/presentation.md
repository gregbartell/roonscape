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

Progress or activity uses the complete rail width within a reserved timing
slot at the top of the unified footer. The slot fits the taller timing variant
and retains its height when timing is absent. Status and timing availability
changes leave unchanged artwork and metadata bounds and wrapping stable,
including during crossfades and within the Synchronized Lyric Composition.
The identity row follows at a responsive gap and contains two compact
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

The active Cue is the room-scale focal point on the information rail. Its first
line lands at the fixed viewport-specific Primary Position, independently of
the reel's lower extent; subsequent lines extend below it. The Lyric Reel packs
earlier cues above and upcoming cues below, using the available space rather
than a fixed cue count. Normal internal line spacing and modest inter-cue gaps
keep each Cue distinct.

Each cue retains its fitted size and wrapping in earlier, active, and upcoming
roles. The preferred size is 96 px at a viewport height of 2160 px, scaled
proportionally with viewport height, rounded to whole pixels, and limited to
42–118 px. The active cue uses the palette's primary-text color; earlier and
upcoming cues share its readable secondary-text color. Activation changes no
glyph size, weight, or spacing and adds no accent rule or backdrop.

Short fades at the top and bottom allow partial cues to enter and leave
progressively; cues do not overlap or escape the lyric column into the masthead
or footer. The clearance from the lower fully transparent edge to the reserved
progress-bar top matches the clearance from the compact Title/Artist block to
the upper fully transparent edge. Timing availability does not change that
lower boundary. During composition travel, the upper edge follows the masthead
while the lower fade remains above all footer activity; parent clipping must
not truncate the fade.

A settled Intentional Blank packs available earlier and upcoming nonblank
cues around an empty Primary Position, even after a tall cue, using the same
bounds, surrounding emphasis, partial-cue clipping, and edge fades as the
nonblank Lyric Reel. Intentional Blanks retain an empty row as earlier and
upcoming context, including throughout handoffs. Consecutive blanks share one
row and retain that arrangement without
repeated lifts or additional empty rows, including on direct seeks. Leading blanks
are ignored and all-blank timelines retain ordinary Now Playing. Every Cue,
including an Intentional Blank, activates at its source timestamp. Short blanks
may be interrupted by the next cue before their departure settles; returning
lyrics continue from the displayed geometry without a forced empty dwell.

Cues retain their complete text in every role, with no excerpts, rendered-line
cap, or lyric ellipsis. When a complete cue would exceed the area below the
Primary Position and above the bottom fade, fit it using the full column width.
Establish wrapping before motion and scale the whole cue together; exceptional
fitting may reduce it below the preferred-size limits. Otherwise retain the
preferred size. Do not reduce size to retain more context or change fitted size
and wrapping between roles. Surrounding cues pack around the displayed active
bounds. All lines remain active together; fitting never moves the Primary
Position.

Keep a hyphenated word together when it fits on an otherwise empty lyric line.
If it does not fit at the end of the current line, move the whole word to the
next line, accepting unused space on the preceding line rather than reducing
the cue's font size just to avoid that break. Preserve supplied text and
explicit line breaks, the existing whole-cue fitting rules, and the shared
fitted layout used by focal and context cues.

Protect ordinary hyphens (`-`) and typographic hyphens (`‐`) joining word
segments. Spaced hyphens and en/em dashes retain their normal wrapping rules.
If a complete hyphenated word is wider than the lyric column, permit wrapping
within that word, preferring its existing hyphens as break points. If an
individual segment is still too wide, allow normal emergency wrapping within
it. Preserve the complete text and determine these breaks before animation so
they remain stable throughout composition transitions and cue handoffs.

Same-identity lyric entry and exit animate persistent artwork and information
rail geometry in place. Ordinary metadata relinquishes ownership to the compact
masthead and reel without duplicating artwork, Presentation Status, or footer.
Ordinary metadata finishes fading out before the compact masthead and lyric
reel fade in. Exit reverses this sequence, so departing cues and compact Titles
never compete with the returning large Title.
Preparation starts three seconds before the first nonblank cue's timestamp,
using only the available time when the track starts or the timeline arrives
later. A cue at zero activates immediately, and cue arrival may overlap the
geometry movement. Preparation never delays cue activation. Internal gaps and
blanks retain one continuous composition interval, ending after the hold
following the final timeline entry, including trailing blanks.

Natural Cue Handoffs use Reel Lift: the incoming Cue rises into the Primary
Position while the outgoing cue becomes quieter context above it. Every cue
retains its timed identity, including repeated identical text. Position and
semantic color transfer together with stable fitted wrapping and glyph size,
without a missing-focus interval. Outgoing cues remain visible while their
geometry intersects the lyric area, including handoffs into and out of
oversized cues. Interrupted handoffs continue from the displayed state toward
the newest cue without queueing skipped lyrics.

External seeks and timeline revisions install destination-relative cue state
directly. When Authoritative Timing relocates playback, the first frame showing
that position also shows its lyric destination: clear the entire old reel
outside the Synchronized Lyric Composition, or install the destination's active
cue, Intentional Blank, or preparation state immediately. Starting before the
timing reset does not itself clear lyrics.

Boundary crossings retain the 580 ms Lyric Composition Transition and metadata
fade sequence. After a seek or restart, the lyric area may briefly be empty
while composition geometry settles; the old passage does not linger or fade.
Natural exit after the final lyric hold retains its normal fade. Interrupted
composition movement retargets from its current geometry. The platform's
reduced-animation preference and deterministic Presentation Capture behavior
suppress motion while preserving the complete Lyric Reel.

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

During the existing five-second timing grace, show supported Provisional Timing
when it supplies determinate progress; otherwise leave the timing area quiet.
Starting and Playing share this grace, so entering Playing does not restart it.
When playback begins after Idle in the same Tracked Zone, seed Provisional
Timing at zero if position is absent; expose it only when duration is known.
Initial subscription, reconnection, and Tracked Zone changes do not seed zero.
After grace expires, Playing without determinate timing uses the same footer
role for a compact activity treatment instead of fabricating a timeline.
Seven rounded vertical bars use symmetrical reference heights of
30%, 70%, 100%, 48%, 100%, 70%, and
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
frozen while Paused. Status and timing updates retain unchanged text and its
position in the current composition. Replacement text in the same space fades
out completely before its replacement fades in; determinate timing appears
immediately when available, then numeric progress advances in place.
A transition to a Full-field Presentation retires outgoing text,
crossfades the background with text absent, then fades replacement text in.
Availability loss and disconnection retain that sequence. Now Playing
Transitions follow the coordinated reveal below; Presentation Status changes
and Lyric Composition Transitions retain their separate treatments.

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

### Now Playing Transitions and content updates

A Now Playing Transition enters Now Playing from a Full-field Presentation or
replaces a track. Fade outgoing text out over 225 ms, then reveal incoming
artwork and text together over the next 225 ms with a continuous background. Incoming
timing, Presentation Status, and lyric motion remain live throughout the reveal.
The [layered compositing decision](../adr/0004-use-layered-compositing-for-now-playing-transitions.md)
records the architectural tradeoff supporting this behavior.

The Renderer presents received content with existing fallbacks and adds no wait
for missing content. The Bridge retains its existing behavior of holding a
pending presentation during artwork retrieval and publishing it with artwork
on success or a fallback on failure.

When another destination arrives during a transition, use the latest
destination while preserving visual continuity. Replace an invisible
destination without restarting departure. Retarget an already visible
transition from its current appearance, retiring visible outgoing text before
revealing the latest destination's text, rather than completing an obsolete
presentation.

Compatible updates to the current track change only the affected content.
Crossfade artwork and its background palette together over 225 ms while text
remains visible. For metadata, fade the title/artist/album group out over
225 ms, replace and refit it while invisible, then fade it in over 225 ms;
Presentation Status, timing, and lyrics remain visible and live. Use the same
latest-destination interruption behavior for partial updates. When animations
are disabled, apply the latest destination immediately.

### Track continuity

The current Presentation Snapshot contract has no stable track identifier.
Within a continuously available Tracked Zone, use the timing continuity rule
for presentation updates too: Now Playing descriptions are compatible when no
Title, Artist, or Album value known on both sides conflicts. Missing values
are compatible; retain known values for subsequent continuity comparisons as
metadata arrives. Artwork changes alone do not establish a new track, and
playback state alone does not determine continuity.

A conflicting known metadata value is treated as a track change, even when it
may be a correction to the current track. Consecutive tracks with identical
metadata may be indistinguishable. Accept these limits rather than introduce
a second competing identity heuristic for presentation transitions; this rule
infers continuity and does not establish track identity.

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
because no current Tracked Zone is authoritative. Awaiting Roon
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
