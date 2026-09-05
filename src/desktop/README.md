# Desktop integration

`io.roonscape.Renderer` matches the Renderer application ID, icon name, and
installed desktop-entry filename. The Renderer adds `icons/` to its icon
search path so the window icon is available from a built checkout, even
before per-user installation.

`io.roonscape.Renderer.desktop.in` is completed by
[`scripts/install-desktop.mjs`](../../scripts/install-desktop.mjs). The
installer records absolute paths in a per-user launch helper, using the Node
executable that ran the installer. No shell profile is required at launch.
Setup must already be complete: the entry uses `Terminal=false`.

The icon is derived from the approved
[`icon.svg`](../../assets/icon.svg). Regenerate it with
`node scripts/export-branding.mjs`; do not edit the derived paths manually.

See [development](../../docs/development.md#desktop-launcher) for local
installation and [getting started](../../docs/getting-started.md#add-an-application-menu-entry)
for installation from a future packaged archive.
