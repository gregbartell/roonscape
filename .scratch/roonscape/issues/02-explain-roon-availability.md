# 02 — Explain Roon connection availability

**What to build:** Connect the bridge to Roon under a fresh RoonScape extension
identity and carry connection availability through the shared snapshot to the
viewer. A listener sees a truthful pairing-required, disconnected, or
connected-but-unconfigured explanation instead of stale Now Playing content.

**Blocked by:** 01 — Show a Playing fixture in Gallery split.

**Status:** ready-for-agent

- [ ] RoonScape registers with its own extension identity and uses Roon's normal
  client authorization flow with fresh authorization state.
- [ ] The bridge uses supported Roon JavaScript services for discovery,
  connection status, and extension status.
- [ ] Pairing-required and disconnected source conditions produce complete,
  schema-valid snapshots and distinct viewer-facing explanations.
- [ ] A connected bridge with no valid Display Configuration presents output
  unavailable without showing prior Now Playing content.
- [ ] Authorization state is suitable for storage separately from future
  Display Configuration.
- [ ] Source-event tests cover initial connection, authorization, disconnect,
  and reconnect transitions at the shared state seam.
- [ ] The bridge loads no Browse service, exposes no Roon Control capability,
  and creates no command endpoint, browser UI, or network listener.
