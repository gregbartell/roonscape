# RoonScape Plan

## Goal

Build RoonScape as a lightweight, unattended now-playing display for `roll`,
the always-on Roon Server connected to a 4K OLED television. This is a personal
hobby project: favor a small, understandable implementation and normal-use
confidence over exhaustive infrastructure or release gates.

This is an agreed design, not a claim that RoonScape has been implemented or
installed.

## Product Boundary

RoonScape is display-only. It reads Roon state but never changes playback,
volume, grouping, or playback settings.

- Do not preserve the Web Controller, a network-facing UI, or command APIs.
- Do not embed Chromium, Electron, WebKit, or another browser engine.
- Do not include library browsing, search, notifications, a settings screen,
  theme selection, multi-zone management, or playback controls.
- Keep Display Configuration host-managed. Its exact file or command shape is
  an implementation detail because setup happens rarely.
- Observe one configured Display Output and follow the Display Zone that
  currently contains it. Never switch automatically to another output.
- Treat the television as external: do not power it or change its input.
  RoonScape remains ready while the television is off or showing another input.
- Register as a new Roon extension with fresh authorization state rather than
  inheriting the legacy Web Controller identity.

Lyrics are deferred for now.

## Viewer Experience

Use the selected prototype's **Gallery split** direction:

```text
┌────────────────────────────────────────────┐
│                                            │
│   ┌────────────────┐   TITLE               │
│   │                │   Artist              │
│   │    artwork     │   Album               │
│   │                │                       │
│   └────────────────┘   ━━━━━━━╸━━━━        │
│                         Zone · Playing      │
│                                            │
└────────────────────────────────────────────┘
```

- Make artwork the dominant object on the left and give metadata a dedicated
  column on the right.
- Show only the viewer-facing label **Zone**. Display Output identity remains
  internal and never appears onscreen.
- Treat Roon's first, second, and third display lines as Title, Artist, and
  Album. Missing lines remain absent rather than receiving invented values.
- Derive the entire palette, including text, from the current artwork. Choose
  readable combinations, but first try the expressive fully recolored result
  before constraining it to permanent product colors.
- Use a neutral palette when artwork is absent.
- Pair an editorial serif Title and Album treatment with clean sans-serif
  Artist and utility text. Select the exact fonts during on-TV implementation.
- Wrap and reduce long metadata within firm bounds, then ellipsize extreme
  cases. Do not use a perpetual marquee.
- Crossfade artwork, text, and palette together on track changes. Avoid other
  ambient motion except progress and OLED protection.
- Do not show persistent RoonScape branding.

### Playback and availability

- **Playing**: show artwork, metadata, explicit state, Zone, and determinate
  elapsed/remaining progress when Roon supplies valid position and duration.
- **Paused**: freeze progress and retain the current composition briefly, then
  dim and periodically reposition it to protect the OLED. Calibrate the exact
  timing and dim level on the physical television.
- **Loading**: retain any supplied Now Playing content, freeze progress, and
  show the loading state. Use a neutral loading screen when content is absent.
- **Stopped**: clear track-specific content and show a neutral Zone-specific
  idle state before applying the OLED-safe inactive treatment.
- **Pairing required**, **disconnected**, and **output unavailable**: clear
  stale track content and explain the condition before applying the inactive
  treatment.
- **Missing artwork or indeterminate progress**: treat both as normal. Do not
  fabricate artwork, time values, or metadata.

The renderer interpolates progress locally only while playing, re-anchors when
Roon supplies a new position, and hides progress when position or duration is
not meaningful.

## Architecture

Build a greenfield repository named `roonscape`; do not extract legacy Web
Controller code.

```text
Roon Server
    │ official public node-roon-api services
    ▼
Roon bridge (TypeScript / Node.js)
    │ private Unix socket: latest versioned snapshot
    │ bounded, atomically replaced artwork files
    ▼
Native renderer (Rust / GTK 4 / Pango)
```

Keep the bridge and renderer in one repository and one coordinated release.
A language-neutral JSON Schema and shared fixtures define their contract.
Neither process imports the other's framework types.

### Bridge

- Use Roon's official Transport, Image, and Status services for discovery,
  pairing, zone subscription, artwork, and extension status.
- Do not load Browse or expose any Roon Control capability.
- Resolve the configured Display Output into its current Display Zone on each
  update so grouping and ungrouping behave predictably.
- Retain only the latest complete presentation state, with a schema version and
  monotonic revision. Merge Roon's seek-only deltas into retained zone state.
- Keep Roon authorization state separate from ordinary Display Configuration.
- Provide some one-time host workflow for discovering and selecting the
  Display Output without prescribing that workflow in this plan.

### Renderer and local interface

- Send a complete current snapshot immediately when the renderer connects and
  whenever presentation state changes. Bound frames and pending output; never
  accumulate event history.
- Keep socket permissions local to the display account and open no network
  port.
- Pass compressed artwork through a private runtime directory rather than
  embedding bytes in state JSON.
- Retain at most current artwork and one outgoing artwork during the short
  crossfade. Use a small derivative for palette analysis.
- Reconnect indefinitely after either process restarts and show truthful
  availability rather than stale Now Playing content.
- Include an optional, host-enabled diagnostics overlay for memory, frame
  timing, artwork dimensions, connection state, and state revision. Keep it
  off during normal use.

## Operation on `roll`

Run the bridge independently as a system service. Run the renderer as a user
service tied to a guarded tty1 `startx` session; do not install a display
manager. Either service may restart without tearing down the other.

Relevant current host facts:

- `roll` is an Intel NUC8i3BEH with an i3-8109U, Iris Plus 655 graphics, about
  4 GiB RAM, and 4 GiB swap.
- Rust, Node.js, GTK 4, Pango, Mesa, and Xorg are installed.
- The connected LG television is 3840×2160 and is an OLED.
- No graphical session currently runs. tty1 autologin exists, while automatic
  `startx` is commented out.
- Historical Xorg configuration used the obsolete Intel DDX and a missing
  legacy GL driver. Prefer the standard Xorg modesetting driver and verify GTK
  rendering on the actual host.
- Prefer sharp RGB/4:4:4 output: use 4K60 when full bandwidth is available and
  otherwise favor 4K30 RGB/4:4:4 over 4K60 chroma subsampling.

Keep SSH, Tailscale, and Roon Server independent of the display. RoonScape must
not make display failure a music-service or remote-access failure.

## Hobby-Sized Implementation Sequence

### 1. Establish the greenfield repository

The repository and documentation migration are complete:

- `roonscape` exists as a local repository with no remote.
- `CONTEXT.md`, the ADRs, this plan, the specification, and the selected
  prototype verdict are on `main`.
- The full five-variant throwaway prototype is preserved on
  `prototype/visual-variants`; do not promote its code directly into the native
  renderer.
- No license file was added.

Next, establish TypeScript bridge, Rust renderer, shared schema/fixtures, and
`roll` deployment directories without copying legacy source.

### 2. Exercise the state seam

- Define fixtures for playing, paused, loading, stopped, long/missing metadata,
  missing artwork, pairing required, disconnected, and output unavailable.
- Make both modules consume those fixtures independently.
- Implement the smallest bridge that pairs, selects the Display Output, tracks
  its Display Zone, and publishes the latest snapshot.

### 3. Put Gallery split on the OLED

- Build the Rust/GTK renderer against fixtures first.
- Tune layout, fonts, full artwork-derived palettes, long text, progress,
  crossfade, and OLED-safe inactive behavior on the actual television.
- Confirm decoded artwork and animation work remain obviously bounded; use the
  optional diagnostics overlay when useful.

### 4. Integrate and install

- Connect the renderer to the bridge over the private socket.
- Add artwork handoff, local progress interpolation, reconnection, and the
  agreed availability states.
- Install independent systemd services and restore a guarded graphical session
  using the Xorg modesetting driver.
- Complete one-time pairing and Display Output configuration.

### 5. Use it

- Exercise the ordinary playback states, missing artwork, long metadata, and a
  restart of each process.
- Confirm normal boot and return-to-input behavior on the television.
- Glance at memory, CPU, and swap to catch an obvious problem, but do not turn
  resource guidance into hard release gates or build an enterprise soak suite.
- Adjust dimming, typography, palette selection, and display mode based on
  actual use.

The display is successful when it reaches the current Now Playing view without
routine intervention, does not interfere with Roon, and remains pleasant and
responsive during normal personal use.

## Intentionally Deferred Implementation Details

These choices do not need another design session unless implementation reveals
a real tradeoff:

- exact Display Configuration format and one-time selection command;
- exact socket framing and retry intervals;
- precise palette extraction and contrast algorithm;
- bundled font families and non-Latin fallback coverage;
- paused/idle dim level, grace period, and reposition cadence;
- whether bare X sizing is sufficient or a tiny window manager is useful;
- final 4K refresh mode after checking the television input; and
- numerical resource budgets beyond avoiding obvious growth or interference.
