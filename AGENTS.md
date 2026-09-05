## Agent skills

### Issue tracker

Issues and specs are tracked as GitHub issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Triage state uses the default canonical label vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

For domain context, read
[`CONTEXT.md`](CONTEXT.md) and relevant decisions in [`docs/adr/`](docs/adr/).
Use the glossary's canonical terms in code, tests, docs, and explanations.
Surface conflicts with existing ADRs explicitly. Use `domain-modeling` when
resolving vocabulary gaps or recording domain decisions; create domain docs
only when there are resolved terms or decisions to record.

### Development preparation

Before preparing a worktree or diagnosing verification prerequisites, read
the [preparation section](docs/development.md#prepare-an-existing-worktree).
For missing tools, fonts, or execution permissions, read the relevant parts of
[host provisioning](docs/development.md#provision-the-development-host-explicitly).

### Verification

Before choosing focused checks or final verification, read
[Choose checks](docs/agents/verification.md#choose-checks). Follow its pointers
to execution, evidence, and presentation review only when applicable to the task.

### Durable repository content

Keep committed code, tests, and documentation focused on the current product.
Do not retain completed implementation specs, disposable prototypes, one-off
verification narratives, or historical benchmark results. Never commit
references to particular personal or development hosts; use reserved example
hostnames and addresses in examples and tests. Keep run evidence outside the
repository. Preserve protocol knowledge with explicit confidence and limits,
rather than the chronology of its collection.

RoonScape has no backwards-compatibility requirement. Maintain only the current
configuration contract; do not add legacy accommodations, migration-only
behavior, or tests specifying how obsolete configurations or options behave.
