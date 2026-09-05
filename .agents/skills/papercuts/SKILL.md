---
name: papercuts
description: "Capture workflow friction encountered during tasks: tool failures, confusing instructions, setup obstacles, and local host or permission problems. Review recorded observations on request."
---

# Papercuts

A papercut is an observed obstacle or unnecessary effort worth preventing,
explaining, or making easier to recover from. It may involve the repository,
local host, tooling, or agent workflow. One encounter is sufficient; recurrence,
root cause, and remedy may be unknown. Each independent encounter is one
occurrence; retries within it belong to the same observation.

Skip routine setup, ordinary implementation work, and harmless isolated typing
mistakes. Capture recurring command pitfalls and unexpected setup or recovery
effort. Product correctness, security, and user-data problems still require
their normal handling.

The host-local log is `~/.local/state/roonscape/PAPERCUTS.md`, shared by all
RoonScape worktrees. Create the parent directory if absent.

For an incident encountered during a task, follow Capture below. For an explicit
request to review recorded observations, follow [REVIEW.md](REVIEW.md).

## Capture

The maintainer authorizes appends to this local log during all tasks, including
read-only tasks and questions, without asking permission for each incident.
This permission covers log entries only.

1. Compose a concise observation from evidence already gathered for the current
   task. Lead with the friction, then explain the attempted work, impact, and
   relevant circumstances. Include useful diagnostics, evidence references,
   or workarounds; distinguish facts from suspected causes and omit secrets.
   Keep any extra check brief.
2. Append the complete entry in one append-mode write, creating the file if
   absent. Capture never reads, searches, or deduplicates the log.
3. Report the capture tersely in the final task summary. If appending fails,
   report the candidate and capture failure once, then continue the main task.
   Logging failures can be recorded when logging is available; avoid recursive
   attempts to record the failure.

Use UTC and the model name supplied by the session, or `unknown` if unavailable:

```markdown
- [ ] `YYYY-MM-DDTHH:MM:SSZ` — `model` — <observation>
```
