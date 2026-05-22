# Themes

BLXCode themes are **CSS custom properties** switched at runtime via `html[data-theme]`. No backend involvement.

## Key files

| Path | Role |
|------|------|
| `themes/tokens.css` | Token definitions per theme (`[data-theme="…"]`) |
| `styles.css` | Layout rules consuming `var(--*)` |
| `src/theme/catalog.rs` | `AppTheme` metadata (id, mode, preview colors) |
| `src/theme/i18n.rs` | Maps theme id → `I18nKey` for name/description |
| `src/workbench/theme_service.rs` | `ThemeService`: persist, apply, dispatch event |
| `src/workbench/appearance_settings_pane/` | Settings UI |
| `index.html` | Anti-flash boot script + CSS load order |
| `public/terminal_bootstrap.mjs` | xterm palette from computed tokens |
| `frontend-js/graph3d_entry.mjs` | 3D graph colors + theme listener |
| `src/workbench/memory_graph/mod.rs` | 2D SVG reads tokens via `getComputedStyle` |

## Runtime flow

```text
index.html boot script
  -> document.documentElement.dataset.theme
themes/tokens.css (before styles.css)
  -> [data-theme] { --bg-app, --accent, … }
ThemeService::set_theme (App root context)
  -> localStorage blxcode_theme_v1
  -> dataset.theme
  -> CustomEvent blxcode-theme-changed
terminal_bootstrap.mjs / graph3d_entry.mjs
  -> refresh instances from getComputedStyle
```

`ThemeService` is provided in `src/app.rs` (same level as `I18nService`) so EULA and boot screens inherit the active theme.

## Adding a theme

1. Add an `AppTheme` entry in `src/theme/catalog.rs` (include `ThemePreviewColors`).
2. Add `ThemeName*` / `ThemeDesc*` keys to `src/i18n/keys.rs` and all locale files.
3. Add `theme_name_key` / `theme_desc_key` arms in `src/theme/i18n.rs`.
4. Add a `[data-theme="your-id"]` block in `themes/tokens.css` with **all** extended tokens (overlays, terminal, agent accents, git lanes, semantic colors).
5. Register the id in the boot script `VALID` map in `index.html`.
6. Extend xterm / graph theme maps if you add new token names (prefer reusing existing tokens).

Default theme id: `blxcode-dark` (`DEFAULT_THEME_ID`).

## Styling rules

- Use semantic tokens (`--accent`, `--overlay-2`, `--danger`, …) — not raw hex in component CSS.
- Component CSS lives beside components; global tokens live in `themes/tokens.css`.
- No `var(--token, #fallback)` literals in `src/**/*.css`.
- Run `scripts/lint_theme_tokens.sh` before merge.
- See `.agents/rules/rule-theme-tokens.md`.

## Tokenization tooling

`scripts/tokenize_styles_css.py` replaces hardcoded colors in `styles.css` with `var(--*)` references. Re-run after large CSS edits.

## Exceptions

Surfaces that cannot follow `data-theme` are documented in [THEME_EXCEPTIONS.md](../THEME_EXCEPTIONS.md).

## i18n

Appearance chrome uses keys such as `AppearanceHeroTitle`, `AppearanceFilterDark`, etc. Theme catalog strings use paired `ThemeName*` / `ThemeDesc*` keys per theme id. Regenerate missing locale rows with:

```bash
python scripts/tools/render_i18n_locales_from_en.py
```
