# Omarchy integration

Murvix's Linux window class is `com.murvix.app` (stable) or `com.murvix-beta.app` (beta). Omarchy's universal copy/paste bindings use that class to decide whether to send `CTRL+SHIFT+C`/`CTRL+SHIFT+V` (terminal) or `CTRL+C`/`CTRL+V` (normal windows).

Copy the rule in `murvix.lua` into `~/.config/hypr/hyprland.lua`, then run `hyprctl reload` so Omarchy treats Murvix as a terminal. This makes **Super+C** copy the selection instead of interrupting the running process.
