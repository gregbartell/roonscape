# Gallery split visual acceptance

Use this workflow to inspect the native GTK 4/Pango renderer at the Reference
Deployment and development dimensions. Captures are review artifacts, not
pixel-golden test inputs: renderer, font, and host differences must be judged
against the selected prototype and specification rather than treated as an
automated screenshot-difference gate.

## Generate captures

Install the project toolchains and GTK development libraries described in the
repository README. The capture host also needs `Xvfb`, `xwininfo`, and `scrot`.
No browser engine is used or added to the runtime.

From the repository root, run:

```sh
npm run capture:gallery
```

The command prints a new output directory under `/tmp/codex/roonscape/`. It
starts the production native renderer against each shared fixture, waits for
the exact-size RoonScape window, captures that window, verifies the PNG
dimensions, and writes `manifest.json`. Files have stable, descriptive names;
the new directory prevents a repeat run from overwriting earlier evidence.
The command supplies its own Display Configuration with a one-hour inactivity
grace, so a host's OLED calibration cannot dim or reposition capture evidence.

Inspect the plan without launching GTK:

```sh
npm run capture:gallery -- --list
```

During iteration, select one scenario and viewport or choose an empty output
directory explicitly:

```sh
npm run capture:gallery -- --only playing --viewport 1600x900
npm run capture:gallery -- --output /tmp/codex/roonscape/gallery-review
```

The full run captures every matrix scenario at 3840×2160, 3840×2400, and
1600×900:

| Scenario                | Shared fixture                | Decision coverage                             |
| ----------------------- | ----------------------------- | --------------------------------------------- |
| Playing                 | `playing.json`                | Determinate, advancing progress; dark artwork |
| Paused                  | `paused.json`                 | Frozen determinate progress                   |
| Loading with content    | `loading.json`                | Gallery split retained                        |
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

At 3840×2160, five additional representatives cover the host-provided
preferred pair, the forced packaged fallback pair, and diagnostics over dark,
light, and fixed no-art palettes. A preferred-pair capture fails clearly if
Palatino Linotype and Segoe UI are not both installed; it never substitutes
other faces while claiming to show the preferred pair. Both typography
representatives append `月` to Album so readable Pango glyph fallback is
visible with each complete pair.

## Review references

Review each capture alongside:

- the selected Gallery split direction and corrections in
  [`docs/design/gallery-split.md`](../design/gallery-split.md);
- Variant A (preferred) and Variant D (fallback) in the throwaway
  [`prototype/gallery-split-font-study/`](../../prototype/gallery-split-font-study/README.md);
- the complete behavior and testing decisions in
  [the Gallery split restyle specification](../../.scratch/gallery-split-restyle/spec.md).

Browser chrome and the prototype control bar are excluded. Native GTK/Pango
output should be a close visual match in character and composition, not pixel
identity with the browser prototype.

## Decision checklist

Record **pass**, **needs work**, or **not applicable**, plus a short reason for
each item. Review all three dimensions unless an item names a representative.

- **Composition:** The asymmetric album-sleeve field and dedicated metadata
  column retain the prototype's proportions without letterboxing at 16:9 or
  16:10.
- **Negative space:** Artwork breathes inside the left field, metadata is not
  crowded, and neither side reads as an opaque competing panel.
- **Hierarchy:** Title dominates; Artist, Album, status, progress, and utility
  text step down deliberately. Missing fields do not leave broken gaps. Long
  and extreme values remain calm within their three/two/two line bounds. The
  typography representatives render `月` readably without a missing-glyph box.
- **Artwork fit:** Square artwork is complete; non-square artwork is centered
  and contained; missing artwork preserves the Gallery split without an
  invented icon or label.
- **Palette:** Dark and light artwork recolor the complete presentation. The
  no-art navy/coral/cream field remains deliberate. Primary, secondary,
  accent, progress, and diagnostic roles remain visibly readable.
- **Footer position:** Output and Zone share one stable bottom-right row in
  available states, ordinary names fit comfortably, and unavailable states do
  not expose stale identities.
- **Full-field grammar:** Idle, empty Loading, details unavailable, pairing,
  disconnected, and output-unavailable captures share the vertical accent-bar
  language while retaining their distinct meanings and restrained copy.
- **Transition boundaries:** Compare the settled captures on both sides of a
  fixture change, then watch a native fixture startup to confirm artwork,
  metadata, palette, footer, and diagnostics crossfade as one layer with no
  clipped or stale boundary state. Static captures establish endpoints; they
  do not automate motion quality.
- **Diagnostics:** The three diagnostics representatives remain quiet,
  legible, inside the OLED-safe field, and non-displacing over dark, light, and
  fixed no-art palettes. Normal matrix captures contain no overlay.

Do not commit captures as goldens or add screenshot differences to CI. Keep
automated coverage at the shared fixture, layout-policy, palette-contrast, and
preserved-behavior seams.

## Physical Reference Deployment handoff

These captures do not establish viewing-distance legibility, OLED luminance,
crossfade quality on the television, inactivity dimness, movement cadence, or
burn-in protection. Complete those judgments on the physical 3840×2160 OLED
under [Tune and accept the Reference Deployment](../../.scratch/roonscape/issues/10-tune-accept-reference-deployment.md).
That human ticket remains the authority for display-mode, physical transition,
resource, television-off, and unattended-boot acceptance.
