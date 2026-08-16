# 04 — Support representative landscape viewports

**What to build:** Make RoonScape's visual support independent of one personal
4K deployment by treating common landscape displays at 1280x720 or larger as a
responsive range. Exercise ordinary, tall, wide, and high-resolution examples
without naming any resolution as canonical or weakening the presentation's
existing hierarchy and OLED-safe behavior.

**Blocked by:** 03 — Adopt Now Playing vocabulary and neutral fixtures.

**Status:** ready-for-agent

- [ ] No layout interface, capture planner, test, script message, or maintained
      source concept designates a reference or canonical viewport.
- [ ] Automated layout coverage includes 1280x720, 1600x1200, 1920x1200,
      2560x1080, and 3840x2160 as peer representative viewports.
- [ ] At every representative viewport, the Now Playing composition uses the
      available landscape field without unintended letterboxing or arithmetic
      overflow.
- [ ] Artwork remains contained, metadata remains bounded and readable,
      playback status and progress retain their hierarchy, and Output and Zone
      identities remain stable.
- [ ] Full-field states, diagnostics, transitions, and the OLED movement
      envelope remain within the available presentation field across the
      representative matrix.
- [ ] The visual-capture planner includes every maintained Fixture Scenario at
      every representative viewport and does not reserve typography or
      diagnostics coverage for a privileged resolution.
- [ ] Capture-plan listing and focused viewport selection continue to work with
      the renamed visual workflow.
- [ ] Visual captures remain optional human-review artifacts and are neither
      committed nor compared as pixel-golden CI inputs.
- [ ] Portrait displays and landscape viewports smaller than 1280x720 are not
      presented as supported by this work.
- [ ] Relevant layout, presentation, capture-planning, and formatting checks
      pass.
- [ ] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.
