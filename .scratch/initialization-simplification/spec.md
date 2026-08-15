# Simplify RoonScape setup and launch

**Status:** ready-for-agent

## Problem Statement

An owner currently has to install Node and Rust build toolchains, install
project dependencies, run multiple configuration commands, copy an internal
Tracked Output ID by hand, create and secure a runtime directory, export a
socket path, and start the bridge and renderer separately. That workflow makes
first use difficult to understand and makes every later launch unnecessarily
fragile. RoonScape also has no release artifact that an owner can run without
building the product from source.

## Solution

Publish a relocatable Linux release containing a single owner-facing
`roonscape` command and everything RoonScape needs except the host's existing
GTK runtime. On first invocation without a Display Configuration, the command
runs a terminal wizard that guides Roon Authorization, lets the owner select a
Tracked Output with the arrow keys, offers OLED customization, writes the
Display Configuration, and then launches RoonScape. Later invocations launch
the bridge and renderer together without setup work. `roonscape --setup`
reopens the wizard for reconfiguration, and `roonscape --config PATH` lets an
owner or supervisor provide a Display Configuration and avoid interaction.

The command owns the private runtime directory, socket, singleton lock, and
the coupled lifetime of the bridge and renderer. Release users do not need
Node, npm, Rust, a compiler, or GTK development libraries.

## User Stories

1. As a RoonScape owner, I want to download a finished release, so that I do
   not need to build RoonScape from source.
2. As a RoonScape owner, I want the release to include its required Node
   runtime, so that I do not need to install or manage Node or npm.
3. As a RoonScape owner, I want the renderer to be precompiled, so that I do
   not need Rust, Cargo, a compiler, or GTK development libraries.
4. As a RoonScape owner, I want to extract the release anywhere, so that I do
   not need root access or an installer.
5. As a RoonScape owner, I want one `roonscape` command, so that I do not need
   to understand the bridge, renderer, socket, or process boundary.
6. As a first-time owner, I want bare `roonscape` to detect that setup is
   needed, so that I do not need to discover a separate first-run command.
7. As a first-time owner, I want setup to continue directly into the
   presentation, so that first use is one continuous operation.
8. As a returning owner, I want bare `roonscape` to launch immediately when my
   Display Configuration is valid, so that normal operation has no setup
   ceremony.
9. As an owner changing the RoonScape Host, I want `roonscape --setup` to
   reopen setup explicitly, so that reconfiguration is discoverable.
10. As an owner using `--setup`, I want the command to save and exit rather
    than opening the presentation, so that I can reconfigure safely over SSH.
11. As an owner, I want the wizard to explain how to enable RoonScape under
    Roon's extension settings, so that I can complete Roon Authorization
    without external instructions.
12. As an owner who needs time to use another device, I want the wizard to
    continue waiting for Roon Authorization, so that an arbitrary timeout does
    not discard my progress.
13. As an owner waiting for Roon, I want delayed troubleshooting guidance and
    explicit Retry and Quit actions, so that waiting never looks frozen.
14. As an owner, I want Roon Authorization to persist independently from
    Display Configuration, so that changing presentation choices does not make
    me authorize RoonScape again.
15. As an owner, I want to choose a Tracked Output with the arrow keys, so that
    I never have to copy or type an internal output ID.
16. As an owner, I want each choice to show the Tracked Output and its current
    Tracked Zone, so that I can distinguish physical outputs accurately.
17. As an owner with identically named outputs, I want the wizard to reveal
    enough identity information to disambiguate them, so that I can select the
    intended Tracked Output.
18. As an owner reconfiguring RoonScape, I want the saved Tracked Output to be
    highlighted, so that I can keep or change it deliberately.
19. As an owner whose Roon system currently exposes no outputs, I want a clear
    empty state with Refresh and Quit, so that setup does not silently create
    an unusable Display Configuration.
20. As an owner, I want Tracked Output selection to remain read-only with
    respect to Roon, so that setup never becomes Roon Control.
21. As an owner, I want setup to show the default OLED behavior in familiar
    units, so that I can make an informed choice without knowing the schema.
22. As an owner satisfied with the defaults, I want to accept them in one
    action, so that optional calibration does not become mandatory ceremony.
23. As an owner calibrating the attached display, I want to customize the
    inactivity grace period, dimmed opacity, and reposition cadence, so that
    OLED protection fits the installation.
24. As an owner entering custom OLED values, I want immediate validation and
    existing values prefilled, so that mistakes are easy to correct.
25. As an owner cancelling setup, I want the prior Display Configuration left
    untouched, so that an incomplete wizard cannot break a working display.
26. As an owner completing setup, I want the new Display Configuration written
    atomically, so that interruption cannot leave a partial file.
27. As an owner with a prepared Display Configuration, I want to provide it
    with `--config PATH`, so that first launch can be noninteractive.
28. As an owner providing a valid Display Configuration without existing Roon
    Authorization, I want RoonScape to launch into its pairing-required state,
    so that configuration provisioning genuinely bypasses the terminal wizard.
29. As an owner invoking RoonScape without a terminal, I want missing or
    invalid Display Configuration to produce a clear nonzero error, so that a
    boot supervisor never hangs inside an invisible wizard.
30. As an owner with a malformed Display Configuration in an interactive
    terminal, I want the problem reported before repair is offered, so that
    corruption is not silently hidden.
31. As an owner, I want persistent choices under my XDG configuration
    directory, so that configuration survives updates and runtime cleanup.
32. As an owner, I want Roon Authorization under my XDG state directory, so
    that persistent extension state remains separate from user-authored
    presentation choices.
33. As an owner, I want sockets and synchronization objects under my private
    XDG runtime directory, so that they have appropriate permissions and do
    not survive logout or reboot.
34. As an owner, I want the launcher to reject a second live RoonScape session,
    so that two instances cannot compete for one extension identity and
    physical display.
35. As an owner recovering after an interrupted launch, I want stale runtime
    artifacts reclaimed only after their former owner is known to be gone, so
    that recovery is safe.
36. As an owner, I want closing the renderer normally to stop the bridge and
    return success, so that the whole application has one understandable
    lifetime.
37. As an owner, I want a bridge failure to stop the renderer and surface a
    failure, so that RoonScape does not conceal crashes by restarting children.
38. As an owner, I want a renderer failure to stop the bridge and surface a
    failure, so that a failed presentation does not leave a hidden bridge
    running.
39. As an owner or supervisor terminating RoonScape, I want both children
    asked to stop cleanly, so that authorization, sockets, and artwork are not
    abandoned unnecessarily.
40. As an external supervisor, I want the launcher to stop a stuck child after
    a finite grace period, so that shutdown cannot hang forever.
41. As an external supervisor, I want RoonScape's exit result to reflect the
    child that ended the session, so that failures remain observable.
42. As an owner starting RoonScape at boot, I want one foreground command, so
    that I can choose my own boot integration without managing two services.
43. As an owner troubleshooting a release, I want `--help` and `--version`, so
    that I can identify the supported interface and installed build.
44. As an owner updating RoonScape, I want to replace the release directory
    without losing Display Configuration or Roon Authorization, so that an
    updater is unnecessary.
45. As a source developer, I want npm commands to exercise the same setup and
    launch behavior as the release, so that the production path receives daily
    use.
46. As a source developer, I want bridge-only and fixture workflows to remain
    available for focused work, so that unified launch does not weaken
    diagnostics or testing.
47. As a release maintainer, I want one reproducible packaging command, so that
    local and CI archives cannot drift.
48. As a release maintainer, I want tagged releases to publish the archive and
    checksum automatically, so that owners receive a repeatable artifact.
49. As a release maintainer, I want a narrow initial platform promise, so that
    release compatibility is testable rather than implied for every Linux
    system.

## Implementation Decisions

- The public release command is `roonscape`. With a valid Display
  Configuration it starts RoonScape; with no Display Configuration and an
  interactive terminal it runs first-time setup and then starts RoonScape.
- `roonscape --setup` always runs the interactive setup flow, saves a completed
  Display Configuration, and exits without starting the presentation.
- `roonscape --config PATH` selects an explicit Display Configuration for both
  setup and normal launch. It takes precedence over the standard XDG location.
- The remaining public command surface is limited to `--help` and `--version`.
  No noninteractive setup flags are added; a supplied Display Configuration is
  the noninteractive setup path.
- The TypeScript CLI owns the wizard and launcher behavior and runs on the
  private Node runtime carried by the release. The normal launcher starts the
  bridge in a separate Node child and starts the precompiled native renderer as
  another child.
- The bridge remains Node.js because Roon's supported integration API is
  JavaScript. The renderer remains native Rust with GTK 4 and Pango. This
  preserves the architectural boundary while giving both processes one
  owner-facing lifecycle.
- Setup uses an interactive terminal locally or over SSH. Missing or invalid
  Display Configuration without a TTY produces an actionable error and a
  nonzero exit instead of waiting for input.
- An invalid existing Display Configuration is reported before an interactive
  repair is offered. The invalid file is not overwritten unless the owner
  completes the wizard.
- The wizard explains Roon's extension authorization flow and waits without a
  terminal deadline. After a reasonable delay it replaces the ordinary waiting
  message with troubleshooting guidance while continuing to wait. The owner
  can retry discovery or quit.
- The output chooser is keyboard-driven. Each item presents the Tracked Output
  name and current Tracked Zone. Internal IDs remain storage details and appear
  only when otherwise identical labels require disambiguation.
- Output discovery remains read-only and never invokes a Roon Control method.
  If no Tracked Outputs are discoverable, setup offers Refresh and Quit rather
  than saving an empty or guessed selection.
- First-time setup offers the existing OLED defaults: a five-minute grace
  period, 35 percent dimmed opacity, and repositioning every minute. The owner
  can accept all defaults or customize each existing inactivity value with
  immediate validation.
- Reconfiguration loads and displays the current Tracked Output and inactivity
  values. Changing the Tracked Output preserves inactivity values unless the
  owner explicitly changes them.
- The complete Display Configuration is validated and written privately and
  atomically only after setup succeeds. Cancelling or failing setup leaves the
  prior configuration unchanged.
- A supplied valid Display Configuration bypasses setup even when Roon
  Authorization is absent. Normal live startup exposes the existing
  pairing-required presentation while the owner authorizes RoonScape through a
  Roon client.
- Display Configuration defaults to
  `$XDG_CONFIG_HOME/roonscape/display.json`, with the standard
  `~/.config/roonscape/display.json` fallback. Roon Authorization defaults to
  `$XDG_STATE_HOME/roonscape/authorization.json`, with the standard
  `~/.local/state/roonscape/authorization.json` fallback.
- The existing Display Configuration path environment override is removed.
  There are no existing release users requiring compatibility, and
  `--config PATH` is the sole public override.
- Existing windowed and diagnostics environment controls remain advanced
  development or troubleshooting behavior rather than wizard questions. The
  authorization-store override may remain an advanced internal facility. The
  socket variable becomes private launcher-to-child plumbing.
- The launcher creates a mode-0700 runtime directory beneath
  `$XDG_RUNTIME_DIR`, or uses a validated `/run/user/<uid>` equivalent when the
  environment variable is absent. It fails with remediation when neither safe
  location exists; it does not place sockets or locks in the persistent
  configuration directory or a predictable `/tmp` directory.
- The launcher owns a per-user singleton lock, socket, and bounded artwork
  staging area. A second invocation fails clearly when the live owner is still
  present. Stale artifacts are reclaimed only after confirming that no live
  launcher owns them.
- The bridge and renderer form one runtime session. Either child exiting begins
  shutdown of the other. The launcher does not restart a failed child.
- During shutdown the peer receives `SIGTERM` and has five seconds to stop
  cleanly before forced termination. Launcher termination applies the same
  policy to both children.
- A normal initiating exit, including closing the renderer with Escape,
  results in successful launcher exit after cleanup. A crash, signal, or
  nonzero child exit remains a launcher failure. The initiating child's result
  determines the session result.
- An external boot supervisor may restart the complete `roonscape` command.
  RoonScape does not install or configure that supervisor in this effort.
- Source development exposes `npm run setup`, `npm start`, and
  `npm run package`. The first two exercise the same CLI orchestration as the
  release. Focused bridge, fixture, check, and test commands remain available.
- The old owner workflow that lists outputs and requires copying an internal ID
  is no longer the documented setup path. Its reusable discovery and
  configuration logic is retained behind the wizard rather than duplicated.
- The initial release is a relocatable, versioned x86-64 archive for
  glibc-based Linux systems, built against an Ubuntu 22.04-era compatibility
  baseline and requiring GTK 4.6 or newer at runtime. ARM64, musl, and older
  GTK targets are not included.
- GTK is an existing renderer dependency, not a new product dependency. The
  release changes the requirement from GTK development libraries needed to
  build from source to the GTK runtime needed to execute the precompiled
  renderer.
- The archive contains a top-level executable wrapper, the TypeScript CLI and
  bridge JavaScript, production JavaScript dependencies, schemas, a pinned
  private Node runtime, and the precompiled renderer. It is relocatable and
  runs as `./roonscape` without an installation step; owners may place or link
  it on `PATH` themselves.
- One reproducible npm packaging command builds and stages the complete archive
  and checksum. Tag-triggered GitHub release automation invokes that command
  rather than reimplementing packaging steps.
- Updates consist of replacing the release archive. Persistent Display
  Configuration and Roon Authorization remain outside the release directory.
  No automatic updater or version manager is introduced.
- Product documentation separates the release-owner workflow from the source
  developer workflow and removes the repeated socket-export and two-terminal
  launch instructions from the normal path.
- The product specification and unattended-deployment ticket must be reconciled
  with the revised lifecycle decision: a boot supervisor manages one complete
  RoonScape runtime session rather than independently restarting the bridge and
  renderer.

## Testing Decisions

- Good tests assert behavior visible at the top-level RoonScape command:
  terminal output and choices, resulting Display Configuration, child launch
  and signals, runtime cleanup, and final exit status. They do not assert prompt
  library internals, child-process implementation details, or private helper
  call sequences.
- The primary behavioral seam is one top-level command runner with injected
  adapters for terminal input/output, Roon discovery, configuration and state
  locations, runtime ownership, child processes, signals, and time. Tests drive
  this seam as an owner would drive `roonscape` while keeping Roon and GTK
  deterministic.
- Existing configuration-command tests provide prior art for invoking a
  top-level operation with controlled dependencies. Existing fixture-launcher
  tests provide prior art for observing child exit, signal propagation, and
  runtime cleanup. The new command seam consolidates these behaviors rather
  than adding separate seams for every wizard screen and launcher helper.
- First-run tests cover authorization guidance, indefinite waiting with delayed
  troubleshooting text, Retry and Quit, arrow-key Tracked Output selection,
  duplicate-label disambiguation, no-output refresh, default OLED values,
  customized values, validation, atomic save, and continuation into launch.
- Reconfiguration tests cover the highlighted current Tracked Output, prefilled
  inactivity values, preserving values when only the output changes, successful
  replacement, cancellation, and `--setup` exiting without presentation
  launch.
- Configuration-entry tests cover the standard XDG path, `--config` precedence,
  valid supplied configuration bypassing setup, absent Roon Authorization,
  malformed configuration, interactive repair, and noninteractive failure.
- Runtime tests cover secure directory creation, singleton exclusion, safe
  stale-artifact recovery, bridge-first and renderer-first readiness, normal
  renderer exit, bridge failure, renderer failure, launcher signals, graceful
  peer termination, five-second escalation, result propagation, and cleanup.
- The packaging check is intentionally narrow. It builds and unpacks the
  archive, verifies expected contents and executable permissions, verifies the
  checksum, and runs `./roonscape --help` and `./roonscape --version` with
  system Node, npm, Cargo, and Rust excluded from the command search path. This
  proves that the wrapper finds the bundled runtime without constructing a
  pretend deployment environment.
- The packaging check does not attempt a full clean-host GTK/Roon session in a
  container. Existing renderer, snapshot, fixture-launcher, and IPC tests
  continue to cover their established behavior.
- Manual acceptance on a compatible RoonScape Host verifies that the unpacked
  release loads against the host GTK runtime, completes real Roon
  Authorization, discovers and selects the intended Tracked Output, enters the
  presentation, exits cleanly, and starts again without setup.
- Repository-wide formatting, linting, typechecking, Rust tests, bridge tests,
  fixture-launcher tests, and IPC smoke checks remain required.

## Out of Scope

- Installing GTK or other operating-system dependencies.
- Installing or configuring systemd, Xorg, tty autologin, a display manager,
  or any other boot integration.
- Automatically restarting a failed bridge or renderer child inside the
  launcher.
- Automatic updates, a version manager, or migration between release layouts.
- Debian, RPM, AppImage, Flatpak, Snap, or other native package formats.
- ARM64, musl-based Linux, non-Linux systems, older GTK versions, or a broad
  cross-distribution compatibility guarantee.
- A graphical settings screen, browser interface, or network configuration
  endpoint.
- A second noninteractive setup API beyond supplying a valid Display
  Configuration.
- Rewriting the Roon bridge in Rust, merging the bridge and renderer into one
  process, or introducing a browser engine.
- Changing Roon playback, volume, grouping, or any other Roon Control.
- Embedding a real Roon Server or graphical desktop session in automated
  packaging checks.
- Final Reference Deployment boot acceptance and physical OLED calibration.

## Further Notes

- The unattended-deployment effort remains responsible for choosing how the
  single foreground `roonscape` command starts at boot. This specification
  deliberately gives that effort one stable command to supervise without
  taking ownership of privileged host changes.
- Roon Authorization is persistent application state rather than a
  user-authored presentation choice. Keeping it under the XDG state location
  preserves the existing separation from Display Configuration.
- The runtime-session lifecycle supersedes earlier requirements for independent
  bridge and renderer supervision. Process separation, private local IPC, and
  the absence of Roon Control remain unchanged.
- The repository currently has no release artifact or release workflow. This
  work establishes the first owner-consumable release layout, so no compatibility
  shim is required for the removed Display Configuration environment override.
