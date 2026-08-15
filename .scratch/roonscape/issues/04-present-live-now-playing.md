# 04 — Present live Now Playing and artwork

**What to build:** Carry the configured Display Zone's live Now Playing values
and artwork from Roon into Gallery split. The listener sees expressive,
readable artwork-derived colors for ordinary content and a calm, intentional
neutral presentation when artwork is missing.

**Blocked by:** 03 — Select and follow the Display Output.

**Status:** ready-for-agent

- [ ] Roon's first, second, and third prepared display lines map positionally
  to optional Title, Artist, and Album values without invented fallbacks.
- [ ] The bridge obtains current artwork through Roon's supported Image service
  and passes compressed artwork through atomically replaced local files rather
  than state JSON or a network service.
- [ ] Artwork identity follows presentation revision rather than assuming
  Roon's opaque image key is a stable track identity.
- [ ] Gallery split makes artwork dominant, gives metadata a dedicated column,
  emphasizes Title, and shows Artist, Album, Zone, and playback state without
  persistent RoonScape branding.
- [ ] The renderer derives the full presentation palette, including readable
  text, from current artwork and uses a deliberate neutral palette when
  artwork is absent.
- [ ] Automated fixtures cover ordinary metadata, missing artwork, artwork
  revision changes, and Roon display-line mapping.
- [ ] Completed artwork is bounded to the current presentation and obsolete
  files are not retained as history.
