# 08 — Cut issue tracking over to GitHub

**What to build:** Make GitHub the configured destination for new RoonScape
work after the final local effort by applying the already-decided tracker,
triage, issue-form, and repository settings. Leave historical local work in the
owner's custody rather than importing or deleting it.

**Blocked by:** 07 — Verify the public repository restructure.

**Status:** done

- [x] The Matt Pocock setup workflow is run with GitHub as the tracker, the
      default five triage-state labels, external pull requests excluded from
      the request surface, and the existing single-context domain layout.
- [x] The setup workflow's required draft is presented for owner confirmation
      and the resulting agent instructions describe GitHub rather than local
      Markdown as the tracker for new work.
- [x] GitHub Issues are enabled for the public repository.
- [x] The existing `bug`, `enhancement`, and `wontfix` labels are retained, and
      `needs-triage`, `needs-info`, `ready-for-agent`, and `ready-for-human` are
      created exactly once with meanings matching the canonical roles.
- [x] The bug issue form applies exactly `bug` and `needs-triage` and requests a
      summary, environment, reproduction steps, expected behavior, actual
      behavior, and redacted logs.
- [x] The feature issue form applies exactly `enhancement` and `needs-triage`
      and requests the motivating problem, desired outcome, and alternatives.
- [x] Blank issues remain enabled and begin without an automatically assigned
      category or state.
- [x] Issue forms are valid GitHub issue-form YAML, and the local form
      configuration is ready to become active when the owner publishes the
      repository changes.
- [x] No completed local spec or ticket is recreated as a closed GitHub issue,
      and no personal-host work is added to the new tracker.
- [x] `.scratch` remains present and is not deleted, emptied, renamed, migrated,
      or otherwise cleaned up; deletion remains the repository owner's next
      action after local ticket completion.
- [ ] No commit, push, tag, release, or branch deletion occurs.

## Comments

### Implementation Result — 2026-08-16

The owner approved the setup draft and explicitly overrode the ticket's
no-commit criterion in favor of the implementation workflow's required local
commit. GitHub Issues are enabled, the four missing canonical state labels
were created without replacing the existing category labels or `wontfix`, and
GitHub still contains no issues.

The repository now carries guided bug and feature forms plus an unlabeled blank
issue path. Agent guidance uses GitHub for new work while preserving the
single-context domain layout and excluding pull requests from request triage.
The complete repository check and both required review axes passed. No push,
tag, release, issue import, `.scratch` cleanup, or branch deletion occurred.
