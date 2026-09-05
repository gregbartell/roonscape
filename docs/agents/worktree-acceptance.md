# Two-worktree acceptance

Use this opt-in exercise after implementation changes to development preparation,
native isolation, verification, or presentation evidence orchestration. It is
not part of `verify`: it invokes that command in each supplied worktree.

## Prepare and run

Provision a supported Linux host using [Development](../development.md), including
network access for locked downloads, toolchain/cache execution and writes, native
processes and sockets, and writable `/tmp` and `/var/tmp/codex/roonscape`.
Do not run setup, request Roon Authorization, or provision live access.

Supply two **already-created, clean, distinct worktree roots**, each without
`node_modules`, `target`, or `src/bridge/dist`. Select their revisions yourself;
use the same candidate revision for acceptance of one change. The exercise
validates these conditions before preparing either worktree. It never creates
or deletes Git worktrees, switches branches, or provisions the host.

```sh
npm run accept:worktrees -- /absolute/worktree-a /absolute/worktree-b
```

This command needs only Node before preparation. It retains a new
`/var/tmp/codex/roonscape/acceptance.*` directory and the separate `review.*`
directories printed by verification. Keep all three together when sharing evidence.
Build products remain in their respective worktrees. Shared dependency download
caches are allowed; build output sharing is not.

The exercise performs these observable steps:

1. Run `dev:diagnose` and `dev:prepare` in each fresh worktree. Start a static
   headless Idle Fixture Mode sentinel through `npm run fixture` in A, using its
   own private configuration, Xvfb display, D-Bus, and native Renderer.
2. Start `npm run verify -- --presentation-ci` concurrently in A and B. Observe
   overlapping native session lifetimes and distinct runtime/configuration and
   review directories. Both runs must complete repository checks, design tests,
   and the maintained fallback capture scope. Inspect source identity, clean
   working-tree state, command exit codes/logs, runtime removal, coverage, PNG
   publication, and image links.
3. Start focused `review:presentations:built` commands in both worktrees,
   reusing the prerequisites each verification command just built. A captures
   Playing, Idle, long metadata, and light artwork; B captures Playing and Idle.
   Each uses all seven maintained viewports. This selection exercises Now Playing,
   Full-field, typography pressure, and palette publication without claiming a
   complete presentation profile. It changes no design or Fixture Scenario.
4. After B publishes at least one image, while its native Renderer and A's
   review remain active, send SIGTERM to B's owned review CLI. npm's shell does
   not reliably forward signals, so the exercise identifies the CLI among its
   owned descendants. Require exit 130 within five seconds, a cancelled partial
   index with retained images/diagnostics, absent observed descendant PIDs, and
   removal of B's temporary runtime resources. Require A to remain active and
   subsequently finish its full requested set.
5. Probe the sentinel's mapped window, identity, and dimensions before/during
   verification, after cancellation, and after A completes. Close the sentinel
   and check resource removal before deleting the exercise's own runtime tree.
   Process observation collects only PIDs, parent PIDs, and executable names,
   never command lines or environments.

The expected cancellation is success for the **exercise**, but remains
`cancelled` in the affected capture index. The earlier verification results
remain complete. Any unexpected command, assertion, timeout, or cleanup failure
makes the exercise nonzero and preserves its evidence. SIGINT/SIGTERM stops only
owned processes. SIGKILL or host failure cannot guarantee cleanup; do not remove
neighboring resources or infer ownership from an X display number.

## Inspect and report

Open `acceptance.*/README.md` and `acceptance.json`, both verification indexes,
and every linked presentation index. Confirm the recorded source roots/revisions
match the intended worktrees, the native sessions overlapped, cancellation was
bounded, cleanup passed, and the sentinel retained its window identity. Check
requested/completed counts and image links, including the cancelled set.

Automation validates publication and accounting, not appearance. Open every
completed image and record a separate verdict through
[`review:presentations --record`](../visual-acceptance/presentation.md). Include
filenames, the selection rationale, concrete visual reasons, and unresolved
judgments. Partial and fallback sets cannot be accepted as complete typography
coverage. Do not replace an automated outcome with a visual verdict.

The completion report must link retained evidence, list commands and outcomes,
separate automation/capture completion/visual inspection, and state whether
physical-display and live observation occurred. The sentinel demonstrates
controlled native noninterference; window probes do not establish actual Live
Mode behavior, motion quality, or physical display readability/color.

If Live Mode is already available, a maintainer may additionally observe that
its window and current playback presentation remain undisturbed while the
exercise runs. Record the observation and its limits separately. Do not start,
configure, capture, or disturb personal Live Mode just to satisfy this exercise.
Lack of live access does not block automated acceptance. Synthetic Live Capture
Session helper tests do not verify a real Live Capture Session; that still
requires Roon and a human to cause the event being observed.

## Ubuntu CI

Ordinary CI runs `verify -- --presentation-ci`. For this acceptance exercise,
run the **CI** workflow manually on a published candidate ref with the
`two_worktrees` input enabled. The workflow explicitly creates the two worktrees
before invoking the exercise; repository preparation tooling does not do so.
Both paths use Ubuntu 22.04 and the shared native provisioning action.

```sh
gh workflow run ci.yml --ref <published-candidate-ref> -f two_worktrees=true
gh run view <run-id> --log
gh run download <run-id> --name verification-<run-id>-<attempt> --dir <destination>
```

Dispatching a workflow or publishing a ref requires the applicable session
permission. A local pass is not an Ubuntu CI run. Record the workflow run URL,
revision, conclusion, and downloaded artifact location. Open the downloaded
indexes and images, including the intentionally cancelled capture set, and
confirm the always-run artifact step retained success and incomplete evidence.
If CI fails earlier, report which evidence exists and which steps never ran.
Fallback-font coverage remains representative CI evidence; complete typography
acceptance requires the workstation profile. CI never authors a visual verdict.
