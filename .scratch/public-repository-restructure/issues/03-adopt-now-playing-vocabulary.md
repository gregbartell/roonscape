# 03 — Adopt Now Playing vocabulary and neutral fixtures

**What to build:** Make production vocabulary agree with RoonScape's domain
language by replacing the prototype-derived Gallery design name with Now
Playing or presentation terminology and replacing deployment-derived fixture
identities with an unmistakably audio-oriented Speaker System in the Living
Room. Preserve the behavior exercised through the renamed interfaces.

**Blocked by:** 01 — Consolidate product modules under the source root.

**Status:** done

- [x] The split layout for Now Playing content uses Now Playing terminology in
      renderer types, functions, fields, styles, diagnostics, and tests.
- [x] Capture commands, planners, output messages, temporary artifact names,
      and script tests use presentation or visual terminology rather than the
      Gallery design name.
- [x] No compatibility aliases retain the obsolete Gallery implementation
      vocabulary after all internal callers are migrated.
- [x] Representative fixtures and tests use Speaker System as the Tracked
      Output and Living Room as the Tracked Zone, with corresponding neutral
      output and zone identifiers.
- [x] Long-identity Fixture Scenarios retain their overflow-testing value
      without referring to a gallery wall or the former visual direction.
- [x] Neutral sample identities still prove that the Tracked Output is a Roon
      audio endpoint and that the Tracked Zone may change around it.
- [x] Shared snapshots remain schema-compatible and continue to produce the
      same viewer-facing presentations, progress, transitions, palettes, and
      availability behavior.
- [x] Production source, maintainer scripts, shared contract assets, and tests
      contain no remaining Gallery design vocabulary or `NUC HDMI` sample
      identity; durable documentation and the current-tree prototype are left
      for their dedicated ticket.
- [x] Relevant bridge, renderer, script, contract, and formatting checks pass.
- [x] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.

## Comments

### Implementation Result — 2026-08-16

Implemented in this change. The native renderer now names the split content
seam with Now Playing types, functions, fields, styles, diagnostics, and tests.
The visual-acceptance capture command, planner, messages, and temporary
artifacts use presentation terminology without compatibility aliases for the
former names.

Shared fixtures and bridge/renderer tests now use Speaker System in Living
Room with neutral output and zone identifiers. Grouping coverage continues to
show that the same Tracked Output can move into Whole Home, and the long
identity Fixture Scenario keeps its overflow pressure with an explicitly audio
endpoint name.

`npm test`, `npm run format:check`, and `npm run lint` passed, covering the
bridge and renderer suites, presentation capture planning and orchestration,
source and Fixture Mode launchers, release packaging, snapshot contracts, and
the IPC restart smoke exercise. A current-code scan found no remaining Gallery
design vocabulary, `NUC HDMI`, or `AudioDevice` outside the deferred durable
documentation, prototype, and local tracker history.
