-- Tag Murvix as a terminal for Omarchy's universal clipboard shortcuts.
-- Without this rule, SUPER+C sends CTRL+C and interrupts the running shell.
o.window("com\\.muxy(-beta)?\\.app", { tag = "+terminal" })
