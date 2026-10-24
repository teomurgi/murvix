<h1 align="center">Murvix</h1>

<p align="center"><strong>A terminal workspace that keeps your work running.</strong></p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#command-line">Command line</a> ·
  <a href="#terminal-ui">Terminal UI</a> ·
  <a href="#documentation">Documentation</a>
</p>

Murvix is a native terminal multiplexer and development workspace for macOS and Linux. One persistent server keeps your terminals alive while you move between the desktop app, a tmux-style terminal UI, and a scriptable command line. Close the window, drop an SSH connection, or pick up the same project from another machine — the work keeps running.

Built in Rust with GPUI and libghostty.

## Highlights

- **Terminals that survive.** Sessions live in `murvix-server`, not in a window. Quit the app, close the lid, or disconnect; screens and scrollback are still there when you return.
- **Projects, tabs, and splits.** Organize work by folder, split panes side by side or top and bottom, zoom a pane, and restore the layout next time.
- **AI agent awareness.** Murvix recognizes Claude Code, Codex, OpenCode, Copilot, Cursor, Droid, Grok, Kiro, Pi, Xal, Antigravity, and Oh My Pi, and shows whether each agent is working, waiting, or done.
- **Git in the workspace.** Branches, changes, worktrees, AI-written commit messages, and pull request status sit next to your terminals.
- **Remote development over SSH.** Add a remote server and work with its projects as if they were local. Its terminals keep running on the remote machine.
- **Browser and composer.** Open documentation or a local web app in a built-in browser pane, and write long prompts with attached files and images.
- **Extensible by design.** Build extensions with standard web tooling to add tabs, panels, sidebar items, commands, events, and language packs.
- **Themes for every setup.** Around 490 built-in themes, custom themes, separate light and dark choices, and native Omarchy theme sync.

## Install

### Arch Linux and Omarchy

The package in `packaging/linux` builds the desktop app, CLI, terminal UI, and server:

```bash
git clone https://github.com/teomurgi/murvix.git
cd murvix/packaging/linux
makepkg -si
```

Launch **Murvix** from the application menu, or run `murvix` in a terminal.

If you run Murvix on Omarchy, add the window rule below so Omarchy treats it as a terminal. Otherwise **Super+C** sends `Ctrl+C` to the focused shell instead of copying:

```lua
-- ~/.config/hypr/hyprland.lua
o.window("com\\.murvix(-beta)?\\.app", { tag = "+terminal" })
```

After saving, run `hyprctl reload` to apply the rule.

### Build from source

You need Git, a C toolchain, Rust 1.92 (installed by `rustup` from `rust-toolchain.toml`), and Zig 0.15.2 on your `PATH`. On macOS, install the Xcode Command Line Tools. On Linux, install the packages in [`packaging/linux/PKGBUILD`](packaging/linux/PKGBUILD); WebKitGTK 4.1 is also needed for extensions and browser panes.

```bash
git clone https://github.com/teomurgi/murvix.git
cd murvix
rustup toolchain install
cargo build --release --locked --workspace
```

The build produces `murvix`, `murvix-server`, and `murvix-app` in `target/release`.

Murvix needs macOS 14 or newer, or Linux with glibc 2.35 or newer, on x86_64 or ARM64. Linux desktop support is experimental; the CLI, terminal UI, and server are the most portable way to use Murvix on Linux. See [platform constraints](docs/tech/constraints.md) for details.

## Quick start

1. Open the Murvix desktop app and choose **Add Project**.
2. Create a terminal tab and split it from the menu or with the keyboard.
3. Quit the app and reopen it — the terminal is still running, with its scrollback intact.

The same projects and sessions are available from a shell:

```bash
murvix                  # open the terminal UI
murvix .                # open the current folder
murvix project list     # list projects
murvix --help           # every command
```

## Command line

`murvix` is both the CLI and the terminal UI. Commands talk to the server, so they work over SSH and can drive terminals without a desktop app.

```bash
murvix project add ~/code/my-app --name my-app
id=$(murvix session create my-app)
murvix session send "$id" "npm test"
murvix session send-keys "$id" Enter
murvix session read-screen "$id" --lines 20
```

- Management commands accept `--json` for scripts.
- `--host <destination>` runs a command against a server on another computer: `murvix --host devbox project list`.
- `murvix exec <project> -- <program>` runs a program in the project folder without a shell.
- `murvix install-skills` installs the CLI reference for supported AI agents.

See the [CLI reference](docs/features/murvix-cli.md) for sessions, worktrees, browser control, mobile pairing, and every other command.

## Terminal UI

Run `murvix` with no arguments to use Murvix inside any terminal. It shares the same server, projects, and sessions as the desktop app, with its own tabs and splits.

```bash
murvix                  # this computer
murvix --host devbox    # another computer over SSH
```

Keys start with `Ctrl-B`, like tmux:

| Keys | Action |
| --- | --- |
| `Ctrl-B ?` | Show help |
| `Ctrl-B c` | New tab |
| `Ctrl-B %` | Split side by side |
| `Ctrl-B "` | Split top and bottom |
| `Ctrl-B` + arrows | Move between panes |
| `Ctrl-B d` | Detach; terminals keep running |

The [terminal UI guide](docs/features/terminal-ui.md) covers scrollback, mouse support, pane management, and project switching.

## Make it yours

**Themes.** Pick light and dark themes in **Settings → Appearance**, or drop your own themes into the profile's `themes/` folder. On Omarchy, Murvix writes the active desktop palette to the theme catalog as `Omarchy` and refreshes it whenever the desktop theme changes; select `Omarchy` once and future theme switches follow along.

**Extensions.** Extensions are web projects that can add tabs, panels, popovers, sidebar and status bar items, commands, shortcuts, background logic, and translations. Manage them in **Settings → Extensions**, and declare the permissions each one needs. Start with the [extension guide](docs/extensions/get-started.md).

**Layouts.** Check a layout file into a project to describe a tab of split terminals that anyone can open with one click. See [layouts](docs/layouts/overview.md).

## Documentation

- [Getting started](docs/user-guide/getting-started.md)
- [Keyboard shortcuts](docs/user-guide/keyboard-shortcuts.md)
- [Settings](docs/user-guide/settings.md)
- [Projects](docs/features/projects.md) · [Terminal](docs/features/terminal.md) · [Git](docs/features/git.md)
- [AI agents](docs/features/ai-agents.md) · [Remote servers](docs/features/remote-servers.md) · [Browser](docs/features/browser.md)
- [Murvix CLI](docs/features/murvix-cli.md) · [Terminal UI](docs/features/terminal-ui.md)
- [Extensions](docs/extensions/overview.md) · [Layouts](docs/layouts/overview.md)
- [Architecture](docs/tech/architecture.md) and [product design](docs/product/README.md) for contributors

## Contributing

Issues and pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and the project's contribution rules, and follow the [Code of Conduct](CODE_OF_CONDUCT.md). Security reports should follow [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE).

## Thanks

Murvix builds on [Muxy](https://github.com/muxy-app/muxy), the project it was derived from. Thank you to [Saeed Vaziry](https://github.com/saeedvaziry) and everyone who has contributed to Muxy — their work made Murvix possible.
