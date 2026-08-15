# 03 — Ship preferred and fallback typography

**What to build:** Give RoonScape the selected editorial typography without
depending on arbitrary host substitutions: use Palatino Linotype and Segoe UI
as one preferred pair when both are present, otherwise use the complete open
Libre Baskerville and IBM Plex Sans pair.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Title and Album use Palatino Linotype while Artist and utility text use
      Segoe UI when both preferred faces are installed and resolvable.
- [ ] If either preferred face is unavailable, Title and Album use Libre
      Baskerville and Artist and utility text use IBM Plex Sans; preferred and
      fallback faces are never mixed into a partial pair.
- [ ] The open fallback fonts and required license notices ship with the
      supported deployment and load without network access.
- [ ] Palatino Linotype and Segoe UI remain host-provided and are not copied,
      downloaded, or redistributed by RoonScape.
- [ ] Characters outside the selected faces' coverage receive readable Pango
      glyph substitution instead of missing-glyph boxes.
- [ ] Automated checks can force preferred-present, partially present, and
      preferred-absent conditions and verify deterministic atomic selection;
      representative manual captures cover both complete pairs.
