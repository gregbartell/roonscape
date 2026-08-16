# Restyle Gallery split from the selected prototype

Status: done

## Problem Statement

RoonScape's native Gallery split renderer has the right broad structure and
preserves the required playback behavior, but it does not look sufficiently
like the selected visual-direction prototype. At 4K, its fixed pixel sizing
makes the metadata feel too small, the artwork dominates its field without the
prototype's deliberate negative space, system-dependent font fallbacks weaken
the editorial character, and the opaque artwork and metadata panels produce a
hard split rather than the prototype's cohesive gallery-like composition.

The fixture also uses different artwork, metadata, identities, and timing, so
it cannot serve as a faithful visual reference. Non-Playing and incomplete
states retain the existing behavior but need the same comprehensive visual
restyle. At the same time, the terms Display Output and Display Zone have
proved ambiguous because Display Output can be mistaken for the host's video
connection rather than the Roon audio endpoint whose playback RoonScape
presents.

## Solution

Restyle the single native renderer used by both fixture and live Roon modes so
the selected Gallery split prototype becomes its close visual reference. Match
the prototype's asymmetric composition, relative scale, generous negative
space, editorial hierarchy, artwork treatment, metadata sequence, and quiet
full-field states while retaining all existing playback, availability,
progress, transition, diagnostics, recovery, and OLED-protection behavior.

Use the prototype's exact fictional release and exact SVG artwork throughout
the coherent fixture family. Rename the stable Roon audio endpoint to Tracked
Output and its current containing Roon zone to Tracked Zone. Present their
names consistently at the bottom right under the concise viewer-facing labels
Output and Zone. Make the state/configuration contract change as a clean break;
there is no legacy compatibility obligation.

Derive the whole presentation palette from usable artwork without forcing a
dark result: light artwork may produce a light presentation when readability
is preserved. Use a fixed navy, coral, and cream palette based on the prototype
when no usable artwork exists. Scale the design fluidly around the 3840x2160
Reference Deployment while preserving the composition at 3840x2400 and in the
1600x900 windowed fixture.

Use Palatino Linotype for Title and Album and Segoe UI for Artist and utility
text when both preferred faces are installed. If either preferred face is
unavailable, switch the complete pair to Libre Baskerville and IBM Plex Sans.
Ship that open pair as the deterministic fallback without redistributing the
preferred proprietary system fonts.

## User Stories

1. As a listener, I want the native Playing presentation to look unmistakably
   like the selected Gallery split prototype, so that the implemented app
   fulfills the visual direction rather than merely sharing its broad layout.
2. As a listener, I want the prototype to be a close visual reference rather
   than a pixel-perfect web specification, so that GTK, Pango, real metadata,
   and production behavior can be accommodated without losing its character.
3. As a listener, I want the artwork to occupy the left side as a deliberate
   album-sleeve object with generous breathing room, so that it feels displayed
   rather than stretched into a panel.
4. As a listener, I want the metadata to occupy a dedicated right-hand column,
   so that artwork and information remain visually distinct and readable from
   across the room.
5. As a listener, I want the entire screen to feel like one cohesive gallery
   field rather than two opaque color panels, so that the composition is calm
   and intentional.
6. As a listener, I want Title to remain the dominant piece of text, so that I
   can identify the selection at a glance.
7. As a listener, I want Artist and Album to retain their clear hierarchy below
   Title, so that supporting identity remains legible without competing with
   it.
8. As a listener, I want playback status to remain explicit and visually
   integrated with the metadata composition, so that the screen does not rely
   on the meaning of a control icon.
9. As a listener, I want elapsed and negative remaining time to remain visible
   when Roon provides determinate progress, so that the restyle preserves the
   current truthful timeline.
10. As a listener, I want indeterminate or live content to omit the entire
    timeline, so that the display never invents timing.
11. As a listener, I want Playing progress to continue advancing smoothly from
    the latest Roon sample, so that the restyle does not introduce visible
    jumps.
12. As a listener, I want Paused and Loading progress to remain frozen, so that
    the display does not imply advancement that is not occurring.
13. As a listener, I want the exact fixture artwork for Signals from the Quiet
    Sea, so that fixture mode can be compared directly with the prototype.
14. As a listener, I want the fixture Title to be Last Light on Phobos, so that
    the reference composition has the intended line break and scale.
15. As a listener, I want the fixture Artist to be Evelyn Lark & The Orbital
    Choir, so that the reference byline matches the prototype.
16. As a listener, I want the fixture Album to be Signals from the Quiet Sea,
    so that the reference album treatment matches the prototype.
17. As a listener, I want the fixture Output to be AudioDevice and its Zone to
    be Living Room, so that every fixture uses one coherent identity set.
18. As a listener, I want the Playing fixture to open at 2:51 elapsed of 4:26
    with −1:35 remaining, so that its initial presentation matches the
    prototype.
19. As a listener, I want fixture progress to advance truthfully after opening
    at the reference position, so that visual fidelity does not turn Playing
    into a frozen mock.
20. As a maintainer, I want related Paused, Loading, missing-field,
    missing-artwork, and transition fixtures to inherit the fictional release
    wherever their edge case does not replace or omit a value, so that fixture
    inspection remains coherent.
21. As a listener, I want the configured Roon audio endpoint described as the
    Tracked Output, so that it is not confused with the host's video output.
22. As a listener, I want the Roon zone currently containing that endpoint
    described as the Tracked Zone, so that its relationship to grouping and
    ungrouping is clear.
23. As a listener, I want the screen to label those identities simply Output
    and Zone, so that internal domain precision does not make the footer
    verbose.
24. As a listener, I want Output and Zone anchored to the same bottom-right
    position in every presentation where authoritative values exist, so that
    the information does not move as playback changes.
25. As a listener, I do not want stale Output or Zone identities during
    pairing, disconnection, or output-unavailable states, so that an outage is
    represented truthfully.
26. As a listener, I want ordinary Output and Zone names to remain comfortably
    inside their footer, so that defensive overflow behavior is effectively
    invisible during normal use.
27. As a listener, I want unexpectedly long Output or Zone names to ellipsize
    on one line rather than wrap or disrupt the layout, so that the footer
    remains stable.
28. As a listener, I want Roon's stopped playback state presented as Idle, so
    that viewer-facing language describes the quiet screen naturally.
29. As a listener, I want Idle to show a status dot, the label Idle, and the
    heading Nothing is playing, so that the state is immediately understandable.
30. As a listener, I want the Idle message to use the prototype's vertical
    accent bar, so that the empty presentation belongs to the same visual
    system as Playing.
31. As a listener, I want Idle to avoid redundant explanatory prose, so that
    the quiet state remains restrained.
32. As a listener, I want empty Loading to use a full-field composition with a
    large Loading heading and no invented explanation, so that a brief state
    stays clear without becoming noisy.
33. As a listener, I want Loading with current metadata and artwork to retain
    the Gallery split composition, so that available information does not
    disappear during a temporary transport state.
34. As a listener, I want Playing without usable metadata or artwork to show
    Now Playing details unavailable, so that active playback remains truthful
    without displaying an empty album square.
35. As a listener, I want pairing-required, disconnected, and
    output-unavailable presentations to retain their distinct corrective
    meanings, so that the restyle does not collapse operationally different
    conditions into one generic failure.
36. As a listener, I want every full-field Idle, empty-Loading, incomplete, and
    unavailable presentation to use the same accent-bar grammar, so that the
    entire app feels deliberately restyled.
37. As a listener, I want a Playing presentation with metadata but no artwork
    to retain the Gallery split and a restrained square artwork field, so that
    missing art does not cause the layout to jump.
38. As a listener, I want missing Artist or Album values to remain absent
    without placeholders or broken spacing, so that RoonScape does not invent
    metadata.
39. As a listener, I want unusually long Title, Artist, and Album values to
    wrap and reduce within readable bounds, so that real-world metadata remains
    useful.
40. As a listener, I want extreme metadata to ellipsize rather than scroll or
    shrink indefinitely, so that the display remains calm and readable.
41. As a listener, I want Title limited to three lines and Artist and Album to
    two lines each, so that the metadata column retains its intended hierarchy.
42. As a listener, I want artwork shown completely without cropping, so that
    RoonScape presents the supplied image accurately.
43. As a listener, I want uncommon non-square artwork centered inside the
    square artwork field, so that its content remains intact without changing
    the Gallery split geometry.
44. As a listener, I want every usable artwork image to recolor the entire
    presentation, including fields, text, and accents, so that the display
    feels connected to the current release.
45. As a listener, I want light artwork to be allowed to create a readable
    light presentation, so that palette extraction responds to the art rather
    than forcing every album into a dark template.
46. As a listener, I want presentations without usable artwork to use the
    prototype's fixed navy, coral, and cream fallback, so that empty and error
    states remain deliberate and visually related to the reference design.
47. As a listener, I want primary, secondary, and accent colors to retain their
    existing contrast guarantees, so that expressive palettes do not sacrifice
    legibility.
48. As a listener, I want artwork, metadata, and the full derived palette to
    continue crossfading as one presentation, so that track changes remain
    coherent.
49. As a listener, I do not want new perpetual animation, so that the
    presentation remains calm and appropriate for an unattended display.
50. As an OLED owner, I want Paused, Idle, and unavailable presentations to
    retain the existing grace, dimming, and repositioning behavior, so that the
    visual restyle does not weaken burn-in protection.
51. As an OLED owner, I want Playing to restore full luminance and the normal
    composition immediately, so that protective behavior never obscures active
    music.
52. As a listener, I want the composition calibrated for 3840x2160, so that it
    serves the Reference Deployment from normal television-viewing distance.
53. As a developer, I want the same visual proportions to remain credible at
    3840x2400, so that fixture inspection on the development display is useful.
54. As a developer, I want the 1600x900 windowed fixture to remain a faithful
    scaled preview, so that ordinary development does not require fullscreen
    4K output.
55. As a listener, I do not want letterboxing at supported 16:9 or 16:10 aspect
    ratios, so that the presentation uses the available display naturally.
56. As an owner, I want fixture and live Roon modes to use the same renderer
    and visual path, so that the fixture cannot conceal live presentation
    behavior.
57. As an owner, I want live Roon mode to continue using real artwork,
    metadata, progress, Tracked Output, and Tracked Zone data, so that only the
    fixture is fictional.
58. As an owner, I want grouping and ungrouping to keep the Tracked Output
    stable while updating the Tracked Zone, so that the restyle preserves
    deterministic tracking behavior.
59. As an owner, I want disconnection and unavailable states to clear stale
    Now Playing content immediately, so that visual continuity never overrides
    truthfulness.
60. As an owner, I want the optional diagnostics overlay to remain available
    and visually compatible with the restyled app, so that troubleshooting
    functionality is preserved without appearing during normal use.
61. As an owner, I want no Roon controls, settings screen, browser interface,
    persistent branding, or new network surface, so that this remains an
    unattended read-only presentation.
62. As an owner, I want the native GTK/Pango renderer retained without an
    embedded browser engine, so that the restyle respects the lightweight
    runtime architecture.
63. As a listener, I want Palatino Linotype and Segoe UI used when both are
    available, so that the production presentation retains the selected
    prototype's editorial character.
64. As a listener, I want Libre Baskerville and IBM Plex Sans used together
    when either preferred face is unavailable, so that fallback typography is
    deliberate and never an accidental mixed pair.
65. As a maintainer, I want repeatable captures and a documented visual
    checklist instead of brittle pixel-golden assertions, so that renderer and
    font differences do not create meaningless failures.

## Implementation Decisions

- Keep the existing Node.js bridge and Rust/GTK 4 renderer boundary. Fixture
  and live Roon paths continue publishing the same complete snapshots to one
  native renderer; do not create a fixture-specific UI.
- Use the selected Gallery split prototype as the close visual reference for
  composition, relative scale, negative space, typographic character, artwork
  treatment, and metadata order. Do not reproduce browser chrome or the
  prototype control bar, and do not promote the throwaway DOM or CSS into the
  native runtime.
- Copy the prototype's album-art SVG byte-for-byte into the production fixture
  assets rather than reconstructing it from a screenshot.
- Give the artwork a square field on the left with prototype-like breathing
  room, restrained depth, and a dedicated metadata column on the right. Replace
  the current conspicuous opaque panel split with a cohesive full-screen field
  driven by the presentation palette.
- Scale layout dimensions and typography from the viewport around the
  3840x2160 reference. Preserve the composition without letterboxing at
  3840x2400 and 1600x900.
- Rename the stable configured Roon audio endpoint to Tracked Output and the
  current Roon zone containing it to Tracked Zone throughout domain language,
  state, configuration, implementation names, tests, and documentation.
- Treat the state/configuration rename as a breaking contract revision and
  increment the snapshot schema version. Use `trackedOutputId` in Display
  Configuration and `trackedOutput` and `trackedZone` in presentation
  snapshots. Reject `displayOutputId` and `displayZone` rather than reading,
  migrating, or serializing them.
- Complete available snapshots carry the current Tracked Output name and
  Tracked Zone name needed by the renderer. Unavailable snapshots do not carry
  identities that cannot be asserted as current.
- Present the identities under the viewer labels Output and Zone in one stable
  bottom-right row. Show the row in Playing, Paused, Loading, and Idle whenever
  the values are authoritative. Omit it during pairing, disconnection, and
  output-unavailable instead of showing stale values or placeholders.
- Keep ordinary Output and Zone names on one line at their intended size.
  Ellipsize only as a defensive fallback for unexpectedly long names; do not
  design normal spacing around expected truncation.
- Map Roon's stopped playback state to the viewer-facing state Idle. Idle uses
  a status dot, the label Idle, the heading Nothing is playing, no explanatory
  sentence, the full-field prototype composition, and a vertical accent bar.
- Loading with usable Now Playing content retains Gallery split and the
  Loading status. Loading without content uses a full-field presentation with
  the accent bar, a large Loading heading, no invented explanation, and the
  identity row when authoritative.
- Playing without usable metadata or artwork uses a full-field presentation
  with Playing status and the heading Now Playing details unavailable.
- Pairing-required, disconnected, and output-unavailable retain distinct
  corrective headings and explanations, restyled into the common full-field
  accent-bar composition. Output-unavailable copy refers clearly to the
  selected Roon output without presenting its stale identity.
- Missing Artist or Album remains absent. Missing artwork with otherwise usable
  metadata retains Gallery split and a quiet square artwork field; do not add a
  placeholder label or icon.
- Display the supplied artwork completely. Center non-square artwork inside
  the square field with palette-derived surrounding color rather than cropping
  it.
- Preserve maximums of three Title lines, two Artist lines, and two Album
  lines. Use viewport-scaled preferred, reduced, and minimum sizes; ellipsize
  only after reaching the readable minimum. Never introduce a marquee.
- Derive the entire palette, including backgrounds, text, secondary text, and
  accents, from usable artwork. Permit either light or dark results according
  to the artwork; do not impose a universal dark theme.
- Preserve the current readability guarantees: primary text has at least a 7:1
  contrast ratio and secondary text and accents at least 4.5:1 against their
  field.
- When artwork is absent or unusable, use a fixed fallback based on the
  prototype's navy, coral, cream, and muted supporting tones.
- Tune the shared extraction path so the prototype artwork yields a result
  close to its original navy, coral, and cream presentation. Do not special-case
  fixture mode or add a fixture-only theme.
- Use the exact fixture values Title Last Light on Phobos, Artist Evelyn Lark &
  The Orbital Choir, Album Signals from the Quiet Sea, Output AudioDevice, Zone
  Living Room, position 171 seconds, duration 266 seconds, and initial labels
  2:51 and −1:35. Related fixtures inherit these values unless their named edge
  case deliberately replaces or omits one.
- Re-anchor the Playing fixture's sample time when fixture mode launches. It
  opens at 171 seconds and then advances through the same progress behavior as
  live Playing rather than freezing or immediately clamping to duration.
- Preserve coordinated crossfades of artwork, metadata, and the complete
  palette on visual revision changes. Preserve in-place progress-only updates,
  immediate stale-content clearing on disconnection, and bounded transition
  resources.
- Preserve the existing OLED inactivity policy, host configuration, and safe
  repositioning behavior. Ensure the larger composition, shadows, and fields
  remain within safe bounds while repositioning.
- Preserve the optional diagnostics data and opt-in behavior. Restyle the
  overlay enough to remain legible and compatible with both light and dark
  palettes without making it part of the normal presentation.
- Do not add controls, settings, branding, ambient animation, a browser engine,
  a browser UI, a network command surface, or any Roon Control capability.
- Prefer Palatino Linotype for Title and Album and Segoe UI for Artist and
  utility text. Treat them as one typography pair: use them only when both
  faces are installed and resolvable.
- When either preferred face is unavailable, use Libre Baskerville for Title
  and Album and IBM Plex Sans for Artist and utility text. Do not mix one
  preferred face with one fallback face.
- Package the open fallback faces and their license notices with the supported
  deployment so fallback does not depend on network access or arbitrary host
  fonts. Do not redistribute Palatino Linotype or Segoe UI; use them only when
  the host already provides them.
- Continue allowing Pango to substitute suitable glyph fonts for characters
  outside the selected faces' coverage rather than rendering missing-glyph
  boxes.

## Testing Decisions

- Use the versioned shared snapshot and fixture family as the primary test seam
  for the feature. It is the highest existing seam shared by the bridge and
  renderer and already has independent TypeScript and Rust coverage.
- Good automated tests assert externally visible state and policy at that seam:
  accepted contract vocabulary, published identities, presentation selection,
  text values, progress behavior, palette guarantees, and bounded layout
  decisions. They should not assert private helper calls, GTK widget nesting,
  or incidental CSS/provider structure.
- Contract tests accept the new Tracked Output and Tracked Zone fields in
  complete available snapshots and reject the removed display-output and
  display-zone vocabulary. Display Configuration tests likewise reject the
  removed legacy identifier rather than testing migration behavior.
- Bridge tests verify that live events publish the current Tracked Output and
  Tracked Zone names, retain the Tracked Output across grouping and ungrouping,
  update the Tracked Zone, and omit identities when they are not authoritative.
- Fixture-publisher tests verify that the default fixture family contains the
  agreed fictional release and identities and that Playing is re-anchored to
  launch at 171 seconds before advancing normally.
- Renderer presentation tests cover Playing, Paused, Loading with and without
  content, Idle, pairing required, disconnected, output unavailable, missing
  metadata, missing artwork, and indeterminate progress without changing their
  established playback semantics.
- Renderer layout-policy tests cover the three/two/two metadata line limits,
  viewport-scaled preferred and minimum sizes, final ellipsis, defensive
  single-line Output/Zone truncation, full-field versus Gallery split
  selection, and non-cropping artwork fit.
- Palette tests cover dark artwork, light artwork, the fixed no-art fallback,
  unusable artwork fallback, full-palette derivation, and the existing 7:1 and
  4.5:1 contrast thresholds.
- Existing transition tests remain the prior art for coordinated replacement,
  progress-only updates, rapid revisions, and bounded outgoing resources.
  Existing presentation tests remain the prior art for inactivity timing,
  immediate Playing restoration, and stale-content clearing.
- Existing socket and integration tests continue proving that fixture and live
  data use the same renderer path, reconnect independently, and replay the
  current complete snapshot.
- Produce repeatable manual captures at 3840x2160, 3840x2400, and 1600x900. The
  checklist covers ordinary Playing, Paused, Loading with and without content,
  Idle, all unavailable states, missing artwork, missing metadata, long and
  extreme metadata, determinate and indeterminate progress, light and dark
  artwork, and non-square artwork.
- Compare captures against the prototype for composition, proportions,
  negative space, hierarchy, artwork treatment, footer position, and
  full-field state grammar. Do not require pixel identity across browser and
  GTK renderers.
- Do not add pixel-golden or screenshot-diff tests. Final legibility, palette,
  transition, and OLED judgments remain a manual check on the physical
  3840x2160 Reference Deployment.
- Produce representative captures with both the preferred pair and a forced
  fallback configuration. Verify that pair selection is atomic and that
  unsupported characters receive readable glyph fallback.

## Out of Scope

- Preserving or migrating legacy display-output or display-zone configuration
  and snapshot field names.
- A fixture-only palette, hard-coded palette for presentations with usable
  artwork, user-selectable themes, or a forced dark mode.
- Cropping supplied artwork to fill the square field.
- Pixel-perfect reproduction of browser rendering or reuse of the prototype's
  throwaway HTML, CSS, or JavaScript in production.
- Separate fixture and live-renderer implementations.
- New playback controls, volume controls, browsing, queue management, lyrics,
  settings screens, remote control, networking, or other Roon Control behavior.
- New ambient animation beyond truthful progress, coordinated crossfades, and
  existing OLED protection.
- Changes to television power, input selection, HDMI-CEC, the graphical-session
  architecture, or the native renderer/browser-engine decision.
- Broad display compatibility claims beyond the Reference Deployment and the
  agreed development preview sizes.
- Pixel-golden testing, a formal exhaustive display matrix, or unrelated
  pending behavior fixes such as same-track artwork flicker.

## Further Notes

- The first supplied screenshot is the selected Variant A Gallery split browser
  prototype; the second is the current native GTK fixture.
- The exact prototype SVG is the `album-art.svg` asset preserved on the
  `prototype/visual-variants` branch at commit
  `7f4dc62521a59f0799aeabe4f2cf0e580dc1b16d`.
- The prototype's browser and control chrome are explicitly excluded from the
  visual reference.
- The source prototype uses fixed colors, but those colors are a target for its
  artwork and the no-art fallback rather than a universal theme.
- On the development host the prototype renders through Palatino Linotype and
  Segoe UI. They are the selected preferred pair but remain host-provided
  proprietary fonts and must not be redistributed with RoonScape.
- The switchable typography study is preserved under
  `prototype/gallery-split-font-study/`. It compares the original system-font
  rendering with four open-font pairs across the agreed Gallery split and
  full-field states. Variant A is selected as the preferred pair, with Variant
  D selected as the complete open fallback.
- The domain glossary and Gallery split design notes record the Tracked Output,
  Tracked Zone, Idle, fixture, palette, layout, and state decisions summarized
  here.
- No new ADR is required: the native architecture and selected visual direction
  are already recorded, while these refinements are reversible presentation and
  pre-release contract decisions without a compatibility tradeoff.
