# 06 — Repair the public README and wayfinding

**What to build:** Keep the current README truthful and navigable after the
repository restructure without performing the separately planned owner-facing
rewrite. Remove obsolete private context and dead wayfinding, update the names
owners and maintainers actually invoke, and leave future installation and
getting-started improvements to their own effort.

**Blocked by:** 02 — License RoonScape and minimize personal metadata; 05 —
Publish self-contained presentation guidance.

**Status:** done

- [x] The README contains no Reference Deployment, Intel NUC, personal-host
      launch policy, Gallery design name, current-tree prototype link, or link
      to local specs and tickets.
- [x] Links to the domain context, architecture decision, presentation design,
      visual acceptance, and MIT license resolve to their current locations.
- [x] Source-development commands and prose use the relocated modules and
      renamed visual-capture workflow accurately.
- [x] Existing owner-facing CLI, setup, Fixture Mode, runtime, diagnostics, and
      release descriptions remain behaviorally accurate after mechanical path
      and terminology changes.
- [x] The README does not claim that one display resolution or the former
      personal installation defines product compatibility.
- [x] The README does not receive the separately planned screenshot layout,
      stable latest-release installation flow, full getting-started guide, or
      `.xinitrc` example.
- [x] No new contribution, security, community-governance, or support-response
      commitment is introduced.
- [x] All retained README links and command names are verified.
- [x] Formatting and relevant documentation checks pass.
- [x] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.

## Comments

### Implementation Result — 2026-08-16

Repaired the README without expanding it into the separately planned owner
rewrite. The public overview now describes fluid support for common landscape
displays without treating a personal host or resolution as canonical. Removed
the obsolete private deployment and guarded-tty launch policy, replaced local
tracker and directory-level architecture links with current durable targets,
added direct visual-acceptance and MIT-license wayfinding, and named the
`capture:presentations` source workflow.

All retained local documentation links exist, the canonical GitHub Releases
destination returned HTTP 200, every documented npm command is present in the
root manifest, and the launcher reported the documented public flags and
version behavior. The README formatting check and complete `npm run check`
suite passed, including formatting, type checking, linting, bridge and renderer
tests, launcher and capture tests, release packaging, and the IPC restart smoke
exercise. No `.scratch` content outside this ticket's lifecycle fields and
implementation comment changed.
