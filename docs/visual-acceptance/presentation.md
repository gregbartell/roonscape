# Presentation visual acceptance

Use this workflow to inspect the native GTK 4/Pango presentation across the
maintained Fixture Scenarios and representative landscape viewports. Captures
are disposable human-review artifacts, not pixel-golden test inputs. Renderer,
font, and host differences must be judged against the current
[presentation design](../design/presentation.md), not treated as an automated
screenshot-difference failure.

## Generate captures

Install the source toolchains and GTK development libraries described in the
repository README. The capture host also needs `Xvfb`, `xwininfo`, and `scrot`.
No browser engine is involved.

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
npm run capture:presentations -- --viewport 1600x1200
npm run capture:presentations -- --only playing --viewport 1600x1200
npm run capture:presentations -- --output /tmp/codex/roonscape/presentation-review
```

## Complete Fixture Scenario matrix

The full matrix captures every maintained Fixture Scenario at each of these
peer representative viewports: 1280x720, 1600x1200, 1920x1200, 2560x1080, and
3840x2160.

| Scenario                | Shared fixture                | Review focus                                  |
| ----------------------- | ----------------------------- | --------------------------------------------- |
| Playing                 | `playing.json`                | Advancing determinate progress; dark artwork  |
| Paused                  | `paused.json`                 | Frozen progress and inactivity-ready layout   |
| Loading with content    | `loading.json`                | Now Playing composition retained              |
| Loading without content | `loading-empty.json`          | Full-field Loading grammar                    |
| Idle                    | `stopped.json`                | Quiet full-field Idle grammar                 |
| Pairing required        | `pairing-required.json`       | Corrective unavailable state                  |
| Disconnected            | `disconnected.json`           | Distinct disconnected copy                    |
| Output unavailable      | `output-unavailable.json`     | Distinct Tracked Output copy                  |
| Playing without content | `playing-empty.json`          | Details-unavailable full field                |
| Missing metadata        | `missing-metadata.json`       | Title-only hierarchy                          |
| Missing Artist          | `missing-artist.json`         | Absent optional Artist spacing                |
| Missing Album           | `missing-album.json`          | Absent optional Album spacing                 |
| Missing artwork         | `missing-artwork.json`        | Fixed no-art palette and square field         |
| Long metadata           | `long-metadata.json`          | Responsive wrapping and reduction             |
| Extreme metadata        | `extreme-metadata.json`       | Final line bounds and ellipsis                |
| Indeterminate progress  | `indeterminate-progress.json` | Timeline omitted                              |
| Non-square artwork      | `non-square-artwork.json`     | Centered contain fit without cropping         |
| Light artwork           | `light-artwork.json`          | Readable light artwork-derived palette        |

Treat all five viewports as peers. Together they exercise the minimum
supported landscape size, 4:3, 16:10, ultrawide, and high-resolution
presentations without making one size the visual authority.

## Typography and diagnostics representatives

At every representative viewport, the plan adds two typography captures and
three diagnostics captures beyond the Fixture Scenario matrix:

- **Preferred typography** requests Palatino Linotype with Segoe UI. The
  capture fails clearly unless both families are available.
- **Fallback typography** forces the packaged Libre Baskerville and IBM Plex
  Sans pair.
- **Dark diagnostics**, **light diagnostics**, and **fixed-no-art
  diagnostics** exercise the overlay against each palette class.

Both typography representatives append `月` to Album so Pango glyph fallback
is visible. Confirm that each serif/sans pair changes atomically and that the
extra character is readable without a missing-glyph box. Confirm that the
diagnostics overlay is quiet, legible, inside the presentation field, and does
not displace content. Normal matrix captures must not contain the overlay.

## Review checklist

Record **pass**, **needs work**, or **not applicable**, plus a short reason for
each item. Compare the relevant captures across every representative viewport
instead of approving a scenario at only one size.

- **Composition and negative space:** The asymmetric artwork field and
  metadata column use the complete landscape field without letterboxing,
  crowding, or panel-like blocks.
- **Hierarchy:** Title dominates; Artist, Album, status, progress, and utility
  text step down deliberately. Missing fields close up cleanly, and long or
  extreme values remain readable within their line bounds.
- **Artwork fit:** Square artwork is complete, non-square artwork is centered
  and contained, and missing artwork preserves a quiet square field without an
  invented icon.
- **Palette:** Dark and light artwork recolor the complete presentation. The
  fixed no-art palette remains deliberate, and text, accent, progress, and
  diagnostics roles remain readable.
- **Identities:** Output and Zone share one stable bottom-right row in
  available states. Ordinary names fit comfortably, long names degrade
  defensively, and unavailable states expose no stale identities.
- **Full-field grammar:** Idle, Loading without content, details unavailable,
  pairing, disconnected, and output-unavailable states share the accent-bar
  language while retaining distinct meanings and restrained copy.
- **Diagnostics:** The three overlay representatives remain legible,
  non-displacing, and inside the OLED-safe field; ordinary captures remain
  overlay-free.
- **Responsive bounds:** Artwork, metadata, full-field copy, identities,
  diagnostics, shadows, and the reserved OLED movement envelope stay inside
  every viewport.

Static captures establish settled endpoints. To review motion, launch Fixture
Mode:

```sh
npm run fixture
```

Keep the renderer window focused and use Left and Right to move between Fixture
Scenarios. Confirm that artwork, metadata, palette, identities, and diagnostics
crossfade as one layer; progress alone updates in place; availability loss
clears stale Now Playing content immediately; and no transition exposes
clipped or stale boundary states.

Do not commit captures as goldens or add screenshot comparisons to CI.
Automated checks belong at the shared fixture, layout, typography,
palette-contrast, transition, and preserved-behavior seams; the PNGs remain
temporary evidence for human visual judgment.
