# 05 — Publish and document the release workflow

**What to build:** Once the owner chooses the unattended deployment approach,
publish repeatable release artifacts and document one coherent owner,
developer, and RoonScape Host lifecycle around the foreground `roonscape`
command.

**Blocked by:** 04 — Build and verify a relocatable Linux release.

**Status:** needs-triage

- [ ] The owner first decides how the RoonScape Host starts and supervises the
      foreground `roonscape` command, and this ticket is moved to the appropriate
      ready state only after that unresolved deployment choice is recorded.
- [ ] Tag-triggered automation invokes the repository's packaging command and
      publishes its versioned archive and checksum without duplicating staging
      or packaging logic in the workflow.
- [ ] Owner documentation covers download, extraction, first-time setup, normal
      launch, explicit reconfiguration, help, version identification, updates by
      directory replacement, and the GTK runtime requirement.
- [ ] Source documentation preserves focused bridge and fixture workflows while
      making `npm run setup`, `npm start`, and `npm run package` the ordinary
      setup, launch, and packaging paths.
- [ ] The product and unattended-deployment requirements are reconciled with the
      chosen boot integration and the launcher-managed runtime session rather
      than retaining independent bridge and renderer supervision.
- [ ] Compatible-host release verification is incorporated into the existing
      unattended-deployment verification ticket instead of creating a second
      human acceptance ticket for this effort.
- [ ] Documentation distinguishes the supported initial x86-64 glibc and GTK
      baseline from unverified platforms and does not imply an installer,
      automatic updater, or broad distribution guarantee.
