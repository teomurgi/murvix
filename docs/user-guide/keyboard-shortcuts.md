# Keyboard Shortcuts

Defaults for the desktop app, shown separately for macOS and Linux. `Opt` means
Option; `Super` is the Linux platform modifier (usually the Windows key).
Change app shortcuts in **Settings → Keyboard**. For the terminal UI, see
[Terminal UI keys](../features/terminal-ui.md#keys).

## Tabs and panes

| Action | macOS | Linux |
| --- | --- | --- |
| New tab | `Cmd+T` | `Ctrl+Shift+T` |
| New tab in Home | `Cmd+N` | `Ctrl+Shift+N` |
| Close pane | `Cmd+W` | `Ctrl+Shift+W` |
| Close tab | `Cmd+Shift+W` | `Ctrl+Alt+W` |
| Split right | `Cmd+D` | `Ctrl+Shift+D` |
| Split down | `Cmd+Shift+D` | `Ctrl+Shift+Alt+D` |
| Focus pane left / right / up / down | `Cmd+Opt+Arrows` | `Alt+Arrows` |
| Zoom pane | `Cmd+Shift+Return` | `Ctrl+Shift+Return` |
| Equalize splits | `Cmd+Ctrl+=` | `Ctrl+Alt+=` |
| Next / previous tab | `Cmd+]` / `Cmd+[`, or `Ctrl+Tab` / `Ctrl+Shift+Tab` | `Ctrl+PageDown` / `Ctrl+PageUp`, or `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Go to tab 1–9 | `Cmd+1`–`Cmd+9` | `Alt+1`–`Alt+9` |
| Existing terminals | `Cmd+Opt+T` | `Ctrl+Shift+Alt+T` |

## Projects and navigation

| Action | macOS | Linux |
| --- | --- | --- |
| Add project | `Cmd+O` | `Ctrl+Shift+O` |
| New worktree | `Cmd+Opt+N` | `Super+Alt+N` |
| Switch worktree | `Cmd+Opt+W` | `Super+Alt+W` |
| Next / previous project | `Ctrl+]` / `Ctrl+[` | `Ctrl+]` / `Ctrl+[` |
| Go to project 1–9 | `Ctrl+1`–`Ctrl+9` | `Ctrl+1`–`Ctrl+9` |
| Back / forward | `Cmd+Ctrl+Left` / `Cmd+Ctrl+Right` | `Ctrl+Shift+Left` / `Ctrl+Shift+Right` |
| Toggle sidebar | `Cmd+B` | `Ctrl+Shift+B` |
| Agents panel | `Cmd+Opt+A` | `Ctrl+Shift+Alt+A` |
| Command palette | `Cmd+Shift+P` | `Ctrl+Shift+P` |
| Theme picker | `Cmd+Shift+K` | `Ctrl+Alt+K` |
| Settings | `Cmd+,` | `Ctrl+,` |
| Full screen | `Cmd+Ctrl+F` | `F11` |

Your desktop environment may reserve some Linux shortcuts, especially Super keys.

## Terminal

| Action | macOS | Linux |
| --- | --- | --- |
| Copy / paste | `Cmd+C` / `Cmd+V` | `Ctrl+Shift+C` / `Ctrl+Shift+V` |
| Find / next / previous | `Cmd+F` / `Cmd+G` / `Cmd+Shift+G` | `Ctrl+Shift+F` / `Ctrl+Shift+G` / `Ctrl+Alt+G` |
| Previous / next prompt | `Cmd+Up` / `Cmd+Down` | `Ctrl+Shift+Up` / `Ctrl+Shift+Down` |
| Font size bigger / smaller / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |
| Clear screen | `Cmd+K` | No default binding |
| Scroll to top / bottom | `Cmd+Home` / `Cmd+End` | `Ctrl+Shift+Home` / `Ctrl+Shift+End` |
| Delete to start of line (`Ctrl+U`) | `Cmd+Backspace` | No default binding |
| Start / end of line (`Ctrl+A` / `Ctrl+E`) | `Cmd+Left` / `Cmd+Right` | No default binding |
| Line feed (`Ctrl+J`) | `Shift+Return` | `Shift+Return` |

A line feed is a new line in many AI tools, but shells treat it like Return.

## Browser

| Action | macOS | Linux |
| --- | --- | --- |
| New browser tab | `Cmd+Opt+B` | `Ctrl+Alt+B` |
| Address bar | `Cmd+L` | `Ctrl+L` |
| Reload | `Cmd+R` | `Ctrl+R` |
| Back / forward | `Cmd+[` / `Cmd+]` | `Alt+Left` / `Alt+Right` |
| Find on page | `Cmd+F` | `Ctrl+Shift+F` |
| Zoom in / out / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |
| Copy address | `Cmd+Shift+C` | `Ctrl+Shift+C` |
| Web Inspector | `Cmd+Opt+I` | `Ctrl+Shift+I` |

In a browser, `Cmd+[` and `Cmd+]` on macOS go back and forward instead of
switching tabs. Browser shortcuts take precedence over workspace shortcuts.

## Composer and dictation

| Action | macOS | Linux |
| --- | --- | --- |
| Toggle composer | `Cmd+I` | `Ctrl+Shift+I` |
| Send / send without Return | `Cmd+Return` / `Cmd+Shift+Return` | `Ctrl+Return` / `Ctrl+Shift+Return` |
| Dictation | `Cmd+Shift+I` | `Ctrl+Alt+I` |

## Changing shortcuts

- In **Settings → Keyboard**, click a shortcut and press the new keys.
  **Reset** restores the default. Murvix refuses conflicting shortcuts.
- Changes are saved to `settings.toml` under `[keymap]`, for example
  `split_right = "cmd-alt-d"` on macOS.
- Terminal keys such as clear screen, line editing, and scrolling have built-in
  bindings. Add your own in **Settings → Terminal → Key bindings**; they take
  precedence while a terminal is focused.
- Commands from **Settings → Commands** and extensions can have shortcuts too.
- The Quick Terminal shortcut is set in **Settings → Quick Terminal**.
