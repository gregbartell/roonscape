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

Open every requested image and record reasons against the [review checklist](#review-checklist)
before recording a verdict. Record uncertain clipping, hierarchy, palette, or
typography judgments as unresolved and use `needs-work` or `unreviewed` until
resolved. Write a JSON file
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

Settled screenshots cannot establish motion quality, distance readability,
physical display brightness/color, OLED behavior over time, or personal aesthetic
preference. Request human review for those judgments and retain them explicitly
as unresolved when they are necessary for acceptance. Actual Live Capture
Sessions require Roon and human-caused events.

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

Beyond the Fixture Scenario matrix, the plan includes these targeted captures
at every representative viewport:

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

Use the linked design sections as the requirements for each item; this checklist
identifies useful comparisons. Record **pass**, **needs work**, or **not
applicable**, plus a short reason for each item across every representative
viewport. Include all Full-field Fixture Scenarios as regression evidence when
reviewing Now Playing changes.

| Inspect | Compare against the design |
| --- | --- |
| Composition and responsive bounds | Compare ordinary and ultrawide captures for rail alignment, negative space, and clipping of artwork, metadata, shadows, and diagnostics. Check [composition and hierarchy](../design/presentation.md#composition-and-hierarchy) and the [inactivity movement envelope](../design/presentation.md#motion-and-inactivity). |
| Metadata hierarchy and fitting | Compare short, long, extreme, punctuation-heavy, and missing-field metadata using [composition and hierarchy](../design/presentation.md#composition-and-hierarchy) and [typography](../design/presentation.md#typography). Look for orphan lines, crowding, incorrect font tiers, and ellipsis. |
| Synchronized lyrics | Compare cue lengths, Intentional Blanks, missing artwork, and long mastheads against the [Synchronized Lyric Composition](../design/presentation.md#synchronized-lyric-composition). |
| Artwork fit and decoration | Compare square, non-square, missing, and unusable artwork against [artwork and palette](../design/presentation.md#artwork-and-palette). Inspect visible image bounds, border, shadow, plate alignment, and transparent reservation space without movement of the information rail. |
| Palette and determinate progress | Compare dark, light, fixed-no-art, matte, and progress representatives against [artwork and palette](../design/presentation.md#artwork-and-palette). Check text readability, fill/track separation, bright-field restraint, and retained artwork hue. |
| Presentation Status | Compare Playing, Paused, Starting, and Full-field symbols, emphasis, and anchors against [Presentation Status](../design/presentation.md#presentation-status). |
| Footer and indeterminate activity | Compare determinate progress with indeterminate activity against [composition and hierarchy](../design/presentation.md#composition-and-hierarchy) and [typography](../design/presentation.md#typography). Check rail width, timing stability, waveform symmetry, and legibility. |
| Identities | Compare ordinary, long-name, output-only, and absent identities against [composition and hierarchy](../design/presentation.md#composition-and-hierarchy) and [Full-field states](../design/presentation.md#full-field-states). Inspect baselines, separator position, independent ellipsis, and stable footer geometry. |
| Full-field grammar | Compare every condition with the copy and geometry in [Full-field states](../design/presentation.md#full-field-states). Check complete lines, stable slots, and independent identities; inspect the lower edge of `Nothing is playing` for clipping. Renew visual fit review whenever approved copy changes. |
| Diagnostics | Compare the three overlay representatives for legibility, containment, and non-displacement. Ordinary matrix captures must remain overlay-free. |

## Motion inspection

Static captures establish settled endpoints. Launch dynamic Fixture Mode:

```sh
npm run fixture
```

Keep the renderer focused and use Left and Right to visit the maintained catalog
in order. Compare behavior with [motion and inactivity](../design/presentation.md#motion-and-inactivity):

- Exercise playback-only updates, simultaneous playback/content changes,
  availability loss, disconnection, and rapid revisions. Look for stale content,
  mismatched artwork and metadata, or clipped transition boundaries. Toggle
  diagnostics to check that it follows the presentation without displacing it.
- Observe Starting and indeterminate activity with platform animations enabled
  and disabled. Check their movement and static reference states against the
  design's timing and geometry.
- Shorten the configured inactivity grace period and reposition cadence for
  inspection. Check every movement position for Paused, Idle, and unavailable
  presentations, including shadow and diagnostics containment. Confirm that
  Playing and Starting retain their normal appearance. Keep production defaults
  unchanged.

For lyric entry, Natural Cue Handoffs, Intentional Blanks, seeks, and interrupted
motion, use the [lyric motion captures](../development.md#lyric-motion-captures)
against the [Synchronized Lyric Composition](../design/presentation.md#synchronized-lyric-composition).

Keep generated captures as temporary human-review evidence. Automated checks
belong at the shared fixture, layout, typography, palette-contrast, transition,
and preserved-behavior seams; do not commit PNG goldens or add screenshot
comparisons to CI.
