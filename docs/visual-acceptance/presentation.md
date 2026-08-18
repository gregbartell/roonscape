# Presentation visual acceptance

Use this workflow to inspect the native GTK 4/Pango presentation across the
maintained Fixture Scenarios and representative landscape viewports. Captures
are disposable human-review artifacts, not pixel-golden test inputs. Renderer,
font, and host differences must be judged against the current
[presentation design](../design/presentation.md), not treated as an automated
screenshot-difference failure.

## Generate captures

Install the source prerequisites described in the [Development
guide](../development.md). The capture host also needs `Xvfb`, `xwininfo`, and
`scrot`. No browser engine is involved.

From the repository root, run:

```sh
npm run capture:presentations
```

The command builds the bridge, starts the production native renderer against
each shared fixture, waits for an exact-size RoonScape window, verifies the PNG
dimensions, and writes a `manifest.json`. It prints the new output directory
under `/tmp/codex/roonscape/`. Stable descriptive filenames identify the
viewport and scenario, while a new directory prevents later runs from
overwriting earlier evidence.

The capture process supplies a temporary Display Configuration with a one-hour
inactivity grace period, so OLED-safe dimming and repositioning do not alter
the evidence.

Inspect the complete plan without launching GTK:

```sh
npm run capture:presentations -- --list
```

For focused review, filter by one scenario, one viewport, or both. An explicit
output directory must be absent or empty:

```sh
npm run capture:presentations -- --only playing
npm run capture:presentations -- --only identity-baselines
npm run capture:presentations -- --viewport 1600x900
npm run capture:presentations -- --only playing --viewport 1600x900
npm run capture:presentations -- --output /tmp/codex/roonscape/presentation-review
```

## Complete Fixture Scenario matrix

The full matrix captures every maintained Fixture Scenario at each of these
peer representative viewports: 1280×720, 1600×900, 1600×1200, 1920×1200,
2560×1080, 3840×2160, and 3840×2400.

| Scenario                 | Shared fixture                | Review focus                                   |
| ------------------------ | ----------------------------- | ---------------------------------------------- |
| Playing                  | `playing.json`                | Advancing determinate progress; dark artwork   |
| Paused                   | `paused.json`                 | Frozen progress and inactivity-ready layout    |
| Starting with content    | `loading.json`                | Now Playing composition and rotating ring      |
| Starting without content | `loading-empty.json`          | `Preparing playback` on one complete line      |
| Idle                     | `stopped.json`                | One-line, unclipped `Nothing is playing`        |
| Awaiting Roon Authorization | `pairing-required.json`       | One-line heading and authorization instruction |
| Disconnected             | `disconnected.json`           | One-line heading and recovery explanation      |
| Output unavailable       | `output-unavailable.json`     | One-line heading and complete corrective copy  |
| Playing without content  | `playing-empty.json`          | Details-unavailable full field                 |
| Paused without content   | `paused-empty.json`           | Details-unavailable full field                 |
| Missing metadata         | `missing-metadata.json`       | Title-only hierarchy                           |
| Missing Artist           | `missing-artist.json`         | Absent optional Artist spacing                 |
| Missing Album            | `missing-album.json`          | Absent optional Album spacing                  |
| Missing artwork          | `missing-artwork.json`        | Fixed no-art palette and square field          |
| Long metadata            | `long-metadata.json`          | Responsive wrapping and reduction              |
| Extreme metadata         | `extreme-metadata.json`       | Final line bounds and ellipsis                 |
| Indeterminate progress   | `indeterminate-progress.json` | Artwork, activity waveform, and timing copy    |
| Non-square artwork       | `non-square-artwork.json`     | Image-shaped frame in a reserved square        |
| Light artwork            | `light-artwork.json`          | Readable light artwork-derived palette         |

Treat all seven viewports as peers. Together they exercise the minimum
supported landscape size, the 1600×900 windowed Fixture Mode presentation,
4:3, 16:10, ultrawide, 4K, and 3840×2400 fullscreen presentations without
making one size the visual authority.

## Typography, identity, and diagnostics representatives

At every representative viewport, the plan adds two typography captures, one
identity capture, and three diagnostics captures beyond the Fixture Scenario
matrix:

- **Preferred typography** requests Sitka Display for Now Playing Title. The
  capture fails clearly when the host family is unavailable; supporting roles
  continue to use packaged IBM Plex Sans.
- **Fallback typography** forces packaged Libre Baskerville for Now Playing
  Title while supporting roles continue to use packaged IBM Plex Sans.
- **Identity baselines** uses long Tracked Output and Tracked Zone names to
  expose single-line baseline alignment and defensive end ellipsis.
- **Dark diagnostics**, **light diagnostics**, and **fixed-no-art
  diagnostics** exercise the overlay against each palette class.

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
  crowding, or panel-like blocks.
- **Hierarchy:** Title is bold and upright with normal tracking and dominates;
  short single-line Titles use the larger optical tier. Artist and Album are
  upright IBM Plex Sans, with Artist stronger than Album, and remain a close
  credit group beneath the tighter Title gap. Missing fields close up cleanly.
  Long and punctuation-heavy Titles balance deterministically at word
  boundaries without short final-line orphans; two-line Titles use 0.94
  leading and denser Titles use 0.98. Long or extreme values select the first
  fitting established font tier, expose up to five Title lines, three Artist
  lines, and three Album lines, then end-ellipsize cleanly without overlapping
  other content. Metadata uses no scrolling, marquee motion, or pagination and
  never shrinks below its readable font floors.
- **Artwork fit and decoration:** The artwork column reserves the same
  imaginary square for square, non-square, missing, and unusable artwork.
  Square supplied artwork fills that reservation with its existing one-pixel
  border and shadow. Non-square supplied artwork is centered and contained;
  its surrounding reservation is transparent, and its visible surface,
  one-pixel border, and shadow hug the image rectangle. Missing or unusable
  artwork preserves a quiet decorated square field without an invented icon.
- **Palette:** Dark and light artwork recolor the complete presentation. The
  fixed no-art palette remains deliberate, and text, accent, progress, and
  diagnostics roles remain readable.
- **Presentation Status:** Every condition has the correct circular symbol
  and bold uppercase label in both presentation forms. No condition has a glow
  or halo; Playing and Starting use the full artwork accent, Paused uses a
  muted artwork accent, and no status row contains secondary detail. In Now
  Playing, the row begins at its responsive imaginary-square inset. Across
  Full-field Presentations, it occupies one separate stable slot above the
  centered heading slot and aligns with the accent's top edge.
- **Progress and utility typography:** In Now Playing, Presentation Status,
  timing, activity copy, and identities are visibly stronger while remaining
  subordinate to Title. Preferred sizes follow viewport height, stay within
  the accepted 8–12% strengthening band, and fit available width at all peer
  viewports. Now Playing Presentation Status, timing, and identities are
  mildly condensed through the IBM Plex width axis where available and remain
  normal-width otherwise. Full-field status and identity sizes remain
  unchanged. Determinate progress stays a minimal non-interactive line, and
  elapsed and remaining timing remain tabular and stable as values change.
- **Indeterminate activity:** Playing without meaningful timing retains its
  supplied artwork and replaces the complete determinate timeline with seven
  rounded activity bars followed by `Audio active` and `Timing unavailable`
  on separate lines. The waveform uses the current accent, its proportions are
  visibly symmetrical in a settled frame, and the timing explanation is muted.
  The dedicated missing-artwork Fixture Scenario continues to show the quiet
  artwork fallback independently.
- **Identities:** Output and Zone share one stable bottom-right row in
  available states. The complete row ends the same inset above the imaginary
  square's bottom edge that Presentation Status uses below its top edge. Each
  identity label and name share one text baseline. A small muted dot separates
  Output and Zone without becoming an accent. Ordinary names fit comfortably,
  long names remain on one line and end-ellipsize without moving or resizing
  the row, and unavailable states expose no stale identities.
- **Full-field grammar:** Idle, Starting without content, details unavailable,
  Awaiting Roon Authorization, disconnected, and output-unavailable states
  share the accent-bar language while retaining distinct meanings. At every
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
  Available Full-field Presentations retain the same bottom-right identity
  anchor while using their independent centered status-and-copy geometry.

Static captures establish settled endpoints. To review motion, launch Fixture
Mode:

```sh
npm run fixture
```

Keep the renderer window focused and use Left and Right to move between Fixture
Scenarios. Confirm that artwork, metadata, palette, identities, and diagnostics
crossfade as one layer; progress alone updates in place; availability loss and
disconnection use the same crossfade; and no transition exposes clipped or
stale boundary states.

Confirm that only the complete Starting ring rotates, with a steady linear
revolution of about 1.8 seconds. With the platform animation preference
disabled, confirm that Starting retains a clear static ring-and-center frame.
For Indeterminate progress, confirm that the activity bars alternate through
staggered ease-in-out phases over about 1.1 seconds and scale toward 28% without
collapsing. With animation disabled, confirm that the reference-height waveform
remains meaningful and both activity lines remain present.

Do not commit captures as goldens or add screenshot comparisons to CI.
Automated checks belong at the shared fixture, layout, typography,
palette-contrast, transition, and preserved-behavior seams; the PNGs remain
temporary evidence for human visual judgment.
