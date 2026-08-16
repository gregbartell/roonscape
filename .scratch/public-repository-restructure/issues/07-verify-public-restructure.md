# 07 — Verify the public repository restructure

**What to build:** Establish one complete, evidence-backed acceptance result
for the reorganized and generalized repository before its issue tracker changes.
Close integration gaps across build, runtime, packaging, presentation, and
documentation while preserving the owner's local tracker and publication
control.

**Blocked by:** 06 — Repair the public README and wayfinding.

**Status:** ready-for-agent

- [ ] The complete repository check command passes, including formatting,
      TypeScript and Rust checks, linting, bridge and renderer tests, script
      tests, packaging tests, and the IPC smoke exercise.
- [ ] Source-mode setup, ordinary launch discovery, Fixture Mode, visual-
      capture planning, and release packaging resolve every relocated module,
      shared asset, and runtime binary from the intended structure.
- [ ] A human visual review covers 1280x720, 1600x1200, 1920x1200, 2560x1080,
      and 3840x2160 and records that composition, hierarchy, artwork,
      identities, full-field states, diagnostics, and OLED-safe geometry remain
      intentional.
- [ ] Current public source and durable documentation outside `.scratch`
      contain no Reference Deployment, Intel NUC, personal Gmail, obsolete
      Gallery design vocabulary, current-tree prototype reference, or dead
      local-ticket link.
- [ ] Documentation and README links resolve, and every documented maintenance
      command names an existing command after the restructure.
- [ ] Excluding the owner-retained `.scratch` tracker and conventional ignored
      generated output, tracked top-level directories are limited to GitHub
      metadata, documentation, maintainer scripts, and product source.
- [ ] The MIT license and all manifest license metadata agree, and no excluded
      contribution, security, or community document has appeared.
- [ ] Dependencies, toolchains, and GitHub Action versions are unchanged except
      for lockfile path metadata mechanically required by the source move.
- [ ] Git history and prototype branches are unchanged, and no commit, push,
      tag, release, or branch deletion occurs.
- [ ] `.scratch` remains present with the local spec and tickets; no content
      outside normal ticket lifecycle updates is deleted, moved, migrated, or
      rewritten.
