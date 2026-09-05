## Agent skills

### Issue tracker

Issues and specs are tracked as GitHub issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Triage state uses the default canonical label vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository. Before exploring the codebase, read
[`CONTEXT.md`](CONTEXT.md) and relevant decisions in [`docs/adr/`](docs/adr/).
Use the glossary's canonical terms in code, tests, docs, and explanations.
Surface conflicts with existing ADRs explicitly. Use `domain-modeling` when
resolving vocabulary gaps or recording domain decisions; create domain docs
only when there are resolved terms or decisions to record.

### Development preparation

Before preparing a worktree or diagnosing verification prerequisites, read
[Development](docs/development.md) for commands, capability levels, and explicit
host provisioning and agent permissions.

### Verification

Before choosing focused checks or running final verification, read
[Verification policy](docs/agents/verification.md) for required commands,
retained evidence, and the helper-test versus Live Capture Session distinction.
