# Theme tokens

## Ziel

App-weite Themes über `html[data-theme]` und semantische CSS-Custom-Properties. Keine hardcodierten Farben in UI-Stylesheets.

## Vorgaben

- **Neue UI** nutzt ausschließlich `var(--token)` aus [`themes/tokens.css`](../../themes/tokens.css).
- **Keine** `var(--token, #literal)`-Fallbacks in Komponenten-CSS.
- **Theme-Wechsel** läuft über [`ThemeService`](../../src/workbench/theme_service.rs) (`blxcode-theme-changed` Event).
- **JS-Bridges** (xterm, Graph 3D) lesen Tokens via `getComputedStyle(document.documentElement)` — nicht hardcodieren.
- **Rust/SVG** (Memory-Graph 2D): Farben aus `read_css_var()` + Reaktion auf `ThemeService::active_theme_id`.

## Bewusste Ausnahmen

Siehe [`docs/THEME_EXCEPTIONS.md`](../../docs/THEME_EXCEPTIONS.md).

## CI

`scripts/lint_theme_tokens.sh` vor Merge ausführen.
