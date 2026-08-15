# 09 — Run RoonScape unattended on a Linux host

**What to build:** Let an owner install RoonScape as a small appliance on a
compatible Linux RoonScape Host without baking that machine's identity into the
product. Normal boot reaches a ready display without local interaction, and a
display failure cannot take down Roon Server, other RoonScape Host workloads,
or remote administration.

**Blocked by:** 06 — Handle metadata and track transitions gracefully; 07 —
Protect the OLED during inactivity; and 08 — Recover and expose diagnostics.

**Status:** ready-for-agent

- [ ] The included Linux deployment profile runs the bridge as an independently
  supervised system service and the renderer as an independently supervised
  user service tied to the graphical session.
- [ ] The deployment profile can start a guarded Xorg session from tty1
  autologin without a display manager while allowing the graphical session and
  driver settings to be overridden for another host.
- [ ] The service account, runtime locations, graphical session command, and
  display mode enter through deployment configuration or overrides; changing a
  hostname requires no code edit or rebuild.
- [ ] Host variation remains in configuration and deployment templates rather
  than product identifiers, host-identity branches, or a speculative
  host-adapter framework.
- [ ] Runtime directories, artwork files, and the Unix-domain socket have
  permissions restricted to the display account, and no network listener is
  introduced.
- [ ] Service startup order is irrelevant: either process reconnects when its
  counterpart appears, and a failure or restart does not tear down the other
  service or graphical session.
- [ ] Roon authorization and Display Configuration survive ordinary service
  and RoonScape Host restarts while remaining separate from each other.
- [ ] The bridge discovers and connects to Roon without requiring Roon Server
  to run on the RoonScape Host.
- [ ] Disabling or failing RoonScape leaves Roon Server, other RoonScape Host
  workloads, and remote administration operational and does not control
  television power or input selection.
- [ ] A deployment smoke check reaches the current truthful presentation after
  normal boot without routine intervention.
