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

## Presentation Captures

Presentation Captures use the same deterministic static Fixture Mode through
the native renderer. The capture host additionally needs `Xvfb`, `xwininfo`,
and `scrot` on `PATH`.

Discover the maintained Fixture Scenario identifiers and labels without
launching the renderer:

```sh
npm run capture:presentations -- --list-scenarios
```

Capture one Fixture Scenario at the default 3840×2160 resolution:

```sh
npm run capture:presentations -- --scenario playing
```

An ordinary capture writes directly to the current directory unless
`--output` names another directory; a missing output directory is created.
Repeat `--resolution WIDTHxHEIGHT` to capture every requested landscape
resolution. `--artwork /path/to/image` substitutes a maintainer-supplied image
when the selected Fixture Scenario supports custom artwork. Use `--all` to
capture the 17 maintained nonduplicate Fixture Scenarios, with custom artwork
applied to the nine compatible scenarios when requested.

```sh
npm run capture:presentations -- --all --artwork /path/to/image --resolution 1920x1080 --output /path/to/captures
```

Existing final paths stop capture during preflight. Add `--overwrite` only
when replacing those paths is intentional. Each validated PNG receives its
final filename and is printed to standard output as soon as it is ready;
progress and diagnostics use standard error. A later failure exits nonzero,
keeps completed Presentation Captures, lists their paths, and explicitly marks
the planned set incomplete. An overwrite failure may therefore leave a mixture
of refreshed and earlier captures.

The comprehensive design-review plan is available only through the explicit
visual-acceptance profile described in the [presentation visual-acceptance
guide](visual-acceptance/presentation.md).

Run the complete repository check before submitting a change:

```sh
npm run check
```

For release work, follow the [Releasing guide](releasing.md).
