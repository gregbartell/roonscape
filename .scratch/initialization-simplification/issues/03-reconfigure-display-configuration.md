# 03 — Reconfigure Display Configuration safely

**What to build:** Let an owner reopen setup to deliberately change the Tracked
Output or OLED inactivity behavior without damaging a working Display
Configuration or unexpectedly starting the presentation.

**Blocked by:** 02 — Guide first-time setup into the presentation.

**Status:** ready-for-agent

- [ ] `roonscape --setup` always opens the interactive setup flow, saves a
      completed Display Configuration, and exits without launching the bridge or
      renderer.
- [ ] Reconfiguration highlights the saved Tracked Output and prefills the
      existing inactivity grace period, dimmed opacity, and reposition cadence.
- [ ] The owner can keep the defaults or customize each inactivity value in
      familiar units with immediate validation and a clear correction path.
- [ ] Changing only the Tracked Output preserves the saved inactivity values,
      while changing only inactivity behavior preserves the selected Tracked
      Output.
- [ ] The explicit `--config PATH` location applies consistently to setup and
      normal launch and remains the sole public nonstandard configuration path.
- [ ] Cancelling or failing reconfiguration leaves the prior Display
      Configuration byte-for-byte intact; successful completion validates and
      atomically replaces it with private permissions.
- [ ] No additional noninteractive setup API, graphical settings surface,
      browser interface, network endpoint, or Roon Control capability is added.
- [ ] Command-level tests cover current-choice highlighting, prefilled and
      customized values, preservation rules, invalid entries, cancellation,
      explicit paths, atomic replacement, and save-without-launch behavior.
