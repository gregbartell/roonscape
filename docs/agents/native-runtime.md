# Native runtime

Read this reference when diagnosing or changing native-session isolation,
readiness, or cleanup. For routine execution, use
the [verification policy](verification.md#executable-checks-and-presentation-work).

## Isolation and registration

Headless Fixture Mode, Presentation Captures, and verification use
[`native-session.mjs`](../../scripts/native-session.mjs). Each session owns a
temporary Display Configuration, private home/XDG directories, an Xvfb display,
a D-Bus session bus, and monitored child processes. Inherited desktop and fixture
overrides are cleared before the isolated environment is configured.

Xvfb allocates and locks its display atomically, then reports readiness through
`-displayfd`. Concurrent runs never claim a display based on an observed socket.
[`process-harness.mjs`](../../scripts/process-harness.mjs) starts Xvfb with
`-noreset` so readiness probes disconnecting before the Renderer connects do not
reset it.

Renderer processes on separate displays remain independent when sharing a D-Bus
session, as covered by the
[native-session regression test](../../scripts/native-session.test.mjs).
The private bus isolates platform settings and has no service directories, so it
cannot activate host desktop services or leave unmonitored activation children.
The Bridge's runtime ownership lock prevents concurrent Live Mode launches using
the same runtime directory.
Headless sessions select the X11 backends at unit scale and clear inherited Qt
plugin, rendering, and per-screen scale overrides.

## Readiness and process cleanup

Native-session startup and window readiness waits have five-second bounds. Window
readiness checks require a mapped window at the requested dimensions. The
[Presentation Capture renderer](../../scripts/presentation-capture-renderer.mjs)
allows thirty seconds for each painted-revision acknowledgement and separately
for PNG encoding. Painted readiness includes presentation work and a matching
painted frame, which can exceed five seconds under concurrent rendering load.
Fixture Mode publisher startup also
allows thirty seconds: it includes Node startup and complete Fixture Scenario
catalog validation before socket publication. Composite launcher tests allow
forty-five seconds for that work plus native infrastructure startup; cancellation
assertions start only after setup.

The process harness owns detached process groups. Normal cleanup sends SIGTERM,
waits up to two seconds, then escalates to SIGKILL with a further two-second
bound. When passed an aborted cancellation signal, each wait is limited to
250 milliseconds so nested cleanup can finish before the caller escalates.
The harness also signals the owned group after its wrapper exits to stop any
remaining descendants.

The native session stops application processes before its display and bus.
Normal exit, startup failure, and SIGINT/SIGTERM cancellation use bounded
cleanup; cancellation exits nonzero. Runtime trees and unfinished publication
files are disposable. Completed captures, neighboring sessions, and personal
configuration remain intact. SIGKILL of the owner or host failure cannot run
application cleanup.

## Verification supervision

On Linux, [`verify.mjs`](../../scripts/verify.mjs) runs each command through the
Python standard-library [subreaper](../../scripts/verification-process.py).
The subreaper adopts detached native descendants when their test owner exits.
It signals only its children, allows 250 milliseconds for termination, then
kills and reaps survivors. The outer process monitor bounds supervisor
termination with its two-second grace and two-second escalation waits.

Per-command exit codes in `verification.json` are the supervisor's shell-style
outcomes, including 130 for cancellation. Nested temporary resources live under
the run's disposable command runtime directory; completed logs live separately
in the review directory, which is removed on success.

## Acceptance

The native-session, verification, and design regression tests cover concurrent
sessions, cancellation, diagnostic cleanup, and noninterference with an owned
sentinel. Use the [two-worktree acceptance exercise](worktree-acceptance.md) as a
manual diagnostic when fresh dependency preparation or build isolation needs
additional coverage. It is not a routine requirement for runtime changes.
