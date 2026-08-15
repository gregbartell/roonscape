# RoonScape

Status: ready-for-agent

## Problem Statement

The user runs Roon Server continuously on `roll`, an Intel NUC connected to a
4K OLED television. The television is not always on and is not always showing
`roll`'s input, but when that input is selected the user wants an attractive,
current now-playing view that requires no local interaction.

`roll` has no mouse, keyboard, touchscreen, or future need for Roon Control. On
a 4 GiB host, RoonScape should be a small personal appliance without browsing,
settings, command surfaces, or the resource cost of a permanent browser
process. It does not need to become an enterprise-grade platform.

## Solution

Build RoonScape as a greenfield, display-only application in this repository.
It observes one host-configured Display Output, follows the Display Zone that
currently contains that output, and presents Roon's current Now Playing state
on the television without exposing any way to change Roon.

Use Roon's supported JavaScript extension packages in a small TypeScript/Node.js
bridge. Publish the latest complete presentation snapshot over a private local
socket to a separate Rust/GTK 4 renderer. Pass artwork through bounded local
files rather than a network service or JSON payload. Keep the two modules in
one repository and define their boundary with a language-neutral schema and
shared fixtures.

The viewer-facing design follows the selected **Gallery split** prototype:
large artwork on the left and a dedicated metadata column on the right. The
entire palette comes from the current artwork, typography is editorial rather
than dashboard-like, motion is restrained, and inactive presentations dim and
reposition to protect the OLED.

## User Stories

1. As a listener, I want to see the current album artwork from across the room,
   so that I can recognize what is playing at a glance.
2. As a listener, I want the Title to be the most prominent text, so that I can
   identify the current selection quickly.
3. As a listener, I want to see the Artist and Album beneath the Title, so that
   I have useful context without opening another Roon client.
4. As a listener, I want the artwork and metadata to remain legible at 4K from
   normal television-viewing distance, so that the display serves the room
   rather than only someone standing beside it.
5. As a listener, I want to see the current Zone, so that I can confirm which
   Roon playback context the television is presenting.
6. As a listener, I do not want to see the internal Display Output identity, so
   that implementation details do not clutter the presentation.
7. As a listener, I want an explicit playing state, so that playback status is
   visible without relying on the meaning of a control button.
8. As a listener, I want paused playback to freeze the progress display, so
   that the screen does not imply the track is advancing.
9. As a listener, I want loading to be presented as its own state, so that a
   temporary transition is not mistaken for a stop or failure.
10. As a listener, I want stopped playback to clear track-specific content, so
    that stale artwork does not imply an old track is still current.
11. As a listener, I want pairing-required state to explain why Now Playing is
    unavailable, so that initial setup is understandable.
12. As a listener, I want a disconnected state to replace stale content, so
    that a Roon or network outage is represented truthfully.
13. As a listener, I want output-unavailable state to identify the actual
    configuration problem, so that RoonScape does not silently follow the wrong
    playback output.
14. As a listener, I want missing artwork to produce a deliberate neutral
    presentation, so that ordinary incomplete metadata does not look broken.
15. As a listener, I want missing Artist or Album lines to remain absent, so
    that RoonScape does not invent values Roon did not provide.
16. As a listener, I want elapsed and remaining progress when Roon supplies
    meaningful timing, so that I can see roughly where I am in the selection.
17. As a listener, I want progress hidden for indeterminate or live content, so
    that RoonScape does not display a fabricated `0:00` timeline.
18. As a listener, I want locally smooth progress between Roon updates, so that
    the display does not advance in distracting jumps.
19. As a listener, I want each album to recolor the entire presentation, so
    that the television feels visually connected to the current artwork.
20. As a listener, I want derived text and background colors to remain
    readable, so that expressive palettes do not make metadata unusable.
21. As a listener, I want a neutral palette when artwork is absent, so that the
    fallback remains calm and intentional.
22. As a listener, I want artwork, text, and palette to crossfade together when
    the selection changes, so that track changes feel coherent.
23. As a listener, I do not want perpetual ambient animation, so that the
    screen remains calm and the renderer does unnecessary work.
24. As a listener, I want long metadata to wrap and reduce within readable
    limits, so that unusually long names remain useful.
25. As a listener, I want extreme metadata to ellipsize rather than scroll
    forever, so that the display avoids a perpetual marquee.
26. As a listener, I want the Title and Album to use an editorial serif voice,
    so that RoonScape resembles an album sleeve or gallery caption rather than
    a software dashboard.
27. As a listener, I want Artist, Zone, state, and timing text to use a clean
    sans-serif voice, so that utility information stays clear.
28. As a listener, I do not want permanent RoonScape branding onscreen, so that
    the music remains the subject and the OLED avoids another static mark.
29. As an OLED owner, I want paused content to dim and periodically reposition
    after a grace period, so that the screen reduces burn-in risk.
30. As an OLED owner, I want idle and unavailable presentations to receive the
    same protective treatment after first explaining their state, so that
    status remains useful without leaving bright static pixels indefinitely.
31. As a listener, I want full presentation to return immediately when
    playback resumes, so that OLED protection never obscures active music.
32. As the owner, I want RoonScape to observe one configured physical Display
    Output, so that its behavior is deterministic.
33. As the owner, I want RoonScape to follow that output into and out of Roon
    groups, so that grouping does not disconnect the display from the physical
    output I chose.
34. As the owner, I do not want RoonScape to follow whichever zone becomes
    active, so that playback in another room never appears unexpectedly.
35. As the owner, I want Display Configuration to be managed once from the
    host, so that the product needs no local or network settings interface.
36. As the owner, I want first-time setup to provide some practical way to
    discover and select a Display Output, so that I do not need to guess an
    opaque identifier.
37. As the owner, I want to enable RoonScape through an official Roon client,
    so that it uses Roon's normal extension authorization flow.
38. As the owner, I want RoonScape to use its own extension identity and fresh
    authorization state, so that its authorization belongs only to RoonScape.
39. As the owner, I want the television's power and input selection to remain
    outside RoonScape, so that the hobby project does not acquire fragile CEC
    or hardware-control behavior.
40. As the owner, I want RoonScape to remain current while the television is
    off or showing another input, so that returning to `roll` is immediate.
41. As the owner, I want the bridge and renderer to restart independently, so
    that one process failure does not collapse the other process or the whole
    graphical session.
42. As the owner, I want the renderer to reconnect and receive current state,
    so that process order and ordinary restarts require no intervention.
43. As the owner, I want RoonScape to recover after Roon or the network returns,
    so that temporary availability problems do not require an SSH session.
44. As the owner, I want display failure to leave Roon Server, SSH, and
    Tailscale unaffected, so that music and remote access remain independent.
45. As the owner, I want no browser engine in the runtime, so that the display
    remains lightweight on its always-on host.
46. As the owner, I want no network listener or remote command surface, so that
    the display cannot accidentally remain a controller for other LAN devices.
47. As the owner, I want no Roon Control capability inside the bridge, so that
    read-only behavior is an architectural boundary rather than a hidden UI
    choice.
48. As the owner, I want artwork and pending state to remain bounded, so that
    changing tracks cannot accumulate an unbounded history.
49. As the owner, I want an optional host-enabled diagnostics overlay, so that
    memory, frame timing, artwork dimensions, connection state, and revision
    can be inspected when something seems wrong.
50. As the owner, I want diagnostics absent during normal use, so that they do
    not become part of the viewer-facing product.
51. As the owner, I want RoonScape to start from the existing tty1 appliance
    flow without a display manager, so that startup stays small.
52. As the owner, I want a sharp 4K signal even if that means using 30 Hz when
    full-quality 4K60 is unavailable, so that typography is not softened by
    chroma subsampling.
53. As the owner, I want resource use to be obviously reasonable beside Roon,
    so that the display does not interfere with music playback.
54. As a hobbyist maintainer, I want ordinary smoke checks rather than formal
    enterprise release gates, so that engineering effort stays proportional to
    a personal project.
55. As a hobbyist maintainer, I want one repository containing both runtime
    modules, so that releases and deployment remain coordinated.
56. As a hobbyist maintainer, I want a language-neutral state contract and
    shared fixtures, so that the bridge and renderer can be developed and
    checked independently.
57. As a hobbyist maintainer, I want the selected prototype to guide the final
    renderer without copying throwaway code into production, so that the design
    decision is preserved without inheriting prototype shortcuts.
58. As the owner, I want lyrics deferred for now, so that the initial project
    stays focused on the agreed now-playing display.

## Implementation Decisions

- Use the complete five-variant visual prototype preserved on
  `prototype/visual-variants` as reference material only. Do not promote
  prototype code directly into the native renderer.
- Keep the Node bridge and native renderer as separate modules in one
  repository and one coordinated release.
- Implement the bridge in TypeScript on Node.js using Roon's supported
  Transport, Image, and Status services.
- Do not load Roon Browse and do not implement any Roon Control command,
  command endpoint, browser UI, or network listener.
- Implement the renderer in Rust using GTK 4 and Pango. Do not embed Chromium,
  Electron, WebKit, or another browser engine.
- Define one language-neutral, versioned state schema and a shared fixture set
  as the authoritative contract between the modules.
- Keep availability separate from playback. Availability covers pairing
  required, disconnected, output unavailable, and available. Playback within
  available state covers playing, paused, loading, and stopped.
- Represent Now Playing as optional Title, Artist, and Album values mapped
  positionally from Roon's first, second, and third prepared display lines.
  These names fit the owner's dominant music use, but the API does not
  independently guarantee their semantics.
- Represent progress only when finite position and positive duration are
  available. The bridge anchors source samples; the renderer advances locally
  only while playing, freezes while paused or loading, clamps at duration, and
  re-anchors on each new sample.
- Identify artwork by a presentation revision rather than treating Roon's
  opaque image key as a stable track or media identity.
- Select one physical Display Output through host-managed Display
  Configuration and resolve its containing Display Zone on every relevant Roon
  update. Grouping, ungrouping, and renaming must not trigger an automatic
  switch to another output.
- Leave the exact one-time selection workflow and Display Configuration format
  to implementation judgment.
- Register a new RoonScape extension identity and store its authorization state
  separately from ordinary Display Configuration.
- Exchange the latest complete state snapshot over a private Unix-domain
  socket. Include a schema version and monotonic revision, bound message and
  pending-output size, and send current state immediately after connection.
- Never build an event history. A stalled renderer may be disconnected rather
  than allowing the bridge to accumulate pending snapshots.
- Pass compressed artwork through atomically replaced files in a private
  runtime directory. Keep no artwork history and retain at most current artwork
  plus one outgoing image during a short crossfade.
- Use a small artwork derivative for palette analysis rather than retaining a
  second full-resolution decoded copy.
- Adopt the Gallery split composition: dominant square artwork on the left and
  a narrower metadata column on the right.
- Keep Display Output identity offscreen. Present only the Display Zone name
  under the label **Zone**.
- Derive the full presentation palette, including text, from current artwork.
  Choose readable combinations from that artwork and use a neutral fallback
  when artwork is absent. Exact extraction and contrast algorithms remain an
  implementation detail.
- Pair an editorial serif Title and Album treatment with clean sans-serif
  Artist and utility text. Exact typefaces and glyph fallbacks remain an
  implementation detail.
- Wrap and reduce long metadata within bounded lines and a minimum readable
  size, then ellipsize extreme cases. Do not implement a perpetual marquee.
- Crossfade artwork, typography, and palette together on track changes. Keep
  other motion limited to progress and OLED-safe repositioning.
- Do not place persistent RoonScape branding onscreen.
- For paused state, freeze progress, retain the composition for a grace period,
  then dim and periodically reposition it. Apply a corresponding inactive
  treatment after stopped and unavailable states first explain themselves.
  Tune exact timing, dimness, and movement on the physical OLED.
- For stopped, disconnected, pairing-required, and output-unavailable states,
  clear stale track-specific content rather than leaving the prior selection
  visible.
- Run the bridge as an independently supervised system service and the renderer
  as an independently supervised user service tied to the graphical session.
  Each process reconnects rather than requiring the other to start first.
- Use the existing tty1 autologin with a guarded `startx` session rather than
  installing a display manager.
- Use the standard Xorg modesetting driver rather than the obsolete Intel DDX.
  Prefer 4K60 RGB/4:4:4 when available and otherwise favor sharp 4K30
  RGB/4:4:4 over 4K60 chroma subsampling.
- Provide an optional host-enabled diagnostics overlay but leave it off in
  normal operation.
- Keep SSH, Tailscale, and Roon Server operationally independent from
  RoonScape.
- Treat exact socket framing, retry intervals, configuration syntax, palette
  algorithm, font files, inactive timing, optional window manager, and final
  display mode as implementation details unless a real tradeoff emerges.
- Keep validation proportional to a hobby project. Resource observations are
  guidance, not hard gates; do not create a formal soak workload or television
  state matrix.

## Testing Decisions

- Use the versioned state schema and shared fixtures as the primary and highest
  test seam. This seam was explicitly accepted during design and should remain
  the main point at which bridge and renderer behavior meet.
- A good automated test observes external behavior at that seam: given Roon
  source events, the bridge publishes a valid complete snapshot; given a valid
  snapshot, the renderer chooses the expected presentation state. Tests should
  not assert private class structure, helper calls, or toolkit internals.
- Validate the same fixtures independently in TypeScript and Rust so neither
  module can silently drift from the shared contract.
- Include fixtures for playing, paused, loading, stopped, pairing required,
  disconnected, output unavailable, missing artwork, missing Artist/Album,
  indeterminate progress, very long metadata, and artwork revision changes.
- Exercise bridge mapping for initial full zone state, full changes,
  seek-position-only deltas, output grouping and ungrouping, output removal,
  Roon disconnect, and reconnection.
- Verify that the bridge emits only complete latest snapshots, increases the
  revision when presentation changes, immediately replays current state after
  connection, and does not retain an event or artwork history.
- Verify progress behavior through the state seam: advance only while playing,
  freeze while paused or loading, clamp to duration, and disappear when
  timing is indeterminate.
- Verify stale-content policy through presentation fixtures: stopped and
  unavailable states contain no prior Now Playing content.
- Use fixture mode for renderer development and manually inspect Gallery split
  at 4K on the actual OLED. Check ordinary metadata, very long metadata,
  missing artwork, full artwork-derived palettes, crossfade, and inactive
  repositioning.
- Treat the selected Gallery split prototype and its verdict as prior visual
  art. Reimplement the decision in GTK rather than comparing production code
  to prototype DOM or CSS details.
- Perform a small local IPC smoke check: connect, receive current state,
  restart each process once, and confirm reconnection without stale content.
- On `roll`, exercise ordinary playing, paused, loading, stopped, missing
  artwork, and unavailable states; confirm the normal boot and return-to-input
  workflow.
- Glance at process memory, CPU, and swap during normal use to catch an obvious
  regression. Do not enforce numerical budgets, a 72-hour soak, hundreds of
  synthetic track changes, or a formal television-state matrix.

## Out of Scope

- Any Roon Control capability, including play, pause, stop, previous, next,
  volume, loop, shuffle, Roon Radio, grouping, or other mutations.
- A LAN remote, network API, public HTTP service, browser UI, or browser engine.
- Library browsing, search, queue management, notifications, settings screens,
  theme selection, and user-facing diagnostics during normal operation.
- Automatic active-zone selection, fallback zones, multi-zone presentation,
  or switching away from the configured Display Output.
- Television power control, input switching, HDMI-CEC behavior, or making the
  display responsible for television availability.
- Lyrics, which are deferred for now.
- General support commitments beyond `roll` as the initial tested deployment.
- Enterprise release machinery, hard resource budgets, long formal soak
  tests, and exhaustive hardware-state matrices.

## Further Notes

- RoonScape is a personal hobby project. Prefer direct, understandable code and
  implementation judgment over speculative generality.
- The name RoonScape is intentional: it combines Roon with a visual “scape” and
  embraces the RuneScape homophone. Existing small naming collisions are
  accepted because public discoverability is not a goal.
- The television is an LG 4K OLED. Exact inactive luminance and timing should
  be calibrated by looking at that physical screen rather than specified in
  advance.
- `roll` currently has the required Node.js, Rust, GTK 4, Pango, Mesa, and Xorg
  building blocks, but no active graphical session. Its historical Xorg setup
  used an obsolete graphics driver and should not be copied unchanged.
- The user chose Variant A, Gallery split, from a five-variant throwaway visual
  prototype. The verdict includes hiding Display Output, labeling Display Zone
  as **Zone**, and deriving every presentation color from the actual artwork.
- The full visual prototype is preserved only on the
  `prototype/visual-variants` branch.
