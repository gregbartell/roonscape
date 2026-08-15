# Gallery Split

Variant A, **Gallery split**, was selected as RoonScape's visual direction on
2026-08-14. The complete five-variant throwaway prototype is preserved on the
`prototype/visual-variants` branch; it is prior art, not production source.

On 2026-08-15, the prototype was promoted from loose visual inspiration to the
close visual reference for the production renderer. Reproduce its composition,
relative scale, negative space, typographic character, artwork treatment, and
metadata sequence while retaining RoonScape's production behavior. Browser
chrome and the prototype control bar are not part of the reference.

Preserve its asymmetric album-sleeve composition and dedicated metadata
column, with these corrections:

- Present the Tracked Output name under the viewer-facing label **Output** and
  the Tracked Zone name under **Zone**. The Tracked Output is a Roon audio
  endpoint, not the host's video output.
- Show both identities whenever the Tracked Output is available, including
  Playing, Paused, Loading, and Idle. Do not show potentially stale
  identities while pairing, disconnected, or output-unavailable.
- Derive the entire presentation palette, including text, from the current
  album artwork. Tune the shared extraction path so the prototype artwork
  approaches the prototype's navy, coral, and cream character without adding a
  fixture-only theme. Presentations without usable artwork use a fixed fallback
  based on those prototype colors. Select readable combinations from every
  palette. Do not force artwork-derived presentations to remain dark; light
  artwork may produce a light presentation when the resulting text and accents
  remain readable.
- Wrap and reduce long metadata within firm line and minimum-size bounds; do
  not use a perpetual marquee. Preserve maximums of three Title lines, two
  Artist lines, and two Album lines. Scale preferred and minimum sizes with the
  viewport, then ellipsize only at the readable minimum.
- On the OLED television, keep the paused composition briefly, then dim and
  periodically reposition it. Calibrate the dim level on the physical screen
  rather than declaring an untested luminance in the prototype.
- Crossfade artwork, typography, and the derived palette together on track
  changes; keep all other motion limited to progress and OLED protection.
- Preserve the editorial serif Title/Album treatment paired with clean sans
  serif utility text, subject to final legibility and glyph-coverage testing.
- Do not place persistent RoonScape branding on the television.
- Apply the visual language to every Now Playing, Idle, and unavailable
  presentation rather than leaving non-Playing states in the earlier style.
- Scale the composition fluidly around the 3840x2160 Reference Deployment,
  preserve it at 16:10, and keep the 1600x900 windowed fixture representative.
  Do not letterbox either aspect ratio.
- Use the prototype's quiet, full-field treatment for trackless Idle,
  empty-Loading, and unavailable presentations. Preserve the Tracked Zone at
  the lower edge when one exists; do not render a meaningless empty artwork
  square.
- Give every full-field presentation the prototype's vertical accent bar beside
  its editorial message. Present Roon's stopped playback state as **Idle** with
  **Nothing is playing** and no explanatory sentence.
- Present empty Loading with a large **Loading** heading and no explanatory
  sentence. Present Playing without usable metadata or artwork as **Now
  Playing details unavailable** rather than retaining an empty Gallery split.
- Reserve one consistent bottom-right position for the Output and Zone identity
  row across the entire application.
- Keep Output and Zone names on one line at their intended size. Ellipsize only
  as a defensive fallback for unexpectedly long names; ordinary names should
  remain far inside the available space at every supported display size.
- When Now Playing metadata exists without artwork, retain the Gallery split
  and a restrained square artwork field so missing artwork does not reflow the
  presentation.
- Display artwork completely without cropping. If Roon supplies a non-square
  image, center it within the square artwork field and let the surrounding
  field carry the derived palette.
- Validate the restyle with repeatable fixture captures and a visual checklist
  at 3840x2160, 3840x2400, and 1600x900. Do not introduce pixel-golden tests;
  keep automated coverage focused on layout decisions, fixture content,
  palette contrast, and preserved behavior.

The shared fixture family uses the prototype's fictional release wherever a
fixture's edge case does not deliberately replace or omit a value:

- Title: **Last Light on Phobos**
- Artist: **Evelyn Lark & The Orbital Choir**
- Album: **Signals from the Quiet Sea**
- Output: **AudioDevice**
- Zone: **Living Room**
- Playing baseline: `2:51` elapsed of `4:26`, with `−1:35` remaining
- Artwork: the exact `prototype/roonscape-ui/album-art.svg` asset from the
  `prototype/visual-variants` branch

Fixture mode re-anchors the Playing baseline to its launch time so it opens at
the reference position and then advances truthfully.

Rename the state and configuration contract to `trackedOutput` and
`trackedZone` terminology as a clean break. Do not accept or migrate the legacy
`displayOutputId` field; RoonScape has no compatibility obligation yet.

Prefer Palatino Linotype for Title and Album and Segoe UI for Artist and
utility text when both host-provided faces are available. If either is
unavailable, switch the whole pair to the packaged open fallback: Libre
Baskerville for Title and Album and IBM Plex Sans for Artist and utility text.
Do not mix preferred and fallback faces, redistribute the proprietary faces,
or require network font loading. Remaining transition and OLED-safe tuning
stays an implementation detail. Do not promote the prototype directly into the
native renderer.
