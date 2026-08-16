# 01 — Consolidate product modules under the source root

**What to build:** Give maintainers a compact, legible repository root by
grouping the Roon bridge, native renderer, launcher, schemas, and Fixture
Scenarios under the agreed source root while preserving every owner-facing and
developer-facing workflow. The move is an internal clean break: no compatibility
aliases for the old source or archive paths are required.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] The source root contains distinct bridge, renderer, launcher, and shared
      contract modules without merging their responsibilities.
- [x] The shared contract contains the language-neutral schemas and Fixture
      Scenarios consumed by both runtime modules.
- [x] GitHub metadata, documentation, and maintainer scripts remain directly
      discoverable at the repository root; root manifests, locks, agent
      instructions, formatting configuration, and toolchain pins remain
      conventional root entrypoints.
- [x] The former top-level bridge, renderer, schema, and fixtures directories
      no longer remain as tracked compatibility copies or aliases.
- [x] npm resolves the relocated bridge workspace and Cargo resolves the
      relocated renderer workspace from their root manifests and existing
      lockfiles.
- [x] Source-mode setup, ordinary launch, Fixture Mode, visual-capture planning,
      IPC smoke exercise, and focused bridge workflows resolve the relocated
      launcher, runtime modules, schemas, and Fixture Scenarios.
- [x] Release packaging still builds a runnable archive containing the bridge,
      renderer, shared contract, runtime assets, and launcher; its unpublished
      internal layout may change.
- [x] Owner-facing CLI behavior, Display Configuration, Roon Authorization,
      XDG storage, and presentation-snapshot semantics are unchanged.
- [x] Conventional ignored npm, Cargo, and release outputs retain their normal
      generated locations and are not treated as tracked source.
- [x] The complete repository check command passes from the reorganized root.
- [x] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.

## Comments

### Implementation Result — 2026-08-16

Implemented in `e658200`. The bridge, renderer, launcher, schemas, and Fixture
Scenarios now live under `src/` as distinct modules; root npm and Cargo
manifests resolve the relocated workspaces; source, Fixture Mode, capture,
packaging, and IPC workflows use the new paths; and release archives include
the complete shared contract without changing owner-facing behavior.

`npm run check` passed with formatting, TypeScript and Rust type checking,
linting, the bridge and renderer suites, source and Fixture Mode launcher
coverage, visual-capture planning, relocatable packaging, and the IPC restart
smoke exercise. The final two-axis review reported no hard Standards
violations and no Spec findings. Standards retained one non-blocking
judgement-call observation that source layout necessarily coordinates paths
across the shell, TypeScript, Rust, JSON, tests, and packaging.
