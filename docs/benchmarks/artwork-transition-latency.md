# Artwork transition latency

Measured on 2026-08-29 at the display's native 3840×2400 resolution. The
benchmark alternates between the dark-palette **Playing** Fixture Scenario and
the **Light artwork** Fixture Scenario.

The optimization target is preparation: the interval from GTK receiving the
navigation keypress until GTK reports that the crossfade is running. The
configured 450 ms crossfade is intentionally excluded and remains unchanged.

## Fresh album result

This is the decision benchmark. Before every measured transition, temporary
benchmark instrumentation emptied the cross-presentation gradient and artwork
caches. Reuse within one presentation render remained enabled, matching a real
new track instead of decoding the same new artwork twice.

Both runs used the optimized release renderer. Each result is the mean of 10
fresh transitions in each direction after a warm-up transition.

| Direction | Serial fresh preparation | Concurrent fresh preparation | Reduction |
| --- | ---: | ---: | ---: |
| Dark → light | 128.7 ms | 76.3 ms | 40.7% |
| Light → dark | 202.1 ms | 171.3 ms | 15.2% |
| Combined | 165.4 ms | 123.8 ms | 25.2% |

The optimized renderer computes a cache-missing gradient while GTK constructs
and lays out the new artwork and metadata. It then installs the completed
gradient before starting the crossfade. Two gradient workers gave the best
shared-work balance on the eight-CPU benchmark host: one left gradient work on
the critical path, while four competed with artwork decoding and scaling.

The gradient algorithm and its output are unchanged. A representative raster
hash regression test protects exact output bytes, and the existing visual and
quantization tests continue to protect the gradient's appearance.

## Why the earlier warm result is not the headline

The first optimized benchmark alternated two already-rendered Fixture
Scenarios. Its 15.8 ms combined preparation mean was a valid warm-cache result,
but it did not represent ordinary listening, where the next track commonly
brings unseen album artwork and a new palette. The caches remain useful for
revisiting an album, but the fresh benchmark above is the listening-path source
of truth.

## Diagnostic findings

The original stage-instrumented development build averaged 432.5 ms of fresh
preparation. Full-resolution gradient generation accounted for about 279 ms,
artwork decode and widget construction for about 59 ms, artwork scaling for
about 50 ms, and waiting for the renderer's polling tick for about 27 ms.

Two independent effects explained most of the apparent improvement during
investigation:

- Release optimization reduces isolated 3840×2400 gradient generation from
  roughly 300 ms in the development renderer to well under 100 ms on this
  host. Live Mode already uses the release renderer, so development-build
  timings must not be treated as listening-path timings.
- The local snapshot wakeup removes the average 27 ms polling delay. It helps
  both fresh and cached transitions.

The remaining fresh work is asymmetric because the two SVG fixtures have
different decode costs. Concurrent gradient preparation reduces both
directions without weakening image quality or changing transition duration.

## Retained cache behavior

- `PresentationView` owns a two-entry least-recently-used gradient raster cache
  keyed by the complete palette, logical viewport, physical viewport, and
  scale factor. At 3840×2400, two retained RGBA8 buffers use about 74 MB before
  toolkit overhead.
- The view also owns a two-entry least-recently-used artwork cache. Its key
  includes the repository-resolved path and artwork revision, and a scaled
  variant is reused only at identical fitted dimensions.
- These caches accelerate legitimate revisits but are not counted in the fresh
  result.

## Method

- Used an isolated 3840×2400 Xvfb display with software rendering and confirmed
  that the fullscreen renderer window was exactly 3840×2400.
- Ran the release renderer used to represent Live Mode performance.
- Used one Linux monotonic clock for the renderer key handler and GTK's
  `transition-running` notification.
- Forced a cross-presentation cache miss before every measured transition,
  while preserving normal reuse during that transition.
- Alternated 10 Right and 10 Left keypresses with enough settling time to avoid
  overlapping transitions.
- Ran a serial fresh-path control and the concurrent implementation under the
  same harness.
- Removed all cache-forcing switches and tagged timing instrumentation after
  the run.

Because Xvfb used software rendering, the values characterize this isolated
benchmark environment rather than a GPU-backed display. The relative result is
still useful because both implementations rendered the same pixels through the
same display stack.
