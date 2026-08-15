# RoonScape Gallery split font study

> **PROTOTYPE / THROWAWAY.** This study answers one question: which
> glyph-complete open serif/sans pairing preserves the editorial quality of the
> original Gallery split typography?

Five typography variants share the same route, composition, fixture data,
palette, and state treatments. Only each pair's type families and the optical
adjustments needed to make that pair perform well change.

Run from the repository root:

```sh
python3 -m http.server 4174 --directory prototype/gallery-split-font-study
```

Open <http://localhost:4174/?variant=A&state=playing>.

## Variants

- `A` — **Selected preferred:** Palatino Linotype + Segoe UI
- `B` — **Reading-room:** Newsreader + Inter
- `C` — **Source editorial:** Source Serif 4 + Source Sans 3
- `D` — **Selected fallback:** Libre Baskerville + IBM Plex Sans
- `E` — **Lyrical display:** Cormorant Garamond + Work Sans

Variant A uses the system fonts that rendered the original prototype on the
development host. Variants B–E load from [Google Fonts][google-fonts], whose
catalog is distributed under open-source licenses.

The bottom bar is not part of RoonScape. Click its arrows or use Left/Right to
change font pairings. Click **State** or use Up/Down to cycle Playing, Paused,
Loading with metadata, long metadata, Idle, empty Loading, missing artwork,
missing details, Pairing required, Disconnected, and Tracked Output
unavailable. Click **Inspect** or press `I` to see the complete font and fixture
state. The URL records both selections for sharing.

The recorded production decision is Variant A as the preferred host-provided
pair, with Variant D as the packaged open fallback. Pair selection is atomic:
if either preferred face is unavailable, both roles use Variant D.

[google-fonts]: https://developers.google.com/fonts
