# Presentation visual acceptance

Use this workflow to inspect the native GTK 4/Pango presentation across the
maintained Fixture Scenarios and representative landscape viewports. Captures
are disposable human-review artifacts, not pixel-golden test inputs. Renderer,
font, and host differences must be judged against the current
[presentation design](../design/presentation.md), not treated as an automated
screenshot-difference failure.

## Generate captures

Prepare the worktree using the
[development preparation instructions](../development.md#prepare-an-existing-worktree).
The capture host also needs `Xvfb`, `xwininfo`, and `scrot`. No browser engine is
involved.

Start with required automation, then extend the printed retained review directory:

```sh
npm run verify -- --design
npm run review:presentations -- --review /var/tmp/codex/roonscape/review.EXAMPLE --scope focused --scenario paused --scenario playing --rationale "Playback status changes affect Playing and Paused compositions."
```

Focused presentation changes select relevant maintained Fixture Scenarios and
capture each at **every maintained representative viewport**. Explain why the
selected scenarios cover the change, including affected neighboring conditions.
Shared typography, layout, or palette changes **require the complete profile**:

```sh
npm run review:presentations -- --review /var/tmp/codex/roonscape/review.EXAMPLE --scope complete --rationale "Shared layout affects every composition and typography path."
```

The complete profile includes the maintained matrix, typography, palette,
identity, and diagnostics representatives. It fails clearly if required licensed
host fonts or glyph fallback are unavailable. Do not provision or redistribute
proprietary fonts in CI. Scenario and viewport lists come from the fixture
catalog and native capture plan; inspect the current scopes with:

```sh
npm run review:presentations -- --list
```

Each invocation creates a unique `presentation.*` directory inside the retained
review directory. Open its `index.html` for linked images and coverage, and
`captures.json` for requested/completed accounting and source identity. Progress
and failures remain in `capture.log`. Captures use the existing native renderer,
exact-revision layout/paint readiness, real window checks, PNG dimension
validation, and progressive publication. Failure or cancellation retains completed
images and diagnostics, marks the set incomplete, and removes owned runtime
resources. Concurrent invocations cannot overwrite each other's evidence.

Inspect every requested image before recording a verdict. Write a JSON file
containing `verdict` (`accepted`, `needs-work`, or `unreviewed`), `reasons` (text),
`inspected` (an array of image filenames), and `unresolved` (an array of judgments).
Then attach it to that capture set:

```sh
npm run review:presentations -- --record /var/tmp/codex/roonscape/review.EXAMPLE/presentation.EXAMPLE --verdict-file /path/to/verdict.json
```

Verdict records are immutable, retained separately, and linked from the image
index. Generate a new capture review to revise a recorded verdict. An
accepted verdict requires a complete capture set, every image inspected, and no
unresolved judgments. Focused acceptance applies only to the selected scope;
only a complete profile can claim complete-profile visual acceptance. Automated
success, capture completion, and visual acceptance are independent outcomes.
CI never supplies an aesthetic verdict.

CI runs `npm run verify -- --presentation-ci`, which runs repository checks,
the design suite, and the maintained small `ci-fallback` capture scope through
this same review workflow. It forces packaged fonts for both Now Playing and
Full-field Presentations. Its artifacts include the review index, coverage,
images, and logs even on failure. This scope is representative fallback-font
evidence, not complete typography evidence or local visual acceptance. The
complete workstation profile remains required for shared presentation changes.

The lower-level `capture:presentations` command remains available for standalone
captures, including `--profile visual-acceptance`; those standalone outputs do
not include the review accounting or verdict workflow.

## Maintained Fixture Scenario matrix

The canonical sources are
[`fixture-scenario-catalog.json`](../../src/shared/fixtures/fixture-scenario-catalog.json)
and [`presentation-captures.mjs`](../../scripts/presentation-captures.mjs).
The review workflow derives its scenarios, seven peer viewports, and complete
profile from those sources rather than maintaining another documentation list.
Treat all seven viewports as peers; no single size is the visual authority.

## Typography, palette, identity, and diagnostics representatives

At every representative viewport, the plan adds two typography captures, one
identity capture, three diagnostics captures, and two focused palette captures
beyond the Fixture Scenario matrix:

- **Preferred typography** requests Sitka Display for Now Playing Title. The
  capture fails clearly when the host family is unavailable; supporting roles
  continue to use packaged IBM Plex Sans.
- **Fallback typography** forces packaged Libre Baskerville for Now Playing
  Title while supporting roles continue to use packaged IBM Plex Sans.
- **Identity baselines** uses long Tracked Output and Tracked Zone names to
  expose single-line baseline alignment and defensive end ellipsis.
- **Progress early**, **progress middle**, and **progress near complete** keep
  the played fraction visibly below 20%, between 40–80%, and above 90% while
  retaining a visible remaining track. Together they expose the determinate
  rail's direct contrast, redundant weight, and square transition encoding.
- **Dark diagnostics**, **light diagnostics**, and **fixed-no-art
  diagnostics** exercise the overlay against each palette class.
- **Light matte restraint** uses a synthetic light palette to expose the
  luminance ceiling without depending on a host image decoder or network.
- **Dark matte ownership** uses a synthetic dark palette to confirm that
  light-palette restraint does not flatten dark artwork or its teal accent.

Both typography representatives append `月` to Album so Pango glyph fallback
is visible. Confirm that only Title changes between the two paths, every
supporting role remains in IBM Plex Sans, and the extra character is readable
without a missing-glyph box. Confirm that the diagnostics overlay is quiet,
legible, inside the presentation field, and does not displace content. Normal
matrix captures must not contain the overlay.

## Review checklist

Record **pass**, **needs work**, or **not applicable**, plus a short reason for
each item. Compare the relevant captures across every representative viewport
instead of approving a scenario at only one size.

- **Composition and negative space:** The asymmetric artwork field and
  metadata column use the complete landscape field without letterboxing,
  crowding, or panel-like blocks. Every information role uses one strict left
  rail without gutter, print-plate, or artwork overhang. On ultrawide displays,
  only Title, Artist, and Album use the approximately 72%-of-viewport-height
  musical measure; status, progress or activity, timing, and identities retain
  the complete utility width.
- **Hierarchy:** Title is bold and upright with normal tracking and dominates;
  short single-line Titles use the same calm preferred tier as other fitting
  Titles. Artist and Album are upright IBM Plex Sans, with Artist stronger than
  Album, and remain a close credit group beneath the calibrated Title gap.
  Missing fields close up cleanly. Long and punctuation-heavy Titles balance
  deterministically at word boundaries without short final-line orphans. Long
  or extreme values select the first tier that fits the complete metadata
  group's width and height, use compact-credit density only when necessary,
  expose up to five Title lines, three Artist lines, and three Album lines, then
  end-ellipsize cleanly without overlapping status or footer. The complete
  metadata group stays vertically centered with its small optical correction.
  Metadata uses no scrolling, marquee motion, or pagination and never shrinks
  below its readable font floors.
- **Artwork fit and decoration:** The artwork column reserves the same
  imaginary square for square, non-square, missing, and unusable artwork. Its
  side is the lesser of 84% of viewport height and 56% of viewport width, and
  the result remains vertically centered. Square supplied artwork fills that
  reservation with its responsive one-to-two-pixel border and quiet
  artwork-surface shadow.
  Non-square supplied artwork is centered and contained; its surrounding
  reservation is transparent, and its visible surface, border, shadow, and
  matching accent plate hug the image rectangle. The plate is centered on the
  same visible bounds before its responsive right/down offset is applied; at
  3840×2160 that offset is approximately 24/16 px. Missing or unusable artwork
  preserves a quiet decorated square field with a square plate and no invented
  icon. The fully opaque plate stays flat, crisp, square-cornered, and free of
  shadow, blur, texture, rotation, grain, or registration marks, while the
  shadow remains on the artwork surface rather than the combined stack.
- **Palette:** Dark and light artwork recolor the complete presentation. The
  gradient geometry remains stable, the fixed no-art palette stays deliberate,
  and text, accent, progress, and diagnostics roles remain readable. The
  determinate fill is immediately distinct from the track in dark, light, and
  low-chroma palettes; the complete track remains visible against the metadata
  field while subordinate to the full artwork-derived accent fill. The
  light-matte representative demonstrates restrained bright-end luminance
  without losing blue and plum chroma; the dark-matte representative
  preserves its dark field and teal ownership. Accent is limited to the print
  plate, active Presentation Status, determinate progress fill, and
  indeterminate activity.
- **Presentation Status:** In Now Playing, every condition has the correct
  circle-free symbol in one fixed cell and a bold uppercase label beginning at
  one fixed horizontal position. It has no border, circle, glow, or halo;
  Playing and Starting use the full artwork accent, Paused uses a muted artwork
  accent, and no status row contains secondary detail. The row begins at its
  responsive imaginary-square inset. Full-field Presentations use the
  circular symbol and stable slot above the centered heading, aligned with the
  accent's top edge.
- **Unified footer and utility typography:** In Now Playing, progress or
  activity occupies the complete rail width and the identity row follows as
  part of the same low, optically raised footer. Presentation Status, timing,
  activity copy, and identities remain subordinate to Title but readable at
  distance. Their preferred sizes follow viewport height and preserve their
  floors at all peer viewports. Now Playing Presentation Status, timing, and
  identities are mildly condensed through the IBM Plex width axis where
  available and remain normal-width otherwise. Full-field status and identity
  sizes use their dedicated layout. Determinate progress uses a heavier played
  segment over a centered, lighter remaining track, with square ends and a
  square transition at the current position. It has no thumb, endpoint marker,
  or other interactive affordance. Elapsed and remaining timing remain tabular
  and stable as values change.
- **Indeterminate activity:** Playing without meaningful timing retains its
  supplied artwork and replaces the complete determinate timeline with seven
  rounded activity bars followed by `Audio active` and `Timing unavailable`
  on separate lines. The waveform uses the current accent, its proportions are
  visibly symmetrical in a settled frame, and the timing explanation is muted.
  The dedicated missing-artwork Fixture Scenario continues to show the quiet
  artwork fallback independently.
- **Identities:** Output and Zone share one stable row in the Now Playing
  footer. Each semibold, slightly tracked uppercase label shares one baseline
  with its more prominent name, each phrase receives at most its bounded share,
  and a muted dot scaling to approximately 10 px at 3840×2160 stays centered
  between them without participating in their baseline. Ordinary names fit
  comfortably; long names remain on one line and end-ellipsize independently
  without moving the separator or resizing the footer. Available Full-field
  Presentations use their bottom-right row. Output unavailable uses that same
  anchor for only the persisted Tracked Output, with no separator or empty Zone
  phrase; the row is absent for a legacy configuration with no saved name.
  Awaiting Roon Authorization and disconnected expose neither identity.
- **Full-field grammar:** Idle, Starting without content, details unavailable,
  Awaiting Roon Authorization, disconnected, and output-unavailable states
  share the accent-bar language while retaining distinct meanings. Output
  unavailable keeps its output-only identity independent of this centered
  composition. At every
  peer viewport, confirm that the bar-and-copy composition occupies 60% of the
  layout viewport and is horizontally centered; the bar remains its stable
  left edge; and Presentation Status, heading, and explanation share the same
  left-aligned text edge after the responsive inset. The fixed heading slot is
  vertically centered and all scenarios share its position. Presentation
  Status and the bar top do not move. Explanations occupy the fixed slot below
  the heading and extend only the bar bottom. Confirm every approved heading
  and explanation is complete on one line without ellipsis, shorter copy keeps
  its preferred size, only over-capacity copy shrinks to its largest fitting
  size, and `Nothing is playing` is not clipped at its lower edge. Any approved
  Full-field copy change requires renewed visual fit review rather than relying
  on a permanent minimum font floor.
- **Diagnostics:** The three overlay representatives remain legible,
  non-displacing, and inside the OLED-safe field; ordinary captures remain
  overlay-free.
- **Responsive bounds:** Artwork, metadata, full-field copy, identities,
  diagnostics, shadows, and the reserved OLED movement envelope stay inside
  every viewport. In Now Playing, status and identity anchors remain tied to
  the imaginary square for square, non-square, missing, and unusable artwork.
  Available Full-field Presentations use the same bottom-right identity
  anchor while using their independent centered status-and-copy geometry.

Static captures establish settled endpoints. To review motion, launch Fixture
Mode:

```sh
npm run fixture
```

Keep the renderer window focused and use Left and Right to move between Fixture
Scenarios. Confirm that artwork, metadata, palette, identities, and diagnostics
crossfade as one layer when the composition changes; playback-only changes to
the same composition update Presentation Status and progress immediately in
place; simultaneous playback and Now Playing changes take the composition
crossfade; a Playing Track A to Paused Track B update never exposes Track A's
artwork with Track B's metadata; availability loss and disconnection use the
same crossfade; rapid revisions settle on the newest presentation; Fixture
navigation visits the maintained catalog in order; and no transition exposes
clipped or stale boundary states. Toggle diagnostics while navigating and
confirm that the overlay remains non-displacing.

Confirm that only the complete Starting ring rotates, with a steady linear
revolution of about 1.8 seconds. With the platform animation preference
disabled, confirm that Starting retains a clear static ring-and-center frame.
For Indeterminate progress, confirm that the activity bars alternate through
staggered ease-in-out phases over about 1.1 seconds and scale toward 28% without
collapsing. With animation disabled, confirm that the reference-height waveform
remains meaningful and both activity lines remain present.

Exercise inactivity with the configured grace period and reposition cadence
shortened for review. Confirm that eligible Paused, Idle, and unavailable
presentations dim and follow the bounded movement sequence without changing
the established production timing defaults, while Playing and Starting remain
at full opacity and their normal position. At every movement position, every
Now Playing and Full-field element, including shadow and diagnostics, must
remain inside the reserved safe field.

Review every Full-field Fixture Scenario at all seven peer viewports as
regression evidence. Its approved copy, circular status treatment,
typography, accent-bar geometry, and identity-presence rules must not change
with the Now Playing composition.

Do not commit generated captures as goldens or add screenshot comparisons to
CI.
Automated checks belong at the shared fixture, layout, typography,
palette-contrast, transition, and preserved-behavior seams; the PNGs remain
temporary evidence for human visual judgment.

## Agent and human inspection responsibilities

Agents must open the generated images, compare relevant scenarios across all
requested viewports, and record concrete reasons against the checklist above.
Do not infer visual acceptance from passing tests or successful PNG publication.
Record uncertain clipping, hierarchy, palette, or typography judgments as
unresolved and use `needs-work` or `unreviewed` until resolved. A focused selection
must explain omissions; use the complete profile when effects are shared.

Settled screenshots cannot establish motion quality, distance readability,
physical display brightness/color, OLED behavior over time, or personal aesthetic
preference. Request human review for those judgments and retain them explicitly
as unresolved when they are necessary for acceptance. Actual Live Capture
Sessions remain a separate workflow requiring Roon and human-caused events.
