import { registerCustomCSSVariableTheme } from "@pierre/diffs";

export const MURVIX_DIFF_THEME = "murvix-diffs";

const tint = (mix, base = "var(--murvix-foreground)") =>
  `color-mix(in srgb, ${base} ${mix}%, var(--murvix-accent) ${100 - mix}%)`;

const VARIABLE_DEFAULTS = {
  foreground: "var(--murvix-foreground)",
  background: "var(--murvix-background)",
  "token-comment": "var(--murvix-foreground-muted)",
  "token-string": tint(35),
  "token-string-expression": tint(35),
  "token-keyword": tint(15),
  "token-constant": tint(45),
  "token-function": tint(25),
  "token-parameter": "var(--murvix-foreground)",
  "token-punctuation": "var(--murvix-foreground-muted)",
  "token-link": "var(--murvix-accent)",
  "token-inserted": "var(--murvix-diff-add)",
  "token-deleted": "var(--murvix-diff-remove)",
  "token-changed": "var(--murvix-diff-hunk)",
};

export const HEADER_CSS = `
[data-diffs-header][data-diffs-header] {
  cursor: pointer;
  background-color: var(--bg);
  background-image: linear-gradient(var(--diff-header-bg), var(--diff-header-bg));
  border-bottom: 1px solid var(--diff-header-border);
  min-height: 0;
  height: calc(34px * var(--diff-zoom, 1));
}
[data-diffs-header][data-diffs-header]:hover {
  background-image: linear-gradient(var(--diff-header-hover), var(--diff-header-hover));
}
[data-diffs-header] [data-title] {
  font-weight: 600;
}
`;

let registered = false;

export function registerMurvixDiffTheme() {
  if (registered) return MURVIX_DIFF_THEME;
  registerCustomCSSVariableTheme(MURVIX_DIFF_THEME, VARIABLE_DEFAULTS, true);
  registered = true;
  return MURVIX_DIFF_THEME;
}
