# Development

RoonScape source development requires Node.js and npm, Rust and Cargo,
`pkg-config`, and the GTK 4 development files. Use the versions pinned by
`.node-version`, `package.json`, and `rust-toolchain.toml` rather than versions
copied into documentation.

From a fresh checkout, install the locked JavaScript dependencies:

```sh
npm ci
```

Build and launch RoonScape in Live Mode:

```sh
npm start
```

For presentation work, launch Fixture Mode:

```sh
npm run fixture
```

With the renderer focused, use Left and Right to move between Fixture
Scenarios. Set `ROONSCAPE_WINDOWED=1` when a window is more convenient than
fullscreen.

Launch static Fixture Mode when a deterministic presentation is useful for
inspection or capture:

```sh
npm run fixture -- --static
```

Left and Right continue to navigate the Fixture Scenarios, but Playing
progress remains at its recorded source position and presentation motion,
inactivity treatment, and crossfades are disabled. Ordinary Fixture Mode and
Live Mode retain their normal behavior.

Run the complete repository check before submitting a change:

```sh
npm run check
```

For presentation capture and review, follow the [presentation visual-acceptance
guide](visual-acceptance/presentation.md). For release work, follow the
[Releasing guide](releasing.md).
