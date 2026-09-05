# Verification policy

Select local verification by what the change affects. Ordinary CI continues to run
repository checks, design tests, and representative fallback captures for all
changes.

## Choose checks

During implementation, choose focused typechecking and tests for the affected
area. Existing focused commands and reusable `:built` stages are listed in
`package.json`; build prerequisites before invoking a built stage directly.
Before running agent-driven checks, follow [Agent execution permissions](#agent-execution-permissions).

Use the most demanding applicable row, including for mixed changes:

| Change | Required local verification |
| --- | --- |
| Documentation or agent guidance with no executable or presentation changes | Review accuracy, links, referenced commands, and affected skill conventions; run `git diff --check` |
| Code, dependencies, configuration, or build/test tooling | `npm run verify` |
| Renderer, presentation design, Fixture Scenarios, artwork, fonts, styles, capture planning/execution, or related build/native orchestration | `npm run verify -- --design`, plus applicable presentation review below |

For executable checks, read [execution guidance](#executable-checks-and-presentation-work)
and [evidence inspection](#inspect-retained-evidence). For presentation changes,
also follow [Presentation review](#presentation-review). Documentation-only work
uses the guidance in this section.

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

Use a prepared worktree as described in
[development preparation](../development.md#prepare-an-existing-worktree).
No Roon Server or Roon Authorization is required for these checks. General
network access is allowed; verification does not install host packages or
prepare dependencies automatically.

1. Resolve missing prerequisites or execution permissions using the relevant
   [host provisioning instructions](../development.md#provision-the-development-host-explicitly)
   before claiming verification.
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

When diagnosing or changing isolation, readiness, or cleanup, read the
[native-runtime reference](native-runtime.md).

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

### Agent execution permissions

Verification requires subprocess IPC, including local sockets used by Node's
captured input/output, Xvfb, and D-Bus. Some agent sandboxes deny these operations;
even a successful child exit can carry a pipe error or lose stdout/stderr.

When that restriction is established in the current environment, request
escalated execution for `npm run verify` (including its applicable options),
`npm run dev:diagnose`, `npm run dev:prepare`, and focused checks that use the
affected subprocess or native IPC. Use the normal approval mechanism for each
specific command; Auto-review can assess eligible requests while ordinary
agent work remains sandboxed. Authorization still depends on the command and
its effects. If review denies it, report the denial and remaining verification
gap rather than treating a sandboxed retry as equivalent evidence.

In an unassessed environment, the development diagnostic first checks that a
child Node process executes and returns known stdout and stderr. A failure
means tool prerequisites were not assessed. Request the execution permissions
needed by that command before interpreting tool or font readiness. Once the
restriction is known, use the appropriate execution permissions directly on
subsequent affected checks instead of repeating the failing probe.

Retain the normal verification workflow, private configuration and native
sessions, bounded cleanup, and evidence logs. The scripts diagnose execution
capabilities; they never escalate themselves or change sandbox policy.

## Inspect retained evidence

The command prints a unique `/var/tmp/codex/roonscape/review.*` directory. Open
its `README.md` for the source revision, working-tree state, environment,
automated outcome, and links to each command's stdout/stderr logs.
`verification.json` records timestamps, supervisor details, and exit codes/signals. Inspect
logs while a command runs; the index remains incomplete until completion.
CI uploads these directories even when verification fails.

Failed checks stop the sequence. SIGINT/SIGTERM mark cancellation and stop owned
processes before removing disposable runtime resources. Let cleanup finish;
completed logs remain and later runs use new review directories. Keep `review.*`
directories as retained deliverables, separate from disposable task scratch.
Treat unfinished indexes as incomplete, including after SIGKILL or host failure.

Automated completion does not establish Presentation Capture completion or
visual acceptance. `--presentation-ci` additionally generates the maintained
representative fallback-font capture scope after repository and design checks;
its command failure fails the workflow while `automatedOutcome` separately
records successful checks. Ordinary CI uses this conservative scope, including
for presentation-related changes. Release builds use `--design` and retain
check logs without generating this capture scope. Both workflows upload the
entire retained directory even on failure, including any partial evidence.
Never add environment dumps, credential files, or authorization material to
review evidence.

When reporting live observation, distinguish the
[Live Capture Session helper tests](#live-capture-session-helper-tests) from an
actual Live Capture Session.

## Presentation review

For presentation changes, follow the [presentation visual-acceptance workflow](../visual-acceptance/presentation.md).
It defines required scope selection, viewport coverage, image inspection, verdict
recording, and judgments requiring human review. Extend the completed verification
directory with its capture evidence and verdict.

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
