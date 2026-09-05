# Getting Started

These steps assume you extracted the release into the stable `roonscape`
directory in your home directory.

## Complete first-time setup

From a terminal in your graphical session, start RoonScape:

```sh
cd ~/roonscape
./roonscape
```

RoonScape waits for Roon Authorization. In an official Roon client, open
**Settings → Extensions** and enable RoonScape. When the available outputs
appear in the terminal, use Up and Down to choose the Tracked Output that the
RoonScape Host should present, then press Enter.

RoonScape next shows its OLED protection defaults. Press Enter to accept them,
or press C and answer the prompts to customize when dimming begins, the dimmed
opacity, and how often the presentation repositions. After saving the Display
Configuration, RoonScape continues directly into the presentation.

## Change setup choices later

To choose a different Tracked Output or change the OLED protection choices,
open a terminal, return to the extracted release directory, and rerun setup:

```sh
cd ~/roonscape
./roonscape --setup
```

Complete the same Tracked Output and OLED protection choices. RoonScape saves
the updated Display Configuration and exits; start it normally when you are
ready to return to the presentation.

## Add an application-menu entry

After completing setup, run this from the extracted RoonScape directory:

```sh
./runtime/node/bin/node scripts/install-desktop.mjs
```

The entry launches the display without a terminal and supplies the RoonScape
icon to desktop environments that support it. It does not enable automatic
startup. If launch fails, run `./roonscape` in a terminal to see the error.

The entry, icon, and launch helper are installed for the current user under
`$XDG_DATA_HOME` (default `~/.local/share`). Keep the extracted directory in
place, or rerun the command after moving it. Remove the entry and icon with:

```sh
./runtime/node/bin/node scripts/install-desktop.mjs --uninstall
```

These commands require an archive containing the desktop integration files;
they are not available in older archives. For a source checkout, use the
[development instructions](development.md#desktop-launcher).

## Launch RoonScape when X starts

After first-time setup is complete, an X session that reads the owner's
`~/.xinitrc` can launch RoonScape. For an owner whose release is extracted at
`/home/roon/roonscape`, make this the final line of that file, replacing
`/home/roon` with the owner's actual absolute home-directory path:

```sh
exec /home/roon/roonscape/roonscape
```

This line launches RoonScape when that X session starts. It does not configure
tty login, automatic login, or starting X during boot.
