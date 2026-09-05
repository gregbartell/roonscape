# Two-worktree acceptance

Use this manual diagnostic when changes to fresh dependency preparation or
build isolation need coverage beyond the existing regression tests. Ordinary
feature, verification-tool, capture-tool, and runtime-isolation changes do not
require it. Native-session, verification, and design tests already exercise
concurrent runtime isolation and cancellation using built executables.

This exercise deliberately starts with fresh dependencies and build directories,
so its cost includes initial compilation and is outside the prepared-worktree
verification target. It is not part of `verify`: it invokes that command in each
supplied worktree.

## Prepare and run

Provision a supported Linux host using the
[host provisioning instructions](../development.md#provision-the-development-host-explicitly),
including network access for locked downloads, toolchain/cache execution and
writes, native processes and sockets, and writable `/tmp` and
`/var/tmp/codex/roonscape`.
Do not run setup, request Roon Authorization, or provision live access.

Supply two **already-created, clean, distinct worktree roots**, each without
`node_modules`, `target`, or `src/bridge/dist`. Select their revisions yourself;
use the same candidate revision for acceptance of one change. The exercise
validates these conditions before preparing either worktree. It never creates
or deletes Git worktrees, switches branches, or provisions the host.

```sh
npm run accept:worktrees -- /absolute/worktree-a /absolute/worktree-b
```

This command needs only Node before preparation. Successful runs remove their
scratch diagnostics and captures. Failures leave an `acceptance.*` directory
under `/var/tmp/codex/roonscape`, plus any failed verification's `review.*`
directory. Build products remain in their respective worktrees. Shared dependency
download caches are allowed; build output sharing is not.

The exercise shares a thirty-minute work budget across preparation, builds,
verification, and captures. Each phase uses the smaller of its own deadline and
the remaining work budget; starting another phase does not reset that budget.
Expiry fails the exercise and enters bounded cleanup, which is outside the work
budget. CI allows thirty-five minutes for the exercise step within its existing
forty-five-minute job limit, leaving room for cleanup and evidence upload along
with host setup. These are hang watchdogs, not performance acceptance targets.
The separate five-second cancellation assertion below remains behavioral.

The exercise performs these observable steps:

1. Run `dev:prepare`, including its readiness preflight, in each fresh worktree.
   Start a static headless Idle Fixture Mode sentinel through `npm run fixture`
   in A, using its own private configuration, Xvfb display, D-Bus, and native Renderer.
2. Start `npm run verify -- --design` concurrently in A and B. Observe
   overlapping native session lifetimes and distinct runtime/configuration and
   review directories. Both runs must complete repository checks and design
   tests, including assertions on fallback captures. Check source cleanliness and
   removal of successful verification diagnostics and native runtime resources.
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
   capture set with diagnostic images, absent observed descendant PIDs, and
   removal of B's temporary runtime resources. Require A to remain active and
   subsequently finish its full requested set.
5. Probe the sentinel's mapped window, identity, and dimensions before/during
   verification, after cancellation, and after A completes. Close the sentinel
   and check resource removal before deleting the exercise's own runtime tree.
   Process observation collects only PIDs, parent PIDs, and executable names,
   never command lines or environments.

The expected cancellation is success for the **exercise**, but remains
`cancelled` in the capture progress file. The earlier verification results
remain complete. Any unexpected command, assertion, timeout, or cleanup failure
makes the exercise nonzero and leaves its diagnostics. SIGINT/SIGTERM stops only
owned processes. SIGKILL or host failure cannot guarantee cleanup; do not remove
neighboring resources or infer ownership from an X display number.

## Ubuntu CI

Ordinary CI runs `verify -- --design`. For this acceptance exercise,
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
permission. Failure diagnostics are available as GitHub Actions artifacts for
seven days. A local pass is not an Ubuntu CI run.
