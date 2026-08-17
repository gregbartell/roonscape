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

**Idle**:
The viewer-facing state used when an available Tracked Output has no current
playback. It corresponds to Roon's stopped playback state and contains no Now
Playing content.
_Avoid_: Stopped in viewer-facing copy

**Live Mode**:
The normal RoonScape workflow in which its presentation reflects current state
observed from Roon.
_Avoid_: Live version, production mode

**Now Playing**:
The Roon-provided content currently associated with the Tracked Zone.
_Avoid_: Now-playing presentation, current content

**Presentation Status**:
The viewer-facing condition label that identifies RoonScape's current
playback or availability condition, such as Playing, Paused, Idle, or
Disconnected.
_Avoid_: Screen name, eyebrow

**Roon Authorization**:
Roon's persisted approval for RoonScape to connect as an extension. It is
independent of Display Configuration.
_Avoid_: Pairing state, extension credentials

**Roon Control**:
An action that changes Roon playback, volume, or playback settings. Roon
Control is outside this product's scope.
_Avoid_: Display interaction

**RoonScape**:
An unattended, read-only presentation of current Roon playback.
_Avoid_: Roon Display, web controller, remote

**RoonScape Host**:
The machine that runs RoonScape and drives its attached display. It may also
run Roon Server or unrelated workloads.
_Avoid_: Dedicated host

**Starting**:
The viewer-facing playback condition used while Roon reports that the Tracked
Zone is loading, whether or not Now Playing content is available.
_Avoid_: Loading in viewer-facing copy, Preparing

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
