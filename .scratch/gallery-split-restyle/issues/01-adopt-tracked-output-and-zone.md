# 01 — Adopt Tracked Output and Tracked Zone

**What to build:** Replace the ambiguous Display terminology with the
canonical Tracked Output and Tracked Zone model across configuration, live Roon
tracking, complete presentation snapshots, fixture data, and viewer-facing
identity output. This is an intentional clean break: the removed contract is
rejected rather than migrated.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Display Configuration requires `trackedOutputId`, increments the
      relevant contract version, and rejects `displayOutputId` and unexpected
      legacy fields.
- [ ] Available presentation snapshots require authoritative Tracked Output
      and Tracked Zone names, while pairing-required, disconnected, and
      output-unavailable snapshots carry neither identity.
- [ ] The bridge follows the configured Tracked Output through grouping and
      ungrouping, retains its identity, and publishes the current containing
      Tracked Zone without following unrelated active zones.
- [ ] Configuration discovery, commands, status text, diagnostics, and errors
      use Tracked Output and Tracked Zone terminology consistently without
      introducing Roon Control.
- [ ] The renderer consumes the new contract and presents the authoritative
      names under the concise labels Output and Zone.
- [ ] TypeScript, Rust, schema, fixture, and integration checks cover the new
      vocabulary, grouping changes, removal of the selected output, and
      explicit rejection of the removed fields.
