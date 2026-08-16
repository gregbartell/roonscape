# RoonScape

Status: ready-for-agent

## Problem Statement

The initial Reference Deployment runs RoonScape and Roon Server on an Intel NUC
connected to a 4K OLED television. The television is not always on and is not
always showing the RoonScape Host's input, but when that input is selected the
user wants an attractive, current Now Playing view that requires no local
interaction.

The Reference Deployment has no mouse, keyboard, touchscreen, or future need
for Roon Control. On a 4 GiB RoonScape Host, RoonScape should be a small personal
appliance without browsing, settings, command surfaces, or the resource cost of
a permanent browser process. It does not need to become an enterprise-grade
platform.

During renderer development, Fixture Mode currently opens only one predefined
presentation at a time. Manually inspecting another viewer-facing state
requires restarting the session with a different fixture selection, which
makes it slow and awkward to compare the complete visual matrix or watch the
real transitions between its states.

## Solution

Build RoonScape as a greenfield, display-only application in this repository.
It observes one Tracked Output configured on the RoonScape Host, follows the
Tracked Zone that currently contains that output, and presents Roon's current
Now Playing state on the television without exposing any way to change Roon.

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

In an ordinary Fixture Mode session, let a focused renderer use the Left and
Right arrow keys to move backward and forward through the maintained 18-scenario
visual-acceptance catalog. Publish every selection through the normal complete
snapshot path so the native renderer exercises the same presentations and
transitions as Live Mode. Keep navigation unavailable in Live Mode and preserve
the existing explicit single-fixture override.

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
6. As a listener, I want to see the Tracked Output name under **Output**, so
   that I can confirm which physical Roon audio endpoint is being presented.
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
    screen remains calm and the renderer does not do unnecessary work.
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
32. As the owner, I want RoonScape to observe one configured physical Tracked
    Output, so that its behavior is deterministic.
33. As the owner, I want RoonScape to follow that output into and out of Roon
    groups, so that grouping does not disconnect the display from the physical
    output I chose.
34. As the owner, I do not want RoonScape to follow whichever zone becomes
    active, so that playback in another room never appears unexpectedly.
35. As the owner, I want Display Configuration to be managed once from the
    RoonScape Host, so that the product needs no local or network settings
    interface.
36. As the owner, I want first-time setup to provide some practical way to
    discover and select a Tracked Output, so that I do not need to guess an
    opaque identifier.
37. As the owner, I want to enable RoonScape through an official Roon client,
    so that it uses Roon's normal extension authorization flow.
38. As the owner, I want RoonScape to use its own extension identity and fresh
    authorization state, so that its authorization belongs only to RoonScape.
39. As the owner, I want the television's power and input selection to remain
    outside RoonScape, so that the hobby project does not acquire fragile CEC
    or hardware-control behavior.
40. As the owner, I want RoonScape to remain current while the television is
    off or showing another input, so that returning to the display input is
    immediate.
41. As the owner, I want the bridge and renderer managed as one foreground
    RoonScape session, so that a child failure is visible without leaving half
    of the application running.
42. As the owner, I want unattended recovery to restart the complete RoonScape
    session, so that the deployment does not supervise private runtime
    processes independently.
43. As the owner, I want RoonScape to recover after Roon or the network returns,
    so that temporary availability problems do not require an SSH session.
44. As the owner, I want display failure to leave Roon Server, other RoonScape
    Host workloads, and remote administration unaffected, so that they remain
    operationally independent.
45. As the owner, I want no browser engine in the runtime, so that the display
    remains lightweight on its always-on RoonScape Host.
46. As the owner, I want no network listener or remote command surface, so that
    the display cannot accidentally remain a controller for other LAN devices.
47. As the owner, I want no Roon Control capability inside the bridge, so that
    read-only behavior is an architectural boundary rather than a hidden UI
    choice.
48. As the owner, I want artwork and pending state to remain bounded, so that
    changing tracks cannot accumulate an unbounded history.
49. As the owner, I want an optional diagnostics overlay enabled through
    RoonScape Host configuration, so that memory, frame timing, artwork
    dimensions, connection state, and revision can be inspected when something
    seems wrong.
50. As the owner, I want diagnostics absent during normal use, so that they do
    not become part of the viewer-facing product.
51. As the owner, I want the included Linux deployment to support unattended
    graphical startup without requiring a display manager, so that startup
    stays small.
52. As the owner, I want a sharp 4K signal even if that means using 30 Hz when
    full-quality 4K60 is unavailable, so that typography is not softened by
    chroma subsampling.
53. As the owner, I want resource use to be obviously reasonable on a
    constrained RoonScape Host, so that the display does not interfere with
    Roon Server when they share a machine.
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
59. As a hobbyist maintainer, I want Fixture Mode to expose the complete
    viewer-facing visual matrix in one session, so that I can inspect it without
    repeatedly restarting RoonScape.
60. As a hobbyist maintainer, I want Right to select the next Fixture Scenario,
    so that forward inspection follows a familiar keyboard convention.
61. As a hobbyist maintainer, I want Left to select the previous Fixture
    Scenario, so that I can easily return to a presentation I just inspected.
62. As a hobbyist maintainer, I want navigation to wrap at both ends of the
    catalog, so that repeated inspection remains a continuous cycle.
63. As a hobbyist maintainer, I want an ordinary Fixture Mode session to start
    at Playing, so that the primary presentation remains the predictable entry
    point.
64. As a hobbyist maintainer, I do not want Fixture Mode to persist my last
    selection, so that every new session starts deterministically.
65. As a hobbyist maintainer, I want the 18 Fixture Scenarios to follow the
    visual-acceptance order, so that common playback and availability states
    precede progressively rarer content and artwork cases.
66. As a hobbyist maintainer, I want the interactive and capture workflows to
    share one ordered scenario catalog, so that two definitions of the complete
    matrix cannot drift apart.
67. As a hobbyist maintainer, I want all catalog scenarios loaded and validated
    before inspection begins, so that a broken session fails clearly instead of
    silently omitting coverage.
68. As a hobbyist maintainer, I want scenario changes to exercise the normal
    presentation transitions, so that Fixture Mode reveals transition defects
    as well as settled endpoints.
69. As a hobbyist maintainer, I want every selected Playing scenario to restart
    at its reference progress position, so that repeated inspection remains
    deterministic.
70. As a hobbyist maintainer, I want every selected inactive scenario to begin
    a fresh inactivity grace period, so that I can inspect its undimmed state
    before OLED protection takes effect.
71. As a hobbyist maintainer, I want each selected Fixture Scenario named in the
    terminal, so that I can identify it without altering the presentation.
72. As a hobbyist maintainer, I do not want an onscreen navigation label or
    control, so that Fixture Mode preserves the pixels I am judging.
73. As a hobbyist maintainer, I want arrow navigation only while the renderer
    window has focus, so that RoonScape does not install global shortcuts.
74. As a hobbyist maintainer, I want a held arrow key to advance only once, so
    that keyboard repeat does not accidentally skip several scenarios.
75. As a hobbyist maintainer, I want distinct rapid arrow presses to remain
    responsive, so that I can move quickly through the catalog and the latest
    deliberate selection wins.
76. As an owner using Live Mode, I want Left and Right to remain inert, so that
    development navigation never becomes a live display interaction.
77. As a hobbyist maintainer, I want the existing explicit fixture override to
    retain its single-fixture behavior, so that focused engineering workflows
    do not change as a side effect.
78. As a hobbyist maintainer, I want Fixture Mode navigation to use the normal
    complete-snapshot presentation path, so that it does not create a
    fixture-specific renderer implementation.
79. As a hobbyist maintainer, I want selection updates to retain monotonic
    session revisions, so that manual navigation continues to honor the shared
    state contract.
80. As a hobbyist maintainer, I want Escape and existing window-close behavior
    to remain unchanged, so that Fixture Mode remains easy to leave in both
    windowed and fullscreen presentations.

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
- Select one physical Tracked Output through Display Configuration on the
  RoonScape Host and resolve its containing Tracked Zone on every relevant Roon
  update. Grouping, ungrouping, and renaming must not trigger an automatic
  switch to another output.
- Leave the exact one-time selection workflow and Display Configuration format
  to implementation judgment.
- Treat the service account, release location, persistent XDG locations,
  graphical session command, recovery policy, and display mode as deployment
  configuration. Keep host variation in configuration and deployment templates
  rather than product identifiers, host-identity branches, or a speculative
  host-adapter framework.
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
- Present the Tracked Output name under **Output** and the Tracked Zone name
  under **Zone** whenever those identities are authoritative.
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
- Ship an initial Linux deployment profile that launches one foreground
  `roonscape` command in the graphical session. The launcher manages the bridge
  and renderer as one runtime session; the deployment profile may restart the
  complete command after failure but must respect a successful intentional
  exit. RoonScape does not daemonize or restart itself.
- On the Reference Deployment, use tty1 autologin with a guarded `startx`
  session instead of a display manager and use the standard Xorg modesetting
  driver instead of the obsolete Intel DDX. Prefer 4K60 RGB/4:4:4 when
  available and otherwise favor sharp 4K30 RGB/4:4:4 over 4K60 chroma
  subsampling.
- Provide an optional diagnostics overlay enabled through RoonScape Host
  configuration but leave it off in normal operation.
- Keep RoonScape operationally independent from Roon Server, other RoonScape
  Host workloads, and remote administration. The bridge must not require Roon
  Server to run on the RoonScape Host.
- Treat exact socket framing, retry intervals, configuration syntax, palette
  algorithm, font files, inactive timing, optional window manager, and final
  display mode as implementation details unless a real tradeoff emerges.
- Keep validation proportional to a hobby project. Resource observations are
  guidance, not hard gates; do not create a formal soak workload or television
  state matrix.
- Define one ordered Fixture Scenario catalog shared by interactive Fixture
  Mode and repeatable visual-acceptance captures. The catalog contains Playing,
  Paused, Loading with content, Loading without content, Idle, pairing required,
  disconnected, output unavailable, Playing without content, missing metadata,
  missing Artist, missing Album, missing artwork, long metadata, extreme
  metadata, indeterminate progress, non-square artwork, and light artwork.
- Start an ordinary Fixture Mode session at Playing and keep scenario selection
  in memory only. Right selects the next catalog entry, Left selects the
  previous entry, and both ends wrap.
- Load and validate the complete Fixture Scenario catalog before an ordinary
  Fixture Mode session becomes available. Fail the session clearly if any
  catalog entry is absent or invalid rather than publishing a partial catalog.
- Keep the catalog and fixture-file ownership outside the native renderer. The
  renderer emits semantic Previous and Next navigation intents and continues to
  consume only complete presentation snapshots.
- Activate navigation through an explicit, private Fixture Mode control
  channel. Ordinary Fixture Mode supplies that capability; Live Mode does not,
  so Left and Right remain inert there without introducing Roon Control or a
  network command surface.
- Keep the existing presentation socket and snapshot schema one-way and
  unchanged. Fixture navigation is a local development capability separate
  from the Live Mode state boundary.
- In the focused renderer window, map Left to Previous and Right to Next.
  Suppress operating-system repeats until the pressed arrow is released, while
  accepting distinct rapid presses and allowing the latest deliberate
  selection to supersede an in-progress transition.
- Publish every selected Fixture Scenario as the latest complete snapshot over
  the existing presentation path. Allocate monotonic revisions within the
  Fixture Mode session rather than reusing unordered stored fixture revisions.
- Re-anchor Playing progress whenever its Fixture Scenario is selected and
  reset Fixture Mode inactivity timing on every selection. Do not change Live
  Mode progress or inactivity semantics.
- Use the normal immediate-replacement and crossfade policies for selected
  scenarios. Do not add a fixture-only transition or bypass the renderer's
  presentation state.
- Log the initially selected Fixture Scenario and every subsequent selection
  to the terminal. Do not add an onscreen scenario name, navigation hint, or
  control.
- Preserve the existing explicit `ROONSCAPE_FIXTURE` contract unchanged. When
  it is set, launch only the requested fixture and do not activate arrow
  navigation.
- Preserve the existing Escape shortcut, window-close behavior, launcher
  cleanup, fullscreen behavior, and windowed Fixture Mode behavior.

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
- Use Fixture Mode for renderer development and manually inspect Gallery split
  at 4K on the actual OLED. Check ordinary metadata, very long metadata,
  missing artwork, full artwork-derived palettes, crossfade, and inactive
  repositioning.
- Treat the selected Gallery split prototype and its verdict as prior visual
  art. Reimplement the decision in GTK rather than comparing production code
  to prototype DOM or CSS details.
- Perform a small local IPC smoke check of the focused bridge and renderer:
  connect, receive current state, restart each process once, and confirm
  reconnection without stale content. Deployment acceptance separately verifies
  recovery of the complete foreground RoonScape session.
- On the Reference Deployment, exercise ordinary playing, paused, loading,
  stopped, missing artwork, and unavailable states; confirm the normal boot and
  return-to-input workflow.
- Glance at process memory, CPU, and swap during normal use to catch an obvious
  regression. Do not enforce numerical budgets, a 72-hour soak, hundreds of
  synthetic track changes, or a formal television-state matrix.
- Use the Fixture Mode control-to-presentation boundary as the primary automated
  seam. Given Previous or Next at the private control channel, observe the
  complete snapshot published on the existing presentation socket rather than
  asserting publisher helpers or catalog implementation details.
- At the primary seam, verify all 18 Fixture Scenarios in their accepted order,
  backward and forward wraparound, startup at Playing, monotonic session
  revisions, re-anchored Playing progress, and latest selection behavior under
  rapid navigation.
- Extend the existing toolkit-independent renderer keyboard and presentation
  policy seams to verify Left and Right mapping, focused Fixture Mode gating,
  held-repeat suppression, distinct rapid presses, fresh inactivity timing,
  inert Live Mode arrows, and unchanged Escape behavior. Do not make automated
  tests depend on GTK internals.
- Extend the existing process-level Fixture Mode launcher seam to verify that
  ordinary sessions activate navigation and the explicit fixture override
  retains single-fixture behavior.
- Verify catalog startup failure when any of the 18 entries is missing or
  invalid, and verify that the capture workflow derives its ordered scenarios
  from the same catalog as interactive Fixture Mode.
- Manually smoke-test the native GTK renderer in windowed and fullscreen
  Fixture Mode. Confirm focus handling, terminal scenario names, ordinary
  crossfades, rapid selections, inactivity reset, and the absence of onscreen
  fixture controls. Also confirm that Live Mode does not respond to Left or
  Right.
- Do not add pixel-golden tests or automated screenshot-difference gates for
  navigation. Continue judging visual output through the existing repeatable
  capture and physical-display workflows.

## Out of Scope

- Any Roon Control capability, including play, pause, stop, previous, next,
  volume, loop, shuffle, Roon Radio, grouping, or other mutations.
- A LAN remote, network API, public HTTP service, browser UI, or browser engine.
- Library browsing, search, queue management, notifications, settings screens,
  theme selection, and user-facing diagnostics during normal operation.
- Automatic active-zone selection, fallback zones, multi-zone presentation,
  or switching away from the configured Tracked Output.
- Television power control, input switching, HDMI-CEC behavior, or making the
  display responsible for television availability.
- Lyrics, which are deferred for now.
- Broad hardware compatibility guarantees beyond the Reference Deployment.
- Enterprise release machinery, hard resource budgets, long formal soak
  tests, and exhaustive hardware-state matrices.
- Arrow-key scenario navigation in Live Mode or when the explicit
  single-fixture override is active.
- Global keyboard shortcuts, mouse or touch navigation, direct scenario-jump
  keys, menus, settings, and onscreen Fixture Mode controls.
- Adding specialist, invalid-contract, or Display Configuration fixtures to the
  18-scenario navigation catalog.
- Persisting the selected Fixture Scenario between sessions or hot-reloading
  fixture files during a running session.
- Changing the presentation snapshot schema, making its socket bidirectional,
  or letting the renderer load fixture files directly.

## Further Notes

- RoonScape is a personal hobby project. Prefer direct, understandable code and
  implementation judgment over speculative generality.
- The name RoonScape is intentional: it combines Roon with a visual “scape” and
  embraces the RuneScape homophone. Existing small naming collisions are
  accepted because public discoverability is not a goal.
- The Reference Deployment is an Intel NUC RoonScape Host with about 4 GiB RAM,
  Linux, systemd, Xorg, and an attached LG 3840×2160 OLED television. It also
  runs Roon Server, but co-location is not a RoonScape requirement.
- The Reference Deployment has the required Node.js, Rust, GTK 4, Pango, Mesa,
  and Xorg building blocks but no active graphical session. Its historical Xorg
  setup used an obsolete graphics driver and should not be copied unchanged.
  Exact inactive luminance and timing should be calibrated by looking at the
  physical screen rather than specified in advance.
- RoonScape is expected to run on compatible Linux/GTK RoonScape Hosts. Other
  hardware, display resolutions, and graphical session arrangements remain
  unverified until another deployment exercises them.
- The user chose Variant A, Gallery split, from a five-variant throwaway visual
  prototype. The current direction presents Tracked Output and Tracked Zone as
  **Output** and **Zone** and derives every presentation color from the actual
  artwork.
- The full visual prototype is preserved only on the
  `prototype/visual-variants` branch.
- Fixture Mode, Live Mode, and Fixture Scenario are canonical glossary terms.
  Fixture Mode navigation is a development workflow and does not change
  RoonScape's unattended, read-only Live Mode product behavior.
- This feature does not require a prototype or an architecture decision record.
  After this specification is accepted, split its implementation into
  agent-ready tickets under the existing RoonScape effort.
