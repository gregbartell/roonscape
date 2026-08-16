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
metadata column ordered as playback status, Title, Artist, Album, progress,
and the Output and Zone identity row.

Title is the dominant text. Artist and Album step down in size, followed by
playback status, progress times, and identity labels. Missing metadata closes
up cleanly instead of leaving placeholders. Long values reduce within
readable limits before ellipsizing, with bounds of three Title lines, two
Artist lines, and two Album lines. Output and Zone names remain on one line
and ellipsize only as a defensive fallback.

The composition uses the complete landscape field without letterboxing.
Artwork and metadata keep their relative emphasis on ordinary, tall, wide,
and high-resolution displays rather than treating one viewport as canonical.

## Artwork and palette

Artwork is always shown completely. Square images fill the square artwork
field; non-square images are centered and contained without cropping. When
Now Playing metadata exists without usable artwork, a restrained square field
preserves the composition without inventing an icon or label.

Usable artwork supplies the color basis for the complete presentation,
including its background, artwork field, metadata field, text, accent,
progress, and diagnostics roles. Both dark and light results are valid when
the selected roles remain readable. Presentations without usable artwork use
a fixed navy, coral, and cream palette with the same role hierarchy.

## Typography

Title and Album use an editorial serif voice; Artist, playback status,
progress, identities, explanations, and diagnostics use a clean sans serif.
RoonScape selects the pair atomically so the two voices remain intentional:

- Palatino Linotype with Segoe UI when both host-provided families are
  available.
- Packaged Libre Baskerville with IBM Plex Sans otherwise.

The packaged pair provides a consistent open fallback without a network font
request or global installation. Pango may still select a readable glyph
fallback for characters that are absent from the active pair.

## Motion and inactivity

Motion is restrained to information that changes over time or protects the
display. Determinate progress advances in place while Playing and remains
frozen while Paused. A Now Playing revision crossfades artwork, metadata, and
the full palette as one coordinated layer. When availability is lost, stale
Now Playing content is removed immediately.

Paused, Idle, and unavailable presentations retain their normal appearance
during the configured inactivity grace period, then dim and move periodically
through a bounded offset sequence. The layout reserves the movement envelope,
keeping every position inside the available field. Playing and Loading remain
at full opacity and their normal position. This OLED-safe behavior is a
product capability configured through Display Configuration.

## Full-field states

States without a useful artwork-and-metadata composition use the entire field
for a concise editorial message with a vertical accent bar. This grammar
covers Idle, Loading without content, Now Playing details unavailable,
pairing required, disconnected, and an unavailable Tracked Output. Each state
keeps distinct status and explanatory copy while sharing the same hierarchy
and negative space.

Available states show the Tracked Output and current Tracked Zone at one
stable bottom-right position, under the viewer-facing labels **Output** and
**Zone**. The row stays in that position across Now Playing and available
full-field states. Pairing, disconnected, and output-unavailable states omit
the row so potentially stale identities are never shown.

Persistent product branding and unrelated controls are absent from the
viewer-facing presentation.
