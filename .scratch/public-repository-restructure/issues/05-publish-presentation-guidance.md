# 05 — Publish self-contained presentation guidance

**What to build:** Give maintainers durable, self-contained guidance for
RoonScape's current presentation design and visual-review workflow while
removing obsolete prototype history, personal-deployment authority, and local
ticket dependencies from public documentation. Remove the now-redundant
current-tree prototype without touching its separate branch.

**Blocked by:** 04 — Support representative landscape viewports.

**Status:** done

- [x] The runnable font-study prototype is absent from the current tree, and no
      local or remote prototype branch is changed or deleted.
- [x] A presentation-design document records only current visual intent:
      asymmetric artwork and metadata composition, hierarchy, artwork fitting,
      palette behavior, typography, restrained motion, full-field states, and
      stable Output and Zone placement.
- [x] A separate visual-acceptance document explains capture generation,
      focused review, the complete Fixture Scenario matrix, representative
      landscape viewports, typography and diagnostics review, and the
      non-golden nature of captures.
- [x] Both documents use Now Playing and presentation vocabulary and contain no
      Gallery design name, prototype dependency, local-ticket link, Reference
      Deployment, Intel NUC assumption, or physical-host handoff.
- [x] OLED-safe inactivity behavior remains documented as a product capability
      without preserving physical calibration work for the former personal
      deployment.
- [x] The domain context describes RoonScape without a personal-project
      qualifier and introduces no replacement term for the removed Reference
      Deployment or Gallery design name.
- [x] One concise ADR records the tradeoff behind the separate Node.js Roon
      bridge and native renderer; obsolete product-naming and implementation-
      ledger ADRs are absent.
- [x] Documentation links and renamed visual commands within the retained
      guidance resolve correctly.
- [x] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.

## Comments

### Implementation Result — 2026-08-16

Published self-contained current guidance in `docs/design/presentation.md` and
`docs/visual-acceptance/presentation.md`. The design guide records the
responsive Now Playing composition, hierarchy, artwork, palette, typography,
motion, full-field state, identity, and OLED-safe inactivity intent. The
visual-acceptance guide documents current capture commands, all 18 maintained
Fixture Scenarios at five peer landscape viewports, focused selection,
typography and diagnostics representatives, review criteria, and the
non-golden status of capture artifacts.

Removed the runnable font-study prototype from the current tree and normalized
the retained fixture artwork and palette-test wording to current presentation
language. The local and remote `prototype/visual-variants` refs remain at
`7f4dc62521a59f0799aeabe4f2cf0e580dc1b16d` and were not modified. The existing
domain context and consolidated bridge/renderer ADR already satisfy this
ticket's domain and architecture requirements.

`npm run check` passed the complete formatting, typecheck, lint, bridge,
renderer, script, packaging, and IPC smoke suite. The focused presentation-
capture tests, renderer palette tests, retained-guidance link check, forbidden-
term scan, and Git branch/ref audit also passed. No `.scratch` content outside
this ticket's lifecycle fields and implementation comment changed.
