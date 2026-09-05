# Verification policy

Select local verification by what the change affects. CI continues to run
repository checks, design tests, and representative fallback captures for all
changes.

## Choose checks

During implementation, choose focused typechecking and tests for the affected
area. Existing focused commands and reusable `:built` stages are listed in
`package.json`; build prerequisites before invoking a built stage directly.

Use the most demanding applicable row, including for mixed changes:

| Change | Required local verification |
| --- | --- |
| Documentation or agent guidance with no executable or presentation changes | Review accuracy, links, referenced commands, and affected skill conventions; run `git diff --check` |
| Code, dependencies, configuration, or build/test tooling | `npm run verify` |
| Renderer, presentation design, Fixture Scenarios, artwork, fonts, styles, capture planning/execution, or related build/native orchestration | `npm run verify -- --design`, plus applicable presentation review below |

Run applicable final verification when the change is ready. Reuse successful
results while they cover the final change and relevant environment. Rerun when
subsequent changes, failures, or new evidence invalidate that coverage; preparing
a commit alone does not require another run.

### Documentation-only changes

Select this path by content, not file extension. Review changed commands against
their implementations and changed skill instructions against their consumers;
run focused checks where needed to establish correctness. Changes to
presentation requirements follow the presentation row even when only Markdown
files change.

This path requires no native toolchain preparation, dependency installation,
or retained `review.*` directory solely for documentation review. Report the
scope reviewed, checks performed and their outcomes, and any limitations.

## Executable checks and presentation work

Use a prepared worktree as described in [Development](../development.md).
No Roon Server or Roon Authorization is required for these checks. General
network access is allowed; verification does not install host packages or
prepare dependencies automatically.

1. Before preparing an existing worktree, read [Development](../development.md).
   Select the pinned toolchains and explicitly provision host prerequisites;
   then run `npm run dev:diagnose` and `npm run dev:prepare`. Resolve missing
   capability or execution permissions before claiming verification. These
   commands do not select branches, create worktrees, or access Roon.
2. Use headless commands for unattended native work. Use an explicit desktop
   only for requested manual inspection, for example
   `DISPLAY=:0 ROONSCAPE_WINDOWED=1 npm run fixture -- --static`.
3. Inspect retained command logs and evidence indexes; for presentation work,
   open the required captures and record a separate visual verdict. Report the
   revision, working-tree state, commands/outcomes, evidence links, inspected
   scope and rationale, and unresolved visual, physical-display, or live claims.
4. Native commands own their configuration, private home/XDG directories, Xvfb,
   D-Bus, and child processes. Let their bounded cleanup finish after SIGINT or
   SIGTERM. Keep retained `review.*` evidence; remove only task-owned disposable
   runtime artifacts. Do not stop neighboring sessions or delete resources
   based merely on an observed display number.

For implementation changes to preparation, isolation, verification, or evidence
orchestration, use the [two-worktree acceptance exercise](worktree-acceptance.md)
to reproduce concurrent success, cancellation, evidence retention, and sentinel
checks.

`verify` runs the preparation diagnostic, creates a private Xvfb/D-Bus native
session, then runs `npm run check`. `--design` additionally runs the existing
`npm run test:design` suite. Every `verify` invocation runs repository checks. The
underlying focused entry points remain usable independently. Select `--design`
from the change scope; the command does not infer it from Git history.

Checks build and execute this worktree's code. Build directories are pinned to
its `target`; preparation rejects symlinked build/dependency directories.
The session uses private home/XDG directories and preserves toolchain/cache
locations. It does not read personal Display Configuration or Roon Authorization.

## Inspect retained evidence

The command prints a unique `/var/tmp/codex/roonscape/review.*` directory. Open
its `README.md` for the source revision, working-tree state, environment,
automated outcome, and links to each command's stdout/stderr logs.
`verification.json` records timestamps, supervisor details, and exit codes/signals. Inspect
logs while a command runs; the index remains incomplete until completion.
CI uploads these directories even when verification fails.

Failed checks stop the sequence. SIGINT/SIGTERM mark cancellation and stop owned
processes before removing the private runtime directory. On Linux, a Python
standard-library subreaper supervises each command and adopts detached native
descendants when their test owner exits. Cleanup signals only its children,
allows 250 ms for termination, then kills and reaps survivors. The outer
process monitor bounds supervisor termination to its existing two-second grace
and two-second escalation waits. Per-command exit codes in the index are the
supervisor's shell-style outcomes (130 for cancellation). Nested temporary
resources live beneath the run's disposable command runtime directory. Completed logs remain;
subsequent runs never reuse a review directory. Review directories are intentional
retained deliverables, separate from disposable runtime resources. Do not remove
them as ordinary task scratch. SIGKILL/host failure cannot run cleanup; an
unfinished index must be treated as incomplete.

Automated completion does not establish Presentation Capture completion or
visual acceptance. `--presentation-ci` additionally generates the maintained
representative fallback-font capture scope after repository and design checks;
its command failure fails the workflow while `automatedOutcome` separately
records successful checks. All CI runs use this conservative scope, including
presentation-related changes. CI uploads the entire retained directory even
on failure, including useful partial images and diagnostics.
Never add environment dumps, credential files, or authorization material to
review evidence.

## Presentation review

For presentation changes, extend the completed verification directory using
[`review:presentations`](../visual-acceptance/presentation.md). Follow that guide
for command syntax and the maintained scope listing. Select affected Fixture
Scenarios with a written rationale and inspect every image at all seven
representative viewports. Shared typography, layout, or palette changes require
the complete profile on a workstation with the required host fonts.

Open the image index, account for every requested/completed capture, and record
an explicit verdict with reasons, inspected filenames, and unresolved judgments.
Capture completion alone leaves visual acceptance unreviewed. Focused acceptance
covers only its selection; partial capture sets and CI fallback evidence cannot
claim complete-profile acceptance. Record physical-display or subjective judgments
requiring human review as unresolved. Keep capture evidence alongside automation
logs rather than committing images or comparing pixel goldens.

## Live Capture Session helper tests

`npm run test:live-capture-helpers` runs the existing option/environment,
timestamp, frame-selection, and synthetic-video evidence processing/publication
tests. It requires FFmpeg/FFprobe on PATH and a writable project scratch root;
the command creates that root if missing. These tests are included in required
repository automation and CI.

Passing helper tests does **not** verify an actual Live Capture Session. They
never start a session or contact Roon. An actual Live Capture Session requires
Roon, existing authorization/setup, and a human to cause the observed event;
it is outside this verification workflow.
