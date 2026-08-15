# 08 — Recover and expose diagnostics

**What to build:** Make the bridge and renderer tolerate ordinary failures and
start in either order without intervention. When troubleshooting is requested,
the owner can enable a compact diagnostics overlay; during normal listening,
the viewer sees no diagnostic UI.

**Blocked by:** 04 — Present live Now Playing and artwork; and 05 — Make
playback and progress truthful.

**Status:** ready-for-agent

- [ ] Either process can start first or restart independently, and the renderer
  reconnects indefinitely without terminating its own session.
- [ ] Every renderer connection receives the latest complete snapshot
  immediately; disconnected periods never leave stale Now Playing content
  presented as current.
- [ ] Message size, frame count, and pending output are bounded; a stalled
  renderer can be disconnected instead of causing event-history growth.
- [ ] Artwork and transition cleanup remain bounded across disconnects,
  reconnects, and rapid presentation changes.
- [ ] A host-enabled overlay reports memory, frame timing, artwork dimensions,
  connection state, and state revision and is disabled by default.
- [ ] An IPC smoke check restarts each process once and confirms reconnection,
  current-state replay, and truthful availability.
- [ ] Automated checks cover malformed or oversized frames, stalled clients,
  process order, and reconnect behavior without asserting private helpers.
