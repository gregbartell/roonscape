# Physical presentation acceptance

`npm run accept:presentation` is a separately selected physical-display exercise.
It runs the maintained Fixture Mode workloads without Roon Server, Roon
Authorization, or Roon Control. It borrows the selected X11 display and opens
RoonScape fullscreen there. Do not select a display that must remain available
for another viewer during the exercise.

```sh
npm run accept:presentation -- \
  --display :0 --physical-output OUTPUT_NAME --resolution 1920x1080 \
  --baseline ref:BASELINE_REVISION --candidate worktree:. \
  --workloads progress,fresh-content,superseded-content \
  --output /var/tmp/codex/roonscape/physical-review
```

Replace the display, output name, and resolution with the explicitly selected
existing configuration. Execution is never inferred from `DISPLAY`. This command
requires both `--display` and `--physical-output`; routine `compare:renderer`
runs never use a physical display. Select a stable fixed-refresh configuration
before running; the command does not change modes, power policy, or refresh
settings. No physical exercise is required when suitable facilities have not
been explicitly selected, and headless success cannot replace it.

## Supported capabilities

Use a prepared development host when selecting source revisions. A display host
can instead select `build:PATH` for both sides, using retained release builds
with their manifests and resources. That path requires neither Rust/Cargo,
Git/tar, nor `qmake6`; physical runs never require Xvfb. Node.js, Python 3,
Fontconfig, `xwininfo`, D-Bus, and the Renderer runtime libraries must be
available. The measurement host's Qt version is reported as unavailable for
retained builds, with the reason recorded explicitly. Original build toolchain
identities remain in the retained source manifests. Source builds record
`qmake6` build-tool metadata, which does not establish the loaded runtime version.

The collector needs a C++17 compiler and XCB, X Present, and X RandR development
files on the measurement host. They are included in the maintained host package
list and native preflight. The comparison builds its small probe and collector
separately and retains their source/binary digests and compiler identity.

The supported path is Linux, X11, Qt Quick OpenGL through xcb/GLX, and DRI3
PresentPixmap or PresentPixmapSynced submissions on the thread that drew the
native frame. The read-only probe requires Present, DRI3, and RandR, exactly one
active connected output, and a fullscreen viewport matching the existing mode
at unit scale. Composited X11, Xwayland, interlaced/doublescan modes, and missing
capabilities are unavailable. The selected X authority is used without reading
or modifying personal Display Configuration or Roon Authorization.

Preflight alone is insufficient. A supported run must also produce matching
scanout **flip** completions. Copy completions cannot establish physical delivery;
an unsupported driver path that never exposes Present submissions remains
unavailable. Asynchronous presentation is unsupported. The analyzer validates
increasing MSC/UST and checks their agreement with fixed refresh, allowing at
most the larger of 1 ms and 5% of one refresh interval for clock quantization.
Inconsistent timing invalidates cadence instead of interpreting variable refresh
as missed frames. DRM timestamps reference the end of vertical blanking and may
slightly follow event receipt. The analyzer permits only the vertical blank
interval derived from the selected mode, retaining the original timestamps;
see [DRM timestamp semantics](https://docs.kernel.org/gpu/drm-kms.html#c.drm_crtc_funcs).
Capabilities are checked again around each run.

## Identities and evidence

The collector intercepts the actual XCB Present request. A thread-local frame
and scene identity is available only between the native draw and its swap.
Independent X Present completion events arrive on the collector's own X
connection and join requests by **window and Present serial**. Neither a nearby
timestamp nor Qt's swap callback creates an association.

The [X Present protocol](https://cgit.freedesktop.org/xorg/proto/presentproto/tree/presentproto.txt)
defines the matching serial, completion mode, MSC (display sequence), and UST
(presentation timestamp). This is **presentation evidence, not optical
verification** of pixels. It depends on the selected server/driver honoring
those semantics; it does not measure panel scanout position or light emission.

The existing native draw and content recorders connect submitted scenes to
Presentation Snapshot revisions, content/artwork identities, and preparation
milestones without changing the Snapshot contract. Reports keep publication to
preparation, native readiness, and first associated physical presentation
latencies separate. Missing values remain null. `delayed` means first delivery
fell beyond the measurement window during bounded drain. `superseded` means a
replacement overtook content without an observed delivery; `missing`,
`ambiguous`, and `unavailable` never imply successful visibility. Already proven
first presentation is not erased by a later missing completion.

The animation analyzer retains the refresh-stride ceiling of 60 scheduled
frames per second. It reports changed and unchanged scenes, render-start
anomalies, missed eligible refreshes, worst observed delivery gaps, and stalls.
Quiet, static, and reduced-animation intervals do not require every refresh.
Boundary context is retained: a quiet scene can span a window, and an active
scene that stops updating cannot hide a stall at the end of measurement.
Missing or ambiguous in-window scene associations invalidate cadence, including
when the recording file otherwise drained normally.

## Instrumentation and limits

Clean CPU/RSS passes and instrumented physical passes use separate Renderer
processes, with identical assets and elapsed-time publication schedules.
Instrumented samples never enter clean resource summaries. Reports compare
clean and instrumented CPU/RSS for each side/workload, retaining repeat variation
and enqueue timing. These characterize overhead under the recorded conditions;
run order, load, and thermal drift remain confounders. One repeat cannot establish
variation, and microsecond enqueue timing omits the rest of instrumentation.
Inspect overhead and integrity before trusting timing differences. Resource
increases are advisory and never a presentation contract threshold.

Present request recording uses a fixed 4,096-entry queue, a maximum of 131,072
retained request/completion/subscription records, and fixed-size request entries.
The render thread does not wait for the consumer. A worker owns X11 observation
and file writes. The animation recorder queues at most 64 scene entries and
bounds its output to 128 MiB; content recording retains its existing bounds.
Overflow/loss, malformed data, missing normal-shutdown footers, and inconsistent
record counts invalidate evidence. Readers reject oversized files. Long or very
complex workloads can exhaust these bounds; shorten windows or use more repeats
instead of treating partial evidence as complete.

Deterministic tests pause the collector's output consumer, verify submission
continues within capacity, exhaust capacity, and check invalidation and draining.
They also exercise real X Present serials on a private X server and native scene
recording. Those tests validate plumbing and integrity, not physical cadence.

## Profiles, reports, and outcomes

Source/build selection, optimized builds, workload assets, profiles, repeat
ordering, publication tolerance, and `/proc` sampling follow
[Renderer resource comparison](renderer-resource-comparison.md).
Every selected workload has a clean pass and a separate physical pass for each
side/repeat. Default settings therefore schedule about thirteen minutes after
preparation/builds for all ten workloads, before startup/cleanup. Focused
selection reduces that work explicitly. The routine resource comparison retains
its own ten-minute target; physical acceptance adds no long runs to CI.

`--profile smoke` checks plumbing and omits full-cycle/repeat coverage.
`--profile thorough`, `--repeats`, `--warmup-seconds`, and
`--measurement-seconds` remain available. The diagnostic drain is one second
(0.2 seconds for smoke), followed by a five-second normal-shutdown bound.
Cancellation stops only owned processes and retains partial reports; the selected
physical display and neighboring sessions remain owned by their original users.

`report.md` summarizes execution mode, selected/omitted coverage, builds,
conditions, instrumentation, actual windows, cadence, content outcomes and
latencies, repeat variation/deltas, and representative delivery gaps. It links
raw evidence and the matching `report.json`, which retains complete assets,
capabilities, toolchain metadata, exact associations, event timestamps and
per-publication latencies. Both identify invalid evidence separately from
requested contract failures.

By default, valid cadence is observed without imposing a numeric contract. Use
`--max-missed-refreshes N` to request a per-run ceiling on missed eligible
refreshes. The contract is unavailable when cadence evidence is missing or
invalid; it cannot pass through a null-to-zero conversion. This contract concerns
cadence; unavailable content latency remains separately visible.

Exit codes are 0 for completed collection, 1 for execution/prerequisite failure,
2 for invalid evidence or CLI syntax, 3 for a behavioral/requested-contract
failure, 4 for an unavailable physical capability, and 130 for cancellation.
Successful collection does not imply optical verification or successful
repository correctness/presentation checks.

Keep artifacts local. Handoffs should link the report, identify observed behavior
and overhead, explain omitted coverage and uncertain comparisons, and explicitly
state whether actual physical association and first-visible-content timing were
validated on the selected environment. Do not label headless integration as that
validation. Run the required repository verification independently.
