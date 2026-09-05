# RoonScape

This context describes an unattended visual display of Roon playback on a
RoonScape Host.

Keep the Language section alphabetized by vocabulary term.

## Language

**Album**:
Roon's third Now Playing display line, presented as the album even though the
API does not independently guarantee that semantic.
_Avoid_: Tertiary text, line 3

**Artist**:
Roon's second Now Playing display line, presented as the artist even though the
API does not independently guarantee that semantic.
_Avoid_: Secondary text, line 2

**Authoritative Timing**:
Playback position or duration observed directly from Roon, independently of
whether both are currently available.
_Avoid_: Reported timing, real timing

**Display Configuration**:
A choice that changes what or how RoonScape presents without changing
Roon state.
_Avoid_: Roon Control

**Fixture Mode**:
A development workflow in which RoonScape presents predefined Fixture
Scenarios without requiring current Roon state.
_Avoid_: Fixture version, test mode

**Fixture Scenario**:
A predefined RoonScape presentation used for repeatable visual inspection in
Fixture Mode.
_Avoid_: Fixture screen, mode, screen

**Full-field Presentation**:
A viewer-facing presentation for a condition that has no useful Now Playing
composition and instead uses Presentation Status with concise editorial copy.
_Avoid_: Screen without artwork, no-art screen, full-field state

**Idle**:
The viewer-facing state used when an available Tracked Output has no current
playback. It corresponds to Roon's stopped playback state and contains no Now
Playing content.
_Avoid_: Stopped in viewer-facing copy

**Intentional Blank**:
A timed empty lyric cue marking a pause in focal lyrics. Within the
Synchronized Lyric Composition, its settled presentation leaves the focal
position empty while retaining available Previous Cue and Next Cue context;
leading blanks do not establish that composition.
_Avoid_: Missing lyric, composition exit

**Live Capture Frame**:
A retained screenshot from a Live Capture Session that records a visually
meaningful presentation state.
_Avoid_: Presentation Capture, raw frame

**Live Capture Session**:
A bounded observation of RoonScape in Live Mode that produces a curated visual
chronology for human verification.
_Avoid_: Presentation Capture session, live test, recording

**Live Mode**:
The normal RoonScape workflow in which its presentation reflects current state
observed from Roon.
_Avoid_: Live version, production mode

**Lyric Composition Transition**:
The viewer-facing change into or out of the Synchronized Lyric Composition.
It is distinct from a Natural Cue Handoff, an intentional blank, and an
external seek.
_Avoid_: Lyric transition, lyric layout change

**Lyric Feed**:
An optional RoonScape Bridge capability that observes synchronized lyric cues
for the current Tracked Zone without controlling playback or a Web Display.
_Avoid_: Lyrics service, Web Display connection

**Natural Cue Handoff**:
The viewer-facing progression from one nonblank timed lyric cue to its
immediately adjacent nonblank cue as local playback advances.
_Avoid_: Lyric transition, cue change

**Now Playing**:
The Roon-provided content currently associated with the Tracked Zone.
_Avoid_: Now-playing presentation, current content

**Presentation Capture**:
A screenshot artifact of a Fixture Scenario generated for repeatable human
review of RoonScape's presentation.
_Avoid_: Fixture screenshot, screen capture

**Presentation Snapshot**:
A complete point-in-time description of the Roon state needed to determine a
RoonScape presentation.
_Avoid_: Event, update, renderer state

**Presentation Status**:
The viewer-facing condition label that identifies RoonScape's current
playback or availability condition, such as Playing, Paused, Idle, or
Disconnected.
_Avoid_: Screen name, eyebrow

**Previous Cue**:
The nearest nonblank timed cue preceding the current cue in the lyric
timeline. It provides destination-relative context and is not a history of
what the viewer actually saw.
_Avoid_: Last-played lyric, displayed-cue history

**Provisional Timing**:
A short-lived viewer-facing estimate that preserves determinate timing while
Authoritative Timing is briefly incomplete during continuous Live Mode.
_Avoid_: Fabricated progress, interpolated timing

**Reel Lift**:
The selected viewer-facing treatment for a Natural Cue Handoff, in which the
incoming cue rises into focus as the outgoing cue moves upward, retaining
Previous Cue context when the settled hierarchy permits.
_Avoid_: Lyric scroll, lyric crossfade

**Roon Authorization**:
Roon's persisted approval for RoonScape to connect as an extension. It is
independent of Display Configuration.
_Avoid_: Pairing state, extension credentials

**Roon Bridge**:
Roon's separate application named Roon Bridge. It is not a RoonScape
component.
_Avoid_: RoonScape Bridge

**Roon Control**:
An action that changes Roon playback, volume, or playback settings. Roon
Control is outside this product's scope.
_Avoid_: Display interaction

**Roon Server Host**:
A hostname or IPv4 address identifying the Roon Server RoonScape should contact
when ordinary Roon discovery is unsuitable.
_Avoid_: Core endpoint, direct Core, Roon Core Host

**RoonScape**:
An unattended, read-only presentation of current Roon playback.
_Avoid_: Roon Display, web controller, remote

**RoonScape Bridge**:
The part of RoonScape that observes Roon and supplies Presentation Snapshots.
_Avoid_: Roon Bridge

**RoonScape Host**:
The machine that runs RoonScape and drives its attached display. It may also
run Roon Server or unrelated workloads.
_Avoid_: Dedicated host

**RoonScape Renderer**:
The part of RoonScape that turns Presentation Snapshots into the viewer-facing
presentation.
_Avoid_: Frontend, Roon display

**Starting**:
The viewer-facing playback condition used while Roon reports that the Tracked
Zone is loading, whether or not Now Playing content is available. Its
Presentation Status is `STARTING`; when Now Playing is unavailable, its
takeaway is `Preparing playback`.
_Avoid_: Loading as a viewer-facing condition, Preparing as a condition name

**Synchronized Lyric Composition**:
The Now Playing composition in which a timed lyric cue temporarily holds the
central viewer-facing role while artwork and compact metadata remain present.
For a continuously available timeline containing nonblank lyrics, its interval
spans the first nonblank cue through the final entry, including internal gaps
and subsequent Intentional Blanks, with preparation and a final hold.
_Avoid_: Lyrics screen, karaoke mode

**Title**:
Roon's first Now Playing display line, presented as the track title even though
the API does not independently guarantee that semantic.
_Avoid_: Primary text, line 1

**Tracked Output**:
The single physical Roon audio output configured on the RoonScape Host whose
playback RoonScape presents, including while the output joins or leaves a
group. It is not the host's video output; RoonScape presents its name under the
viewer-facing label **Output**.
_Avoid_: Display Output, selected zone, active zone, fallback zone

**Tracked Zone**:
The current Roon zone containing the Tracked Output. It can change when the
Tracked Output is grouped or ungrouped. RoonScape presents its name under the
viewer-facing label **Zone**.
_Avoid_: Display Zone, configured zone, fixed zone
