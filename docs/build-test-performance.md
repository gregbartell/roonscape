# Local build and test performance

Measured on 2026-09-04 against `c8831d05c81483285d6942f7d7ccc08ed036003c`,
using an Arch Linux workstation with eight logical CPUs and 15 GiB RAM,
Node 24.19.0, npm 11.17.0, and Rust 1.97.1. Measurements use the repository's
private Xvfb/D-Bus environment and installed dependencies. Benchmark workloads
run sequentially; ordinary host activity remains a source of variation.

## Results

| Loop | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Unchanged Bridge build (`npm run build`) | 4.03 s | 1.23 s | 70% |
| Rebuild Rust tests after invalidating the Renderer library | 9.50 s | 2.35 s | 75% |
| Clean application prerequisites and Rust test compilation | 22.51 s | 12.00 s | 47% |
| Regular tests with prerequisites built (`npm run test:built`) | 59.96 s | 32.65 s | 46% |

Build figures are medians of three runs, excluding initial cache population.
Regular test execution has one before sample and three after samples
(29.61–32.81 seconds).
Treat these as workstation measurements, not CI performance thresholds.

"Clean application" means `cargo clean --package roonscape-renderer`, then
`npm run test:build` and `cargo test --workspace --no-run`. Third-party Rust
dependencies and npm packages remain cached. Library invalidation uses
`touch src/renderer/src/lib.rs` before `cargo test --workspace --no-run`;
this measures compilation/linking overhead rather than a particular code edit.
Unchanged Rust test compilation already took about 0.17 seconds and remains
about the same. Fully uncached dependency builds and optimized release builds
are outside these comparisons.

## Changes and tradeoffs

- The Bridge keeps TypeScript's dependency-aware incremental compiler cache
  inside `dist`. Removing `dist` resets both output and cache. A disposable
  copy of the real Bridge project verified changed source errors, changed
  imported declaration errors, recovery after correction, and rebuilding
  deleted output. The faster `tsc --build` experiment was rejected because it
  missed a changed imported declaration outside the configured root files.
- The regular Node suite runs its existing 298 tests together, with at most
  four isolated test-file processes. Two workers took 32.94 seconds and four
  took 20.74 seconds for this family alone. The nine Live Capture Session
  helper tests, Rust tests, and IPC smoke check still run afterward.
- Nineteen Rust integration modules share one executable, eliminating repeated
  compilation, linking, and schema initialization. All 231 Rust tests remain.
  Font registration retains a separate executable to preserve first-use
  initialization coverage. Test bodies and compiler/debug profiles are unchanged.
- A test-only rebuild of the snapshot-contract module increased from a median
  0.80 to 1.10 seconds. This is the tradeoff for reducing a library-change
  rebuild by about seven seconds. Test-name filters narrow execution; they
  do not narrow compilation within the shared executable.
- The opt-in design suite retains all 32 tests and its serial file scheduling.
  Two-file concurrency passed, but its 40.0-second isolated run did not translate
  into a consistent full-workflow improvement, so that experiment was not kept.
  The original design execution baseline was 45.5 seconds, excluding builds.
  Native fixtures continue to own their display, bus, configuration, processes,
  and cleanup. No tests were deleted, skipped, or moved out of required coverage.

## Repeat the measurements

Prepare the host as described in [Development](development.md). Start with
`npm run verify -- --design` to build prerequisites and establish a passing
baseline. Time commands one at a time using a monotonic clock or `/usr/bin/time`.
Record wall time, exit status, revision, working-tree state, and cache state.
Keep first-run compilation separate from repeated unchanged builds. Do not run
competing benchmark jobs concurrently.

Use `npm run verify -- --design` for the complete headless workflow. Its retained
`verification.json` provides separate timestamps and logs for repository checks
and the design suite. The `:built` stages require current prerequisites; native
checks must run in an isolated session as verification does. See the
[focused loop commands](development.md#fast-feedback-loops) for everyday work.

Wall-clock assertions would conflate host load with correctness, so no timing
test was added to the default suite. Existing behavioral checks, unchanged test
inventory, explicit cache-invalidation probes, and before/after timings supply
the acceptance signal.
