# 05 — Make playback and progress truthful

**What to build:** Represent Roon playback and timing without implying activity
that is not happening. Playing advances smoothly, paused and loading freeze,
stopped clears track-specific content, and content without meaningful timing
shows no fabricated progress.

**Blocked by:** 03 — Select and follow the Display Output.

**Status:** ready-for-agent

- [ ] Availability remains separate from the playing, paused, loading, and
  stopped playback states in complete versioned snapshots.
- [ ] Playing, paused, loading, and stopped source updates produce distinct,
  truthful presentations with an explicit state label.
- [ ] Loading retains supplied Now Playing content but uses a neutral loading
  presentation when content is absent; stopped always clears track-specific
  content.
- [ ] Progress exists only for finite position and positive duration, clamps at
  duration, and stays absent for indeterminate or live content.
- [ ] The renderer advances progress locally only while playing, freezes while
  paused or loading, and re-anchors whenever Roon supplies a new sample.
- [ ] Seek-position-only deltas merge into retained zone state before the
  bridge publishes a new complete snapshot with an increased revision.
- [ ] Shared fixtures and seam-level tests cover every playback state, valid
  and invalid timing, seek-only changes, and stale-content clearing.
