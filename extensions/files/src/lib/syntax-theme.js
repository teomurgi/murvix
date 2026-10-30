import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

const FG = "var(--murvix-foreground)";
const MUTED = "var(--murvix-foreground-muted)";
const ACCENT = "var(--murvix-accent)";

const PALETTE = {
  keyword: "color-mix(in oklab, var(--murvix-diff-remove) 78%, var(--murvix-foreground))",
  string: "color-mix(in oklab, var(--murvix-accent) 82%, var(--murvix-foreground))",
  regexp: "color-mix(in oklab, var(--murvix-diff-add) 80%, var(--murvix-foreground))",
  escape: "color-mix(in oklab, var(--murvix-accent) 62%, var(--murvix-foreground))",
  constant: "color-mix(in oklab, var(--murvix-accent) 62%, var(--murvix-foreground))",
  function: "color-mix(in oklab, var(--murvix-diff-hunk) 78%, var(--murvix-foreground))",
  type: "color-mix(in oklab, var(--murvix-diff-remove) 50%, var(--murvix-diff-hunk))",
  property: "color-mix(in oklab, var(--murvix-accent) 70%, var(--murvix-foreground))",
  tag: "color-mix(in oklab, var(--murvix-diff-add) 80%, var(--murvix-foreground))",
};

export const syn = (name) => PALETTE[name] ?? FG;

export const SYNTAX_SPEC = [
  { tag: [t.keyword, t.modifier, t.controlKeyword, t.operatorKeyword, t.definitionKeyword], color: syn("keyword") },
  { tag: [t.string, t.special(t.string), t.attributeValue], color: syn("string") },
  { tag: [t.regexp], color: syn("regexp") },
  { tag: [t.escape, t.character], color: syn("escape") },
  { tag: [t.number, t.bool, t.integer, t.float], color: syn("constant") },
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: MUTED, fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: syn("function") },
  { tag: [t.typeName, t.className, t.namespace], color: syn("type") },
  { tag: [t.constant(t.variableName), t.standard(t.name), t.atom, t.self], color: syn("constant") },
  { tag: [t.propertyName, t.attributeName], color: syn("property") },
  { tag: [t.tagName], color: syn("tag") },
  { tag: [t.punctuation, t.separator, t.bracket, t.operator], color: MUTED },
  { tag: [t.meta, t.processingInstruction], color: MUTED },
  { tag: [t.link], color: ACCENT, textDecoration: "underline" },
  { tag: [t.heading], color: FG, fontWeight: "bold" },
  { tag: [t.strong], fontWeight: "bold" },
  { tag: [t.emphasis], fontStyle: "italic" },
  { tag: [t.strikethrough], textDecoration: "line-through" },
  { tag: [t.invalid], color: "var(--murvix-diff-remove, var(--murvix-foreground))" },
];

export function murvix_highlight_style() {
  return syntaxHighlighting(HighlightStyle.define(SYNTAX_SPEC));
}

const PREVIEW_STYLE_ID = "murvix-files-syntax";

export function ensure_preview_highlight_css() {
  if (typeof document === "undefined") return;
  if (document.getElementById(PREVIEW_STYLE_ID)) return;
  const css = SYNTAX_SPEC.map((rule, i) => {
    const decls = [];
    if (rule.color) decls.push(`color: ${rule.color}`);
    if (rule.fontStyle) decls.push(`font-style: ${rule.fontStyle}`);
    if (rule.fontWeight) decls.push(`font-weight: ${rule.fontWeight}`);
    if (rule.textDecoration) decls.push(`text-decoration: ${rule.textDecoration}`);
    return `.md-preview .tok-${i}{${decls.join(";")}}`;
  }).join("\n");
  const style = document.createElement("style");
  style.id = PREVIEW_STYLE_ID;
  style.textContent = css;
  document.head.appendChild(style);
}
