# 09 — Run RoonScape unattended on `roll`

**What to build:** Install RoonScape as a small appliance on `roll` so normal
boot reaches a ready display without local interaction. The bridge and renderer
remain independently supervised, use only local interfaces, and cannot take
down music service or remote administration when display components fail.

**Blocked by:** 06 — Handle metadata and track transitions gracefully; 07 —
Protect the OLED during inactivity; and 08 — Recover and expose diagnostics.

**Status:** ready-for-agent

- [ ] The bridge runs as an independently supervised system service and the
  renderer as an independently supervised user service tied to the graphical
  session.
- [ ] The existing tty1 autologin starts a guarded Xorg session without a
  display manager and uses the standard modesetting driver rather than the
  obsolete Intel driver configuration.
- [ ] Runtime directories, artwork files, and the Unix-domain socket have
  permissions restricted to the display account, and no network listener is
  introduced.
- [ ] Service startup order is irrelevant: either process reconnects when its
  counterpart appears, and a failure or restart does not tear down the other
  service or the graphical session.
- [ ] Roon authorization and Display Configuration survive ordinary service
  and host restarts while remaining separate from each other.
- [ ] Disabling or failing RoonScape leaves Roon Server, SSH, and Tailscale
  operational and does not control television power or input selection.
- [ ] A normal-boot smoke check reaches the current truthful presentation
  without routine intervention.
