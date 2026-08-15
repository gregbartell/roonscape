# RoonScape

This context describes a personal, unattended visual display of Roon playback
on a RoonScape Host.

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

**Display Output**:
The single physical Roon output configured on the RoonScape Host whose playback
RoonScape presents, including while the output joins or leaves a group.
_Avoid_: Selected zone, active zone, fallback zone

**Display Zone**:
The current Roon zone containing the Display Output. It can change when the
Display Output is grouped or ungrouped. RoonScape presents its name to the
viewer under the label **Zone**.
_Avoid_: Configured zone, fixed zone

**Now Playing**:
The Roon-provided content currently associated with the Display Zone.
_Avoid_: Now-playing presentation, current content

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

**Title**:
Roon's first Now Playing display line, presented as the track title even though
the API does not independently guarantee that semantic.
_Avoid_: Primary text, line 1
