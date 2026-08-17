# Review papercuts

An explicit papercut review authorizes changes to the clone-local log, not to
the repository or GitHub. An explicit read-only instruction still takes
precedence: report proposed log changes instead of applying them.

Locate `PAPERCUTS.md` beneath the directory returned by
`git rev-parse --path-format=absolute --git-common-dir`, then read the complete
log. If it is absent or empty, report that and stop.

Review every observation:

1. Apply the papercut test from `SKILL.md`. Delete environment noise, unsupported
   workflows, product problems, and anything else that fails it.
2. Group observations only when they describe the same obstruction with
   substantially the same repository-side cause or remedy. Keep related but
   distinct friction separate.
3. Preserve every occurrence's timestamp and agent label. Combine grouped
   observations into a cumulative occurrence count.
4. Use the smallest direct sanity check to verify that each surviving papercut
   remains reproducible. Delete entries that are already fixed or disproven.
5. When verification would require meaningful setup or investigation, retain
   the entry and ask the user whether to pursue it.

Rewrite the log with open papercuts only. Represent grouped observations as:

```markdown
- [ ] <friction; optional corrective direction> — **N occurrences**
  - `YYYY-MM-DDTHH:MM:SSZ` — `agent`
  - `YYYY-MM-DDTHH:MM:SSZ` — `agent`
```

New raw observations may appear after previously grouped entries; incorporate
them into the cumulative count when equivalent.

Report a compact summary of:

- Noise and fixed entries removed.
- Surviving papercuts and their occurrence counts.
- Entries that could not be verified cheaply.
- Repository fixes or GitHub promotion the user may wish to authorize.

Do not implement fixes or create, modify, or close GitHub issues during review.
Remove resolved or manually promoted papercuts rather than maintaining a local
archive.
