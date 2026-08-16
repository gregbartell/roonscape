# 03 — Reconfigure Display Configuration safely

**What to build:** Let an owner reopen setup to deliberately change the Tracked
Output or OLED inactivity behavior without damaging a working Display
Configuration or unexpectedly starting the presentation.

**Blocked by:** 02 — Guide first-time setup into the presentation.

**Status:** done

- [x] `roonscape --setup` always opens the interactive setup flow, saves a
      completed Display Configuration, and exits without launching the bridge or
      renderer.
- [x] Reconfiguration highlights the saved Tracked Output and prefills the
      existing inactivity grace period, dimmed opacity, and reposition cadence.
- [x] The owner can keep the defaults or customize each inactivity value in
      familiar units with immediate validation and a clear correction path.
- [x] Changing only the Tracked Output preserves the saved inactivity values,
      while changing only inactivity behavior preserves the selected Tracked
      Output.
- [x] The explicit `--config PATH` location applies consistently to setup and
      normal launch and remains the sole public nonstandard configuration path.
- [x] Cancelling or failing reconfiguration leaves the prior Display
      Configuration byte-for-byte intact; successful completion validates and
      atomically replaces it with private permissions.
- [x] No additional noninteractive setup API, graphical settings surface,
      browser interface, network endpoint, or Roon Control capability is added.
- [x] Command-level tests cover current-choice highlighting, prefilled and
      customized values, preservation rules, invalid entries, cancellation,
      explicit paths, atomic replacement, and save-without-launch behavior.

## Comments

### Implemented — 2026-08-15

Implemented in `67d242d`, with review cleanup in `524d584`. The final
Standards and Spec reviews reported no findings, and `npm run check` passed.
