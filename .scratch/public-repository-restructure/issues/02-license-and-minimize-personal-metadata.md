# 02 — License RoonScape and minimize personal metadata

**What to build:** Give prospective owners unambiguous permission to use and
modify RoonScape while preserving accountable project authorship without
publishing a personal contact address or inventing community processes that do
not yet exist.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] The repository carries the standard MIT license with the project owner's
      current name and copyright year.
- [ ] Both private npm manifests declare MIT in machine-readable metadata.
- [ ] The Rust renderer package declares MIT and the canonical GitHub repository
      URL in machine-readable metadata.
- [ ] The Roon extension continues to identify Gregory Bartell as publisher and
      retains the canonical repository URL.
- [ ] The Roon extension uses
      `5353310+gregbartell@users.noreply.github.com` instead of the personal
      Gmail address, and metadata tests expect the privacy-safe identity.
- [ ] No personal Gmail address remains in current public source or durable
      documentation outside the owner-retained local tracker history.
- [ ] No contribution guide, security policy, Code of Conduct, CLA, DCO, or
      other community-governance document is added.
- [ ] Existing package and metadata tests pass without changing runtime
      behavior.
- [ ] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.
