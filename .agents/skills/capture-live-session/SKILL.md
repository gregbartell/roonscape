---
name: capture-live-session
description: Capture and curate a time-ordered visual chronology from RoonScape Live Mode.
---

# Capture a Live Session

Produce human-review evidence of an event visible in RoonScape Live Mode. This
workflow observes Roon; it never performs Roon Control or changes setup.

Before recording, establish:

- The event of interest and the visible pre-event and concluding states.
- Any explicit acceptance criteria. Without them, report observations without
  a pass/fail verdict.
- Whether responsive or fullscreen behavior is under examination. Default to
  a windowed 1280x720 session. Use another supported landscape resolution when
  requested, and use fullscreen only when requested or when the event clearly
  depends on fullscreen behavior.

## Record

Start the helper as a long-running command from the repository root:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs record --event "<event description>"
```

Add `--resolution WIDTHxHEIGHT`, `--fullscreen`, `--duration SECONDS`, or
`--roon-server HOST` only when the request calls for them. The helper
preflights its optional tools and existing RoonScape setup, refuses to disturb
another Live Mode session, builds the app, starts an isolated X display, and
records losslessly at 20 fps. It prints the session directory before
preparation and prints `runtime-ready` when visual inspection can begin. Its
two-minute recording timeout is a hard limit.

While the record command continues, capture the current observation frame:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs snapshot --session <session-directory>
```

Inspect the printed image path. Poll often enough to establish that the
requested pre-event presentation is stable. Only then tell the user that
capture is ready. If the event is already underway or no trustworthy baseline
appears, stop and treat the session as incomplete.

Continue inspecting observation frames while the external actor causes the
event. Stop autonomously when the requested concluding state is clear, or use
the user's declaration that the event is done. In either case, observe two
additional seconds before stopping. Restart that two-second period only for a
meaningful, event-relevant presentation change; routine progress, waveform,
and incidental continuous motion do not restart it.

Request an orderly stop with:

```sh
node .agents/skills/capture-live-session/scripts/live-capture-session.mjs stop --session <session-directory>
```

An explicit duration and the two-minute timeout remain hard limits and receive
no additional post-roll. Wait for the record command to report `recorded`.

## Curate

After recording ends, read [references/curation.md](references/curation.md)
completely and follow it. The session is complete only after the published
Live Capture Frames, timeline, overview, and findings are validated and the
temporary recording has been removed.

When launch, capture, or observation is incomplete, preserve only diagnostics
that materially explain the failure. Otherwise run the helper's `discard`
command so task-owned temporary material does not survive.
