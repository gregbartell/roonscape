# 07 — Verify the public repository restructure

**What to build:** Establish one complete, evidence-backed acceptance result
for the reorganized and generalized repository before its issue tracker changes.
Close integration gaps across build, runtime, packaging, presentation, and
documentation while preserving the owner's local tracker and publication
control.

**Blocked by:** 06 — Repair the public README and wayfinding.

**Status:** done

- [x] The complete repository check command passes, including formatting,
      TypeScript and Rust checks, linting, bridge and renderer tests, script
      tests, packaging tests, and the IPC smoke exercise.
- [x] Source-mode setup, ordinary launch discovery, Fixture Mode, visual-
      capture planning, and release packaging resolve every relocated module,
      shared asset, and runtime binary from the intended structure.
- [x] A human visual review covers 1280x720, 1600x1200, 1920x1200, 2560x1080,
      and 3840x2160 and records that composition, hierarchy, artwork,
      identities, full-field states, diagnostics, and OLED-safe geometry remain
      intentional.
- [x] Current public source and durable documentation outside `.scratch`
      contain no Reference Deployment, Intel NUC, personal Gmail, obsolete
      Gallery design vocabulary, current-tree prototype reference, or dead
      local-ticket link.
- [x] Documentation and README links resolve, and every documented maintenance
      command names an existing command after the restructure.
- [x] Excluding the owner-retained `.scratch` tracker and conventional ignored
      generated output, tracked top-level directories are limited to GitHub
      metadata, documentation, maintainer scripts, and product source.
- [x] The MIT license and all manifest license metadata agree, and no excluded
      contribution, security, or community document has appeared.
- [x] Dependencies, toolchains, and GitHub Action versions are unchanged except
      for lockfile path metadata mechanically required by the source move.
- [x] Git history and prototype branches are unchanged, and no commit, push,
      tag, release, or branch deletion occurs.
- [x] `.scratch` remains present with the local spec and tickets; no content
      outside normal ticket lifecycle updates is deleted, moved, migrated, or
      rewritten.

## Comments

### Implementation Result — 2026-08-16

Completed one repository-wide acceptance pass and closed three integration
gaps found by exercising the real native capture path. Presentation captures
now provide temporary Display Configuration through the renderer's supported
`--config PATH` interface instead of the removed environment override. The
renderer keeps RoonScape's arguments out of GTK's option parser, so an ordinary
launcher-provided configuration is accepted at runtime. High-resolution
full-field copy also reserves enough measure to keep maintained heading words
intact rather than splitting `RoonScape` or `unavailable` at 3840x2160.

The final `npm run check` passed formatting, TypeScript and Rust checks,
linting, all bridge, renderer, launcher and capture tests, real release
packaging and relocation, and the IPC startup/restart/replay smoke exercise.
Focused red-green coverage verifies the capture command's renderer arguments
and the complete-heading-word layout invariant.

The final native GTK 4/Pango review artifact at
`/tmp/codex/roonscape/presentation-captures.qaSYNp` contains 115 disposable PNG
captures: all 18 maintained Fixture Scenarios plus two typography and three
diagnostics representatives at each of 1280x720, 1600x1200, 1920x1200,
2560x1080, and 3840x2160. Review recorded pass for composition and negative
space, hierarchy and bounded metadata, complete and contained artwork, dark,
light, and fixed-no-art palettes, stable identities, distinct full-field
states, preferred and fallback typography with readable glyph fallback,
bounded diagnostics, and the reserved OLED-safe field at every viewport.

Static audits resolved all local Markdown links in the 11 tracked public
documents, matched every documented npm maintenance command to the root
manifest, and received HTTP 200 from the repository Releases destination.
Public product source and durable product documentation contain none of the
removed personal-deployment, Gallery, or current-tree prototype references;
the remaining `.scratch` references are the still-active local tracker
instructions rather than dead ticket links. Tracked top-level directories are
limited to `.github`, `.scratch`, `docs`, `scripts`, and `src`. MIT metadata
agrees across the license and manifests, no excluded community-policy document
exists, and dependency, toolchain, lock, and GitHub Action version review found
only the intended workspace-path and license metadata changes.

The final Standards and Spec reviews reported no findings. Git history, tags,
and both local and remote `prototype/visual-variants` refs remained unchanged;
no commit, push, tag, release, or branch deletion occurred. No `.scratch`
content outside this ticket's lifecycle fields and implementation comment was
changed.
