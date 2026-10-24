<h1 align="center">Murvix</h1>

<p align="center">An Omarchy-integrated, macOs compatible form of <a href="https://github.com/muxy-app/muxy">muxy</a> — a lightweight, memory-efficient terminal multiplexer built with Rust, GPUI and libghostty.</p>

## Credits

This is a fork of **[muxy](https://github.com/muxy-app/muxy)** by [Saeed Vaziry](https://github.com/saeedvaziry). All credit for the core application goes to the original author and contributors. This fork adds Omarchy integration on top.

## What this fork adds

- **Omarchy theme integration** (`crates/murvix-app/src/omarchy.rs`): renders the active Omarchy theme's `colors.toml` into Murvix's theme catalog as a theme named `Omarchy` (Ghostty format, same mapping as Omarchy's own `ghostty.conf.tpl`), and watches Omarchy's state directory so switching themes with `omarchy theme set` keeps it current.
- **Arch/Omarchy packaging**: `packaging/linux/PKGBUILD` and a `.desktop` file for a native install.

## Features (from upstream muxy)

- Background sessions
- Remote sessions
- Split panes
- Git worktrees
- Extensions
- AI coding agent integrations
- 490+ themes
- TUI

## Install (Arch / Omarchy)

```bash
git clone https://github.com/teomurgi/Murvix.git
cd Murvix/packaging/linux
makepkg -si
```

This builds the `murvix` CLI, `murvix-server`, and the `murvix-app` desktop app, and installs the desktop entry.

## Documentation

Upstream user and developer docs live in [docs](docs/) and at [muxy.app/docs](https://muxy.app/docs).

## License

[MIT](LICENSE) — original work Copyright (c) 2026 Muxy; see LICENSE.
