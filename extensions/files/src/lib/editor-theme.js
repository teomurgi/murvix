import { EditorView } from "@codemirror/view";

export function murvix_cm_theme(is_dark) {
  return EditorView.theme(
    {
      "&": {
        backgroundColor: "var(--murvix-background)",
        color: "var(--murvix-foreground)",
      },
      "&.cm-focused": { outline: "none" },
      ".cm-content": {
        fontFamily: '"SF Mono", Menlo, monospace',
      },
      ".cm-gutters": {
        backgroundColor: "var(--murvix-background)",
        color: "color-mix(in srgb, var(--murvix-foreground-muted) 55%, transparent)",
        border: "none",
        borderRight: "1px solid var(--murvix-border)",
      },
      ".cm-activeLine": { backgroundColor: "var(--murvix-hover)" },
      ".cm-activeLineGutter": {
        backgroundColor: "var(--murvix-hover)",
        color: "var(--murvix-foreground)",
      },
      ".cm-lineNumbers .cm-gutterElement": {
        padding: "0 8px",
      },
      ".cm-cursor, .cm-dropCursor": {
        borderLeftColor: "var(--murvix-accent)",
        borderLeftWidth: "2px",
      },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
        backgroundColor: "var(--murvix-accent-soft) !important",
      },
      ".cm-content ::selection": {
        backgroundColor: "var(--murvix-accent-soft)",
        color: "var(--murvix-foreground)",
      },
      ".cm-searchMatch": {
        backgroundColor: "var(--murvix-accent-soft)",
        outline: "1px solid color-mix(in srgb, var(--murvix-accent) 45%, transparent)",
      },
      ".cm-searchMatch.cm-searchMatch-selected": {
        backgroundColor: "color-mix(in srgb, var(--murvix-accent) 40%, transparent)",
      },
      ".cm-panels": {
        backgroundColor: "var(--murvix-background)",
        color: "var(--murvix-foreground)",
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
      },
      ".cm-panels.cm-panels-top": {
        borderBottom: "1px solid var(--murvix-border)",
      },
      ".cm-panel.cm-search.cm-find-panel": {
        boxSizing: "border-box",
        display: "flex",
        flexDirection: "column",
        position: "relative",
        minHeight: "32px",
        gap: "var(--s2)",
        padding: "var(--s2) var(--s5)",
        backgroundColor: "var(--murvix-background)",
        color: "var(--murvix-foreground)",
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
        fontSize: "var(--font-body)",
        lineHeight: "1",
      },
      ".cm-find-panel": {
        width: "100%",
      },
      ".cm-find-row": {
        display: "flex",
        width: "100%",
        minHeight: "24px",
        alignItems: "center",
        gap: "var(--s2)",
      },
      ".cm-replace-row[hidden]": {
        display: "none",
      },
      ".cm-panel.cm-search.cm-find-panel input, .cm-panel.cm-search.cm-find-panel button, .cm-panel.cm-search.cm-find-panel label": {
        margin: "0",
      },
      ".cm-search label": {
        display: "inline-flex",
        height: "24px",
        alignItems: "center",
        gap: "var(--s2)",
        color: "var(--murvix-foreground-muted)",
        fontSize: "var(--font-body)",
        whiteSpace: "nowrap",
      },
      ".cm-search input[type='checkbox']": {
        width: "14px",
        height: "14px",
        margin: "0",
        accentColor: "var(--murvix-accent)",
      },
      ".cm-textfield": {
        boxSizing: "border-box",
        width: "220px",
        height: "24px",
        backgroundColor: "var(--murvix-surface)",
        color: "var(--murvix-foreground)",
        border: "1px solid var(--murvix-border)",
        borderRadius: "6px",
        padding: "0 var(--s4)",
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
        fontSize: "var(--font-body)",
        lineHeight: "22px",
      },
      ".cm-textfield:focus": {
        outline: "none",
        borderColor: "var(--murvix-accent)",
      },
      ".cm-button": {
        boxSizing: "border-box",
        height: "24px",
        backgroundColor: "var(--murvix-surface)",
        backgroundImage: "none",
        color: "var(--murvix-foreground)",
        border: "1px solid var(--murvix-border)",
        borderRadius: "6px",
        padding: "0 var(--s4)",
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
        fontSize: "var(--font-body)",
        lineHeight: "22px",
      },
      ".cm-button:hover": {
        backgroundColor: "var(--murvix-hover)",
      },
      ".cm-button.cm-button-active, .cm-button[aria-expanded='true']": {
        backgroundColor: "var(--murvix-accent-soft)",
        borderColor: "var(--murvix-accent)",
      },
      ".cm-panel.cm-search.cm-find-panel button[name=close]": {
        position: "static",
        inset: "auto",
        display: "inline-flex",
        width: "24px",
        height: "24px",
        flex: "0 0 24px",
        alignItems: "center",
        justifyContent: "center",
        alignSelf: "center",
        padding: "0",
        margin: "0",
        marginLeft: "auto",
        border: "0",
        borderRadius: "6px",
        background: "transparent",
        color: "var(--murvix-foreground-muted)",
      },
      ".cm-panel.cm-search.cm-find-panel button[name=close]:hover": {
        backgroundColor: "var(--murvix-hover)",
        color: "var(--murvix-foreground)",
      },
      ".cm-panel.cm-search.cm-find-panel button[name=close] svg": {
        width: "14px",
        height: "14px",
      },

      ".cm-panel.cm-dialog": {
        display: "flex",
        alignItems: "center",
        gap: "var(--s2)",
        padding: "var(--s2) var(--s5)",
        backgroundColor: "var(--murvix-background)",
        color: "var(--murvix-foreground)",
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
      },
      ".cm-dialog label": {
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--s2)",
        fontSize: "var(--font-body)",
        color: "var(--murvix-foreground-muted)",
        whiteSpace: "nowrap",
      },
      ".cm-dialog input[name=line]": {
        width: "120px",
      },
      ".cm-dialog-close": {
        position: "static",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: "var(--control)",
        height: "var(--control)",
        marginLeft: "auto",
        padding: "0",
        border: "0",
        borderRadius: "var(--radius)",
        background: "transparent",
        color: "var(--murvix-foreground-muted)",
        fontSize: "var(--icon)",
        cursor: "pointer",
      },
      ".cm-dialog-close:hover": {
        backgroundColor: "var(--murvix-hover)",
        color: "var(--murvix-foreground)",
      },

      ".cm-foldGutter": {
        color: "color-mix(in srgb, var(--murvix-foreground-muted) 55%, transparent)",
      },
      ".cm-foldGutter .cm-gutterElement": {
        padding: "0 var(--s1)",
      },
      ".cm-fold-marker": {
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: "var(--icon)",
        height: "var(--icon)",
        cursor: "pointer",
        borderRadius: "var(--s2)",
        color: "var(--murvix-foreground-muted)",
        transition: "transform 120ms ease, background-color 120ms ease",
      },
      ".cm-fold-marker svg": {
        width: "var(--icon-sm)",
        height: "var(--icon-sm)",
      },
      ".cm-fold-marker:hover": {
        backgroundColor: "var(--murvix-hover)",
        color: "var(--murvix-foreground)",
      },
      ".cm-fold-marker-closed": {
        transform: "rotate(-90deg)",
      },
      ".cm-foldPlaceholder": {
        backgroundColor: "var(--murvix-surface)",
        border: "1px solid var(--murvix-border)",
        borderRadius: "var(--s2)",
        color: "var(--murvix-foreground-muted)",
        margin: "0 var(--s2)",
        padding: "0 var(--s3)",
      },

      ".cm-tooltip": {
        backgroundColor: "var(--murvix-background)",
      },
      ".cm-tooltip.cm-tooltip-autocomplete": {
        background: "var(--murvix-background)",
        backgroundImage: "linear-gradient(var(--murvix-surface), var(--murvix-surface))",
        border: "1px solid var(--murvix-border)",
        borderRadius: "var(--radius-card)",
        boxShadow: "0 var(--s4) var(--s9) color-mix(in srgb, black 24%, transparent)",
        overflow: "hidden",
      },
      ".cm-tooltip.cm-tooltip-autocomplete > ul": {
        fontFamily: '"SF Mono", Menlo, monospace',
        maxHeight: "16em",
        backgroundColor: "transparent",
      },
      ".cm-tooltip.cm-tooltip-autocomplete > ul > li": {
        padding: "var(--s1) var(--s4)",
        color: "var(--murvix-foreground)",
        lineHeight: "1.4",
      },
      ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
        backgroundColor: "color-mix(in srgb, var(--murvix-accent) 28%, var(--murvix-background))",
        color: "var(--murvix-foreground)",
      },
      ".cm-completionLabel": {
        color: "var(--murvix-foreground)",
      },
      ".cm-completionMatchedText": {
        textDecoration: "none",
        fontWeight: "600",
        color: "var(--murvix-accent)",
      },
      ".cm-completionDetail": {
        color: "var(--murvix-foreground-muted)",
        fontStyle: "normal",
        marginLeft: "var(--s4)",
      },
      ".cm-tooltip.cm-completionInfo": {
        background: "var(--murvix-background)",
        backgroundImage: "linear-gradient(var(--murvix-surface), var(--murvix-surface))",
        border: "1px solid var(--murvix-border)",
        borderRadius: "var(--radius-card)",
        color: "var(--murvix-foreground)",
        padding: "var(--s4) var(--s5)",
      },

      ".cm-lintRange-error": {
        backgroundImage: "none",
        textDecoration: "underline wavy var(--murvix-diff-remove)",
        textDecorationSkipInk: "none",
      },
      ".cm-lintRange-warning": {
        backgroundImage: "none",
        textDecoration: "underline wavy color-mix(in srgb, var(--murvix-accent) 70%, var(--murvix-foreground))",
        textDecorationSkipInk: "none",
      },
      ".cm-lint-marker": {
        width: "var(--icon-sm)",
        height: "var(--icon-sm)",
      },
      ".cm-lint-marker-error": {
        color: "var(--murvix-diff-remove)",
      },
      ".cm-lint-marker-warning": {
        color: "color-mix(in srgb, var(--murvix-accent) 80%, var(--murvix-foreground))",
      },
      ".cm-tooltip.cm-tooltip-lint": {
        background: "var(--murvix-background)",
        backgroundImage: "linear-gradient(var(--murvix-surface), var(--murvix-surface))",
        border: "1px solid var(--murvix-border)",
        borderRadius: "var(--radius-card)",
        boxShadow: "0 var(--s4) var(--s9) color-mix(in srgb, black 24%, transparent)",
      },
      ".cm-diagnostic": {
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
        fontSize: "var(--font-body)",
        color: "var(--murvix-foreground)",
        padding: "var(--s2) var(--s5)",
        borderLeft: "3px solid transparent",
      },
      ".cm-diagnostic-error": {
        borderLeftColor: "var(--murvix-diff-remove)",
      },
      ".cm-diagnostic-warning": {
        borderLeftColor: "color-mix(in srgb, var(--murvix-accent) 80%, var(--murvix-foreground))",
      },
      ".cm-panel.cm-panel-lint": {
        backgroundColor: "var(--murvix-background)",
        color: "var(--murvix-foreground)",
        fontFamily: '-apple-system, "SF Pro", system-ui, sans-serif',
      },
      ".cm-panel.cm-panel-lint ul [aria-selected]": {
        backgroundColor: "var(--murvix-accent-soft)",
      },
    },
    { dark: is_dark },
  );
}
