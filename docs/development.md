# Development

RoonScape source development requires Node.js and npm, Rust and Cargo,
`pkg-config`, the GTK 4 development files, and FFmpeg/FFprobe for synthetic
Live Capture Session helper tests. Use the versions pinned by
`.node-version`, `package.json`, and `rust-toolchain.toml` rather than versions
copied into documentation.

## Prepare an existing worktree

After the explicit host provisioning below, run these from your worktree:

```sh
npm run dev:diagnose
npm run dev:prepare
```

Both commands use Node's standard library and work before `node_modules`
exists. If npm is unavailable, run `node scripts/development-environment.mjs
diagnose` directly. If Node itself is unavailable, select `.node-version`
first; no JavaScript diagnostic can run without Node.

`dev:diagnose` is read-only. It probes the pinned Node/npm/Rust versions,
formatting/lint tools, C compiler, native prerequisites, packaged fonts, host
font families, and filesystem access. Host font inspection uses Python 3's
standard library with Fontconfig to query font files in memory, without
creating font caches. No Python packages are required. It never starts RoonScape or accesses
personal Display Configuration or Roon Authorization. The reported capabilities
describe host prerequisites, not a completed build or successful verification:

- **Automated checks:** the required toolchain, native tools, packaged fonts,
  and runtime/build locations.
- **Packaged-fallback Presentation Captures:** automated prerequisites plus a
  usable evidence destination.
- **Complete typography/profile:** capture prerequisites plus Sitka Display,
  Palatino Linotype, Segoe UI, and a font providing `月` for glyph fallback.

Missing proprietary host fonts limits the last capability; it does not block
preparation, automated checks, or fallback captures. Exit status is 0 when the
first two capabilities are ready and 1 otherwise. The complete typography
status is reported separately, so read it before requesting the full profile.
The diagnostic can check another evidence destination without creating it:

```sh
npm run dev:diagnose -- --evidence /var/tmp/codex/roonscape/review
```

`dev:prepare` runs the same preflight, then `npm ci` with lifecycle scripts
disabled and development dependencies included, followed by `cargo fetch
--locked`. Network and dependency-cache access are required. It stops on a
failure, prints remediation, and leaves any completed downloads available for
a retry. It does not compile the application. Source lockfiles remain unchanged;
an inconsistent lockfile must be fixed as a separate source change. It neither
creates worktrees nor selects branches. Build/dependency directories must be
local, not symlinks; Cargo output paths are set to this worktree's `target`
during preparation. Shared npm/Cargo download caches are supported. Before
later verification, unset `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`, and
`CARGO_BUILD_BUILD_DIR` overrides and remove external target/build-directory
settings from Cargo configuration so builds and capture executables stay local.

Application setup remains `npm run setup`. Development preparation does not
run it, discover Roon Servers, or request Roon Authorization.

## Provision the development host explicitly

Provisioning instructions cover Arch Linux and Ubuntu 22.04, which CI uses.
Native packages have one maintained source in
[`scripts/development-host-packages.json`](../scripts/development-host-packages.json),
also consumed by the shared CI native-environment action. Provision them
explicitly as an administrator; neither development command runs a package
manager for system packages. With Node already available, these commands are
examples for a Bash shell:

```bash
# Ubuntu 22.04 (the same package list used by CI)
mapfile -t packages < <(node -p 'require("./scripts/development-host-packages.json").ubuntu.join("\n")')
sudo apt-get update
sudo apt-get install --yes "${packages[@]}"

# Arch Linux: update the system, then install its package list
mapfile -t packages < <(node -p 'require("./scripts/development-host-packages.json").arch.join("\n")')
sudo pacman -Syu --needed "${packages[@]}"
```

Select the Node version from `.node-version` with your Node manager;
CI uses `actions/setup-node` with `node-version-file`. Explicitly
install/select npm at the version in `package.json`'s `packageManager` field.
Use the Rust release in `rust-toolchain.toml`, including `rustfmt` and Clippy.
Arch's distribution Rust packages are supported when they match that pin. For
a host using rustup (including Ubuntu CI), provision the toolchain explicitly:

```bash
rust_pin=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)
rustup toolchain install "$rust_pin" --profile minimal --component rustfmt --component clippy
```

Diagnostics and preparation disable automatic rustup installation. They do
not change the active toolchain or persist a toolchain override.

The open Libre Baskerville and IBM Plex Sans fonts are tracked repository
assets, privately registered by the Renderer; no global installation is needed.
Noto CJK is in the system-package list. Provision licensed copies of Sitka
Display, Palatino Linotype, and Segoe UI separately on hosts used for complete
typography review, then refresh Fontconfig with `fc-cache`. Those proprietary
fonts are not distributed by RoonScape or installed in ordinary Ubuntu CI.

Agent permissions must allow reading/executing the selected toolchains and
native tools, network access for locked downloads, writes within the worktree
and dependency caches, and execution of child processes. Grant write access to
the runtime and chosen evidence directories explicitly. `TMPDIR` must be an
existing writable directory with a short path (native capture control uses
Unix sockets); an explicitly set `XDG_RUNTIME_DIR` must also be usable. Create
`/var/tmp/codex/roonscape` and grant agent access for command-test scratch files.
Diagnostic filesystem checks inspect permissions and existing ancestors without
creating files. They cannot prove free space, sandbox permission to bind sockets
or start Xvfb, or the success of a future capture. These need the corresponding
verification command under the intended agent permissions. Neither command
changes permissions or reads personal application credentials.

Run `npm run verify` for headless required checks with retained evidence; use
`npm run verify -- --design` when the design suite is required. Follow the
[verification policy](agents/verification.md) for change-to-command mapping,
focused checks, evidence inspection, and the Live Capture Session helper-test
distinction. No Roon Server or Roon Authorization is required for either suite.

For end-to-end development-tooling acceptance, follow the
[two-worktree exercise](agents/worktree-acceptance.md). It accepts existing fresh
worktrees and retains concurrent verification and cancellation evidence.

## Run from source

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

For unattended Fixture Mode, use a private headless desktop:

```sh
npm run fixture -- --headless --static --scenario playing --resolution 1280x720
```

`--headless` defaults to 1600×900. `--scenario` selects a maintained Fixture
Scenario; omit it to start the normal Fixture Mode catalog. In headless
operation, inherited fixture selections and Renderer overrides are ignored;
select the scenario, viewport, and static behavior explicitly with these flags.
The command reports its display and runtime directory once the native window
is mapped at the requested dimensions. It runs until cancelled with SIGINT or
SIGTERM. `npm run fixture:release -- --headless` builds and executes this
worktree's release Renderer instead. The launcher executes the built binary
directly; when calling `node scripts/run-fixture.mjs` directly, build the Bridge
and the selected Renderer profile first.

Headless Fixture Mode and Presentation Captures each own a temporary Display
Configuration, private home/XDG directories, Xvfb display, D-Bus session bus,
and process groups. They do not read personal Display Configuration or Roon
Authorization and do not contact Roon. Xvfb allocates its display atomically
and reports readiness through `-displayfd`; concurrent runs never claim a
display based on an observed socket alone. GTK application registration is
scoped to D-Bus, so a private display alone is insufficient: the native
regression probe confirms that a second Renderer sharing a bus redirects
activation to the first, even on another display. Verification uses a private
bus without service activation; Live Mode's application registration is
unchanged.

Startup and native readiness waits have five-second bounds (PNG encoding has
a thirty-second bound). Cleanup sends SIGTERM to owned process groups, waits
up to two seconds, then escalates to SIGKILL with a further two-second bound.
On cancellation, each grace and escalation wait is limited to 250 milliseconds
so the CLI can finish nested cleanup before its caller escalates termination.
Application processes stop before their private display and session bus.
Normal exit, startup failure, and SIGINT/SIGTERM cancellation all use this
cleanup path; cancellation exits nonzero. Cleanup removes only the run's
temporary runtime tree and unfinished publication files. Completed captures,
neighboring sessions, and personal configuration remain intact. SIGKILL of
the launcher or host failure cannot run application cleanup.

For manual inspection on a chosen X desktop, explicitly request a window:

```sh
DISPLAY=:0 ROONSCAPE_WINDOWED=1 npm run fixture -- --static --scenario paused
```

Manual operation retains the existing desktop/display defaults. Use headless
operation for unattended verification alongside an existing Live Mode session.

## Presentation Captures

Presentation Captures use the same deterministic static Fixture Mode through
the native renderer. The capture host additionally needs `Xvfb`, `xwininfo`,
`scrot`, and `dbus-daemon` on `PATH`.

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
capture the maintained nonduplicate Fixture Scenarios, with custom artwork
applied to every compatible scenario when requested.

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

For indexed review evidence and recorded visual verdicts, extend the retained
verification directory with `npm run review:presentations`. Follow the
[presentation visual-acceptance guide](visual-acceptance/presentation.md) for
focused selection across all maintained viewports, the complete profile required
for shared typography/layout/palette changes, and the CI fallback scope.

Run the self-contained design test suite before accepting changes to the
Renderer, presentation design, capture planning or execution, Fixture
Scenarios, artwork, fonts, styles, or capture-related build orchestration:

```sh
npm run test:design
```

The command builds its Bridge and Renderer prerequisites once, then runs the
explicitly maintained Presentation Capture test family. These tests are not
part of the regular repository test process.

Run the complete repository check before submitting a change:

```sh
npm run verify -- --design
```

For release work, follow the [Releasing guide](releasing.md).
