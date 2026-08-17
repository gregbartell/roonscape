---
name: papercuts
description: "Papercuts: capture small, repeatable, repository-fixable friction encountered in supported agent workflows; review the clone-local log only when explicitly asked."
---

# Papercuts

Keep the current task primary. A papercut is friction another agent could
encounter in a supported repository workflow and the repository could reduce
through its code, configuration, scripts, documentation, or agent instructions.

Use at most one small sanity check to establish both conditions. If deciding
would require more investigation, report the candidate for the user's judgment
without logging it.

Route product correctness, security, and user-data problems through normal work.
Treat unsupported environments, agent command mistakes, corrupted local state,
and transient external failures as environment noise. A missing prerequisite
qualifies only when the repository's guidance, checks, or error handling is the
friction.

## Capture

Capture only the incident at hand. Capture requires authorization to write.

When the current task is read-only or otherwise does not authorize writes,
finish the task, report the candidate tersely, and ask permission to record it.
The capture workflow is complete unless the user authorizes recording.

With write authorization:

1. Resolve the clone's shared Git directory with
   `git rev-parse --path-format=absolute --git-common-dir`.
2. Append one observation to `PAPERCUTS.md` in that directory. Do not read,
   search, or deduplicate the log.
3. Lead with the friction and keep the entry to one or two sentences. Include a
   likely correction only when it is immediately apparent.
4. Mention the capture tersely in the final task summary.

Use this observation format:

```markdown
- [ ] `YYYY-MM-DDTHH:MM:SSZ` — `model` — <friction; optional corrective direction>
```

Use UTC and your model name, such as `gpt-5.6-sol` or `opus-5`.

If appending fails, attempt no recovery beyond creating the local file. Continue
the main task and report the candidate and capture failure. A failure of this
logging mechanism is not itself a papercut.

## Review

When the user explicitly asks to review papercuts, read
[`REVIEW.md`](REVIEW.md) completely and follow it. Ordinary capture never reads
the log or the review procedure.
