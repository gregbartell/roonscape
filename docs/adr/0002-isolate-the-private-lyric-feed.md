# Isolate the private Lyric Feed

The RoonScape Bridge may open a second connection to the exact Roon Core paired
with its ordinary connection, using Roon's JavaScript implementation and the
first-party `com.roonlabs.display_zone` identity solely to observe private
`LyricsChanged` continuations. This narrow exception to ADR 0001 is accepted
because the ordinary `io.roonscape.bridge` registration does not receive the
data required for synchronized lyrics; the Lyric Feed remains optional and
must not delay or destabilize ordinary Presentation Snapshots or alter Roon
playback.

The Lyric Feed advertises no concrete displays and never activates,
deactivates, or reconfigures a Web Display. It owns any registry token only in
memory for the connection lifetime, discards it on shutdown, and keeps private
identity, opaque keys, LRC parsing, view reporting, and transport details behind
one Bridge-side module. The RoonScape Renderer receives only a bounded,
normalized timed-cue timeline in the Presentation Snapshot contract.
