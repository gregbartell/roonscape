# 03 — Select and follow the Display Output

**What to build:** Give the owner a one-time RoonScape Host workflow for
discovering and saving one physical Display Output, then continuously resolve
the Display Zone that contains it. The viewer follows that physical output
through ordinary Roon grouping changes and never switches to whichever
unrelated zone becomes active.

**Blocked by:** 02 — Explain Roon connection availability.

**Status:** ready-for-agent

- [ ] A practical RoonScape Host workflow lists discoverable outputs and saves
  one Display Output without requiring an onscreen or network settings
  interface.
- [ ] Display Configuration is stored separately from Roon authorization state
  and can be changed without changing Roon playback.
- [ ] Initial zone state, grouping, ungrouping, and zone renaming all resolve
  the configured Display Output to its current Display Zone.
- [ ] Removal or invalid configuration produces output unavailable and clears
  stale Now Playing content rather than selecting a fallback output.
- [ ] The viewer presents the Display Zone name under the label **Zone** and
  never exposes the internal Display Output identity.
- [ ] Source-event tests cover full zone state, grouping, ungrouping, renaming,
  removal, and unrelated active-zone changes.
