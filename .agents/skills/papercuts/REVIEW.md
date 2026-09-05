# Review papercuts

An explicit request to review recorded papercuts authorizes organizing and
rewriting the local log. Fixes and issue promotion require separate
authorization. Assume no other agent is writing the log during review.

Read the complete log at the location defined in `SKILL.md`. If it is absent or
empty, report that and stop.

Review every observation:

1. Apply the papercut definition in `SKILL.md`; remove entries that fail it.
2. Assess current status using existing evidence or a brief sanity check.
   Remove entries established to be resolved or disproven, and manually promoted
   entries whose useful diagnostic information survives in the destination
   issue. Retain plausible observations with uncertain status, noting what
   investigation would resolve it. A workaround, temporary recovery, or inability
   to reproduce immediately does not establish resolution of the friction.
3. Group retained observations when their symptoms and context support treating
   them as the same obstruction; preserve meaningful environment differences.
   Merge new observations into existing groups when equivalent, updating the
   cumulative occurrence count. Preserve each occurrence's timestamp, model
   label, and useful diagnostic details, impact, workarounds, and evidence
   references. Consolidate redundant prose while keeping facts and suspected
   causes distinguishable.

Rewrite the log with the remaining open observations, including uncertain ones.
Use this format for groups:

```markdown
- [ ] <shared observation and current assessment> — **N occurrences**
  - `YYYY-MM-DDTHH:MM:SSZ` — `model` — <distinctive details or evidence, if any>
  - `YYYY-MM-DDTHH:MM:SSZ` — `model` — <distinctive details or evidence, if any>
```

Report a compact summary of:

- Removed entries and the reasons for removal.
- Surviving papercuts and their occurrence counts.
- Uncertain status and proposed investigations.
- Possible remedies and suitable issue promotion.
