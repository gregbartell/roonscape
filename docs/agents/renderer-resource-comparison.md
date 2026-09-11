# Renderer resource comparison

Run `npm run compare:renderer` before finishing potentially performance-affecting
Renderer work. Select the original revision and candidate explicitly. Reuse a
comparison while its source, build, workload, and execution conditions still
cover the final change; a comparison is not required after every edit.

```sh
npm run compare:renderer -- --baseline ref:HEAD --candidate worktree:. \
  --output /var/tmp/codex/roonscape/resource-review
```

`ref:REVISION` resolves a commit in the calling repository. `worktree:PATH`
snapshots tracked files, tracked deletions, and nonignored untracked files from
that repository root, including staged and unstaged changes. It never checks
out, commits, or builds inside the selected source. Symlinks and submodules are
rejected. Keep sources stable while snapshotting. Content SHA-256 manifests,
HEAD revision, and dirty state identify the exact copied input. Output must be
a new directory; keep evidence outside source trees. Nothing is published.

Use a prepared development host and `npm run dev:prepare` in a fresh worktree.
The command needs Linux `/proc`, Python 3's standard library, Git, tar, Cargo,
Rust, Qt/native build prerequisites, Fontconfig, Xvfb, xwininfo, and dbus-daemon.
It installs no host packages. It builds both snapshots with locked dependencies,
Cargo's release profile, and optimization level 3, retaining separate binaries,
fonts, icons, build logs, and source manifests. Build outputs are task-owned;
dependency compilation is shared sequentially between the two snapshots.
Preparation/build durations are separate from resource collection.

A retained build can be selected without recompilation. Its binary and packaged
resources are checked against the retained digests, and its original compiler
and source identity remain in the report:

```sh
npm run compare:renderer -- \
  --baseline build:/var/tmp/codex/roonscape/resource-review/baseline \
  --candidate build:/var/tmp/codex/roonscape/resource-review/candidate \
  --workloads lyrics,replacements --profile thorough \
  --output /var/tmp/codex/roonscape/resource-review-thorough
```

Both sources must have the maintained Presentation Snapshot and Display
Configuration schemas. Schema differences are rejected as incompatible inputs;
this command does not translate old contracts. Workload content comes from the
invoking command's maintained Fixture Scenarios, with common copied artwork.
Workload schedules, content, and artwork digests are retained in `report.json`.

## Workload and duration choices

The default selects all ten workloads at 1280×720. It uses two repeats,
each with one second of warmup and eight seconds of measurement per Renderer
and workload. Three content workloads also run in separate diagnostic processes
on supported builds: 52 runs, about eight minutes of scheduled work including
diagnostic drain, plus startup and cleanup. It
targets completion within ten minutes **after preparation and builds**. This
is a practical target, not a machine-specific CI assertion. Every selected
workload runs; the command never truncates the set to meet a time budget.

`--profile thorough` uses five repeats with five seconds of warmup and thirty
seconds of measurement (about 76 minutes of scheduled work including diagnostic passes).
`--profile smoke` uses one repeat with 0.1 seconds of warmup and 0.4 seconds of
measurement, solely to exercise the plumbing. Smoke does not cover full cycles
or repeat variation. Normal verification runs deterministic command tests and
a small real-Renderer integration exercise, using its already-built debug
executable behind a controlled compiler. That exercise is not release resource
evidence. Full comparisons are invoked separately.

Use `--workloads` with a comma-separated selection:

| Name                 | Coverage                                                                         |
| -------------------- | -------------------------------------------------------------------------------- |
| `progress`           | Determinate progress and continuing local timing                                 |
| `lyrics`             | Settled Synchronized Lyric Composition and adjacent Natural Cue Handoffs         |
| `lyric-transitions`  | Entry and exit of Synchronized Lyric Composition, with an Intentional Blank      |
| `replacements`       | Now Playing Transitions and replacements interrupted 100 ms apart                |
| `activity`           | Starting activity, Idle, inactivity dimming/repositioning, and return to Playing |
| `static`             | Static Fixture Mode                                                              |
| `reduced-animation`  | Lyric, Now Playing, and Presentation Status changes with reduced animation       |
| `fresh-content`      | Unique artwork paths/revisions first published inside measurement                |
| `reused-content`     | Alternating already-prepared artwork identities                                  |
| `superseded-content` | Fresh replacements interrupted 10 ms apart, followed by a settled replacement    |

`--resolution WIDTHxHEIGHT` chooses a common landscape viewport from at least
1280×720 through 8192 pixels wide. For a focused investigation,
`--repeats 1..20`, `--warmup-seconds 0.1..60`, and
`--measurement-seconds 0.2..300` override the selected profile. Short measurements
explicitly omit full-cycle coverage. Each repeat starts a fresh Renderer;
longer measurements repeat the workload within that process to expose memory
behavior across repeated work.

## Evidence and interpretation

Baseline and candidate run sequentially; their order alternates each repeat.
Each uses a private native session, identical display/configuration, explicit
animation preference, unit scale, and software rendering. Roon Server, Roon
Authorization, and real playback are unnecessary. Personal configuration and
system power policy are untouched. Avoid other comparisons, builds, profiling,
tracing, and substantial analysis during resource collection. Reports record
machine/OS/toolchain conditions, CPU affinity, load, font inventory digest, and
instrumentation. Environmental load and thermal drift remain sources of noise.

Publications use elapsed-time deadlines, independent of Renderer reads or paint
acknowledgements. The original workloads restart their schedule at measurement. Content workloads
warm up only the seed; fresh measured artwork identities never enter the
Renderer during warmup. Assets are materialized before measurement, with
identical bytes and paths on both sides. This exercises uncached Renderer
preparation, without claiming a cold OS file cache. Planned
and actual publication times are retained. Backpressure, missing publications,
or delivery more than 50 ms late invalidate evidence; they never extend the
workload or make it easier for a slow Renderer. These are workload-validity
checks, not resource veto thresholds.

The sampler reads `/proc/PID/stat` every 100 ms (50 ms for smoke). CPU time is
user plus system **seconds across Renderer threads**. CPU utilization is CPU
seconds divided by the sampled elapsed seconds × 100; one logical CPU is 100%,
so multithreaded utilization can exceed 100%. RSS is bytes for that process.
Samples exclude the publisher, sampler, Xvfb, child processes, and GPU memory.
Actual sampling intervals and receipt times are retained. First/last samples
inside measurement bound the measured CPU interval; the interval can be shorter
than the scheduled duration. Sampled peak RSS can miss short transients.

Matching `report.md` and `report.json` contain per-repeat CPU, utilization,
peak RSS, first/last RSS and growth, descriptive mean/range/sample standard
deviation, absolute and relative deltas, actual durations/order, and links to
retained evidence. RSS growth and repeat endpoints can motivate longer runs;
they do not establish a memory leak. Zero baselines have undefined relative
deltas, unavailable metrics remain null, and one repeat has no variation
estimate. No confidence interval, statistical significance, physical cadence,
or correctness/presentation acceptance is implied.
No universal CPU/RSS threshold is applied, and increases are advisory.

Exit 0 means completed collection (individual content observations can remain
unavailable or incomplete), 2 invalid evidence (or invalid CLI syntax),
3 an observed content behavior failure, 1 execution/prerequisite failure,
and 130 SIGINT/SIGTERM cancellation.
Failure and cancellation retain partial reports and completed artifacts while
bounded cleanup stops only owned processes and removes private runtime trees.
The command does not run correctness or presentation contracts: run the required
repository checks separately and report their failures separately from resource
findings. SIGKILL or host failure cannot run cleanup.

## Content observations

The three content workloads run again after clean resource collection, in
separate instrumented processes. Their CPU/RSS samples remain identifiable in
JSON and never enter clean resource summaries. Builds without the content
recorder report diagnostic coverage unavailable. The report identifies recorder
version, source, bounds, clock, and drain duration; comparing different recorder
implementations can introduce measurement differences.

Publication and content identities, artwork digests, preparation tokens, frame
and scene identities use the existing Snapshot revision without changing its
contract. Timestamps are microseconds on Linux CLOCK_MONOTONIC within this host
boot; windows and actual publication timestamps use the same clock. Preparation
completion means prepared resources are available to the worker. Native readiness
means a native draw callback observed the installed content, including positive
artwork weight. Neither proves physical display delivery or first visibility.

Per-publication and per-repeat latencies, outcome counts, descriptive variation,
and baseline/candidate deltas appear in Markdown/JSON. Repeat means include only
available observations; compare identities and outcome counts before interpreting
latency differences. `delayed` means readiness was observed after measurement in
the bounded drain; `superseded` means a replacement overtook the publication;
`incomplete` means received content had no readiness before shutdown; `unavailable`
means no receipt or no supported recorder. Missing milestones remain null.
Preparation errors, failed prepared-resource reuse, retained handles after normal
shutdown, and first drawing of discarded preparations are behavioral failures.

Texture creation/release records describe Rust handle lifetime, including peak
retained counts and a timeline across replacements. They do not observe GPU
retirement fences or prove immediate GPU deallocation. Compare these with clean
sampled RSS endpoints, peaks, and repeat variation; use longer repeated workloads
to investigate suspected growth. No universal resource threshold is imposed.

The opt-in recorder enqueues without waiting on the rendering thread. A worker
writes the file; the queue holds at most 1,024 records and recording permits
65,536 records of at most 64 KiB each. The reader also rejects files over 64 MiB.
Overflow, malformed records, missing normal-shutdown footer, or lost records
invalidate evidence. Normal completion allows a one-second diagnostic drain
(0.2 seconds for smoke), requests native shutdown, and drains the writer, with a
five-second process bound. Cancellation retains partial observations and reports;
partial data does not become completed latency evidence.

Handoffs should link the report and explain measured increases, deliberate
tradeoffs, behavioral improvements, omitted coverage, and uncertainty. Do not
claim an optimization without measurements supporting it. Retain raw evidence
outside committed content; do not commit historical results or completion logs.
