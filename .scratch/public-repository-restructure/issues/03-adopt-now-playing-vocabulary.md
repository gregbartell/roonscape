# 03 — Adopt Now Playing vocabulary and neutral fixtures

**What to build:** Make production vocabulary agree with RoonScape's domain
language by replacing the prototype-derived Gallery design name with Now
Playing or presentation terminology and replacing deployment-derived fixture
identities with an unmistakably audio-oriented Speaker System in the Living
Room. Preserve the behavior exercised through the renamed interfaces.

**Blocked by:** 01 — Consolidate product modules under the source root.

**Status:** ready-for-agent

- [ ] The split layout for Now Playing content uses Now Playing terminology in
      renderer types, functions, fields, styles, diagnostics, and tests.
- [ ] Capture commands, planners, output messages, temporary artifact names,
      and script tests use presentation or visual terminology rather than the
      Gallery design name.
- [ ] No compatibility aliases retain the obsolete Gallery implementation
      vocabulary after all internal callers are migrated.
- [ ] Representative fixtures and tests use Speaker System as the Tracked
      Output and Living Room as the Tracked Zone, with corresponding neutral
      output and zone identifiers.
- [ ] Long-identity Fixture Scenarios retain their overflow-testing value
      without referring to a gallery wall or the former visual direction.
- [ ] Neutral sample identities still prove that the Tracked Output is a Roon
      audio endpoint and that the Tracked Zone may change around it.
- [ ] Shared snapshots remain schema-compatible and continue to produce the
      same viewer-facing presentations, progress, transitions, palettes, and
      availability behavior.
- [ ] Production source, maintainer scripts, shared contract assets, and tests
      contain no remaining Gallery design vocabulary or `NUC HDMI` sample
      identity; durable documentation and the current-tree prototype are left
      for their dedicated ticket.
- [ ] Relevant bridge, renderer, script, contract, and formatting checks pass.
- [ ] No `.scratch` content outside this ticket's normal status and comments is
      deleted, moved, migrated, or rewritten.
