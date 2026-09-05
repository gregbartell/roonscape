# Branding assets

These graphics derive from the approved
[RoonScape SVG](../../src/renderer/assets/roonscape.svg). The source geometry
is preserved in every export.

| Asset                                                   | Intended use                                                                                |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `roonscape-black-256.png` / `roonscape-white-256.png`   | Transparent icons for the README and small graphics, on light/dark backgrounds respectively |
| `roonscape-black-1024.png` / `roonscape-white-1024.png` | Transparent icons for larger compositions                                                   |
| `roonscape-avatar-512.png`                              | Square mark on an opaque pale blue background, including circular crops                     |
| `roonscape-share-1280x640.png`                          | Repository social preview, project announcements, or future release posts                   |

The share card contains no version or release claim. Its 1280×640 size follows
[GitHub's social preview guidance](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/customizing-your-repositorys-social-media-preview).
The PNG is ready to upload under the repository's **Settings → Social preview**;
committing it does not change the repository setting automatically.

The share card uses the bundled Libre Baskerville and IBM Plex Sans fonts.
The opaque graphics use pale blue `#e4edf3`, navy `#203443`, and muted text
`#526878`. The transparent exports are pure black or white.

To regenerate all six PNGs and the derived desktop SVG, install
`rsvg-convert` (librsvg), then run:

```sh
node scripts/export-branding.mjs
```

The script reads the canonical SVG and bundled fonts directly; it does not
install fonts, build application binaries, change versions, or publish files.
The desktop SVG is written to
`src/desktop/icons/hicolor/scalable/apps/io.roonscape.Renderer.svg`.
