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

Run the applicable checks from [verification policy](../agents/verification.md#choose-checks).
For visual assessment, select useful Fixture Scenarios and viewports with
`capture:presentations`, or use a maintained capture scope:

```sh
npm run review:presentations -- --output /var/tmp/codex/roonscape/task.EXAMPLE --scope focused --scenario paused --scenario playing
npm run review:presentations -- --output /var/tmp/codex/roonscape/task.EXAMPLE --scope complete
npm run review:presentations -- --list
```

The focused scope captures selected scenarios at all maintained viewports. The
complete scope includes the maintained matrix, typography, palette, identity,
and diagnostics representatives; it requires the licensed host fonts and glyph
fallback. The smaller `ci-fallback` scope uses packaged fonts.

Each invocation writes PNGs, capture progress in `captures.json`, and diagnostics
in `capture.log` to a unique `presentation.*` directory beneath `--output`.
Captures are independent of verification runs. Failure or cancellation leaves
completed images and removes owned runtime resources.

Settled screenshots cannot establish motion quality, distance readability,
physical display brightness/color, OLED behavior over time, or personal aesthetic
preference. Human review is needed when those judgments matter to the change.
Actual Live Capture Sessions require Roon and human-caused events.

## Maintained Fixture Scenario matrix

The canonical sources are
[`fixture-scenario-catalog.json`](../../src/shared/fixtures/fixture-scenario-catalog.json)
and [`presentation-captures.mjs`](../../scripts/presentation-captures.mjs).
Capture scopes derive their scenarios, seven peer viewports, and complete
profile from those sources.
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

Use the linked design sections as requirements; this checklist identifies useful
comparisons for affected behavior.

| Inspect | Compare against the design |
| --- | --- |
| Composition and responsive bounds | Compare ordinary and ultrawide captures for rail alignment, negative space, and clipping of artwork, metadata, shadows, and diagnostics. Check [composition and hierarchy](../design/presentation.md#composition-and-hierarchy) and the [inactivity movement envelope](../design/presentation.md#motion-and-inactivity). |
| Metadata hierarchy and fitting | Compare short, long, extreme, punctuation-heavy, and missing-field metadata using [composition and hierarchy](../design/presentation.md#composition-and-hierarchy) and [typography](../design/presentation.md#typography). Look for orphan lines, crowding, incorrect font tiers, and ellipsis. |
| Synchronized lyrics | Compare Lyric Reel capacity and repeated cues, cue lengths, Intentional Blanks, missing artwork, and long mastheads against the [Synchronized Lyric Composition](../design/presentation.md#synchronized-lyric-composition). |
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

Use Left and Right to visit relevant Fixture Scenarios with the renderer focused.
Compare behavior with [motion and inactivity](../design/presentation.md#motion-and-inactivity):

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

For settled Lyric Reel context, include `lyrics-reel-capacity` at all seven peer
viewports. Check the first-line Primary Position, modest inter-cue gaps, and
partially visible cues at both faded edges. Include `lyrics-four-lines` for
complete oversized text at every viewport: its final line must clear the bottom
fade, its first line must retain the Primary Position, and earlier context must
pack above it. Compare with `lyrics-one-line` for unchanged normal focal size.
Inspect `wrapping-progression` motion captures in both directions around the
oversized cue, including reduced animation, for stable wrapping and continuous
upward departure.

Include `lyrics-blank-cue` at all seven peer viewports. Its consecutive blanks
follow a multiline cue: confirm an empty Primary Position with packed earlier
and upcoming context, quiet emphasis, and partial cues at the faded bounds.
Inspect `blank-lifecycle`, `short-blanks`, and `timeline-edge-cases` motion
captures for stable consecutive-blank holds, destination-relative context on
seeks, and continuous promotion when lyrics return during an unfinished
departure. Compare reduced animation for the same complete destinations.

For lyric entry, Natural Cue Handoffs, Intentional Blanks, seeks, and interrupted
motion, use the [lyric motion captures](../development.md#lyric-motion-captures)
against the [Synchronized Lyric Composition](../design/presentation.md#synchronized-lyric-composition).

Use the `artwork-updates` motion example to inspect fallback arrival, repeated
artwork changes, removal/failure, and artwork updates during a Now Playing
Transition. Compare intermediate frames for continuous artwork/background
blending, readable metadata, and uninterrupted timing and Lyric Reel motion.
The reduced-animation version should show immediate artwork destinations while
preserving the same content and fallback behavior.

Automated checks belong at the shared fixture, layout, typography,
palette-contrast, transition, and preserved-behavior seams; do not commit PNG goldens or add screenshot
comparisons to CI.
