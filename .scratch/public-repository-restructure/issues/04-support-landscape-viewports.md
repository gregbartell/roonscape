# 04 — Support representative landscape viewports

**What to build:** Make RoonScape's visual support independent of one personal
4K deployment by treating common landscape displays at 1280x720 or larger as a
responsive range. Exercise ordinary, tall, wide, and high-resolution examples
without naming any resolution as canonical or weakening the presentation's
existing hierarchy and OLED-safe behavior.

**Blocked by:** 03 — Adopt Now Playing vocabulary and neutral fixtures.

**Status:** done

- [x] No layout interface, capture planner, test, script message, or maintained
      source concept designates a reference or canonical viewport.
- [x] Automated layout coverage includes 1280x720, 1600x1200, 1920x1200,
      2560x1080, and 3840x2160 as peer representative viewports.
- [x] At every representative viewport, the Now Playing composition uses the
      available landscape field without unintended letterboxing or arithmetic
      overflow.
- [x] Artwork remains contained, metadata remains bounded and readable,
      playback status and progress retain their hierarchy, and Output and Zone
      identities remain stable.
- [x] Full-field states, diagnostics, transitions, and the OLED movement
      envelope remain within the available presentation field across the
      representative matrix.
- [x] The visual-capture planner includes every maintained Fixture Scenario at
      every representative viewport and does not reserve typography or
      diagnostics coverage for a privileged resolution.
- [x] Capture-plan listing and focused viewport selection continue to work with
      the renamed visual workflow.
- [x] Visual captures remain optional human-review artifacts and are neither
      committed nor compared as pixel-golden CI inputs.
- [x] Portrait displays and landscape viewports smaller than 1280x720 are not
      presented as supported by this work.
- [x] Relevant layout, presentation, capture-planning, and formatting checks
      pass.
- [x] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.

## Comments

### Implementation Result — 2026-08-16

Implemented in `f91ef06`, with the Spec review's edge-content coverage fix in
`a9b8bd3`. Renderer layout policy now treats 1280x720, 1600x1200, 1920x1200,
2560x1080, and 3840x2160 as peer landscape examples. Metadata layout requires
the actual viewport, and artwork depth remains inside the field at the minimum
supported size while the established Now Playing, full-field, identity, and
OLED-safe policies remain intact.

The visual-acceptance planner now includes every maintained Fixture Scenario,
both typography pairs, and dark, light, and missing-artwork diagnostics at all
five viewports. Plan listing and focused viewport capture remain covered, while
captures stay optional, uncommitted review artifacts.

`npm run check` passed formatting, TypeScript and Rust typechecking, linting,
all bridge, renderer, script, and packaging tests, and the IPC restart smoke
exercise. Final Standards review reported no hard violations and two
test-independence duplication judgements; final Spec review reported no
findings. The ticket commits changed no other `.scratch` content; the separate
`3f11811` tracker-cleanup commit was excluded from implementation and review.
