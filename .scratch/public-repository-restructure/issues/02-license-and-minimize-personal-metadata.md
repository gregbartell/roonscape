# 02 — License RoonScape and minimize personal metadata

**What to build:** Give prospective owners unambiguous permission to use and
modify RoonScape while preserving accountable project authorship without
publishing a personal contact address or inventing community processes that do
not yet exist.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] The repository carries the standard MIT license with the project owner's
      current name and copyright year.
- [x] Both private npm manifests declare MIT in machine-readable metadata.
- [x] The Rust renderer package declares MIT and the canonical GitHub repository
      URL in machine-readable metadata.
- [x] The Roon extension continues to identify Gregory Bartell as publisher and
      retains the canonical repository URL.
- [x] The Roon extension uses
      `5353310+gregbartell@users.noreply.github.com` instead of the personal
      Gmail address, and metadata tests expect the privacy-safe identity.
- [x] No personal Gmail address remains in current public source or durable
      documentation outside the owner-retained local tracker history.
- [x] No contribution guide, security policy, Code of Conduct, CLA, DCO, or
      other community-governance document is added.
- [x] Existing package and metadata tests pass without changing runtime
      behavior.
- [x] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.

## Comments

### Implementation Result — 2026-08-16

Implemented in this change. The repository now carries the standard MIT license;
both private npm manifests and the renderer package expose matching
machine-readable license metadata; and the renderer also exposes the canonical
repository URL. The Roon extension retains Gregory Bartell and the repository
URL while publishing the GitHub noreply address, with integration coverage for
all three fields.

`npm run check` passed with formatting, TypeScript and Rust type checking,
linting, package assembly, the complete Node and Rust test suites, and the IPC
restart smoke exercise. A tracked-tree scan found no remaining personal Gmail
address outside `.scratch`, and no community-governance document was added.
