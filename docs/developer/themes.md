# Themes

BLXCode themes are **CSS custom properties** switched at runtime via `html[data-theme]`. No backend involvement. The **Appearance** pane ships **32 themes** (16 dark + 16 light) plus two theme-independent controls — **Roundings** and **Font** — that ride on the same `ThemeService` machinery.

## Key files

| Path | Role |
|------|------|
| `themes/tokens.css` | Token definitions per theme (`[data-theme="…"]`) |
| `styles.css` | Layout rules consuming `var(--*)` |
| `src/theme/catalog.rs` | `AppTheme` metadata (id, mode, preview colors) for 32 themes |
| `src/theme/i18n.rs` | Maps theme id → `I18nKey` for name/description |
| `src/theme/appearance.rs` | Roundings & Font settings (scales + font picker) |
| `src/workbench/theme_service.rs` | `ThemeService`: persist, apply, dispatch event (incl. roundings + font) |
| `src/workbench/appearance_settings_pane/` | Settings UI (roundings, font, theme grid) |
| `index.html` | Anti-flash boot script + CSS load order |
| `public/terminal_bootstrap.mjs` | xterm palette + `fontFamily` from computed tokens |
| `frontend-js/graph3d_entry.mjs` | 3D graph colors + theme listener |
| `src/workbench/memory_graph/mod.rs` | 2D SVG reads tokens via `getComputedStyle` |

## Runtime flow

```text
index.html boot script
  -> document.documentElement.dataset.theme
  -> document.documentElement.style.setProperty('--radius-scale', …)
  -> document.documentElement.style.setProperty('--font-mono', …)
themes/tokens.css (before styles.css)
  -> [data-theme] { --bg-app, --accent, … }
ThemeService::set_theme / set_roundings / set_font (App root context)
  -> localStorage blxcode_theme_v1 (and related keys)
  -> dataset.theme / inline custom properties on <html>
  -> CustomEvent blxcode-theme-changed
terminal_bootstrap.mjs / graph3d_entry.mjs
  -> refresh instances from getComputedStyle
  -> xterm re-measures and re-fits (with a PTY resize) on font change
```

`ThemeService` is provided in `src/app.rs` (same level as `I18nService`) so EULA and boot screens inherit the active theme. Roundings and Font dispatch the same `blxcode-theme-changed` event so the terminal and 3D graph bridges re-read CSS variables on any change.

## Roundings

A single `--radius-scale` multiplier (Sharp / Default / Rounded / Extra) drives a semantic token set:

| Token | Role |
|-------|------|
| `--radius-xs` | Tightest radius (chips, inline tags) |
| `--radius-sm` | Small surfaces (buttons, inputs) |
| `--radius-md` | Standard surfaces (cards, panels) |
| `--radius-lg` | Larger surfaces (modals, popovers) |
| `--radius-xl` | Largest radius (hero blocks) |
| `--radius-pill` | Pill controls (intentionally not scaled) |
| `--radius-circle` | Perfect-circle controls (intentionally not scaled) |

`ThemeService` writes the scale as an inline custom property on `<html>`, and the previously hardcoded `border-radius` declarations were migrated to these tokens. Pills and circles are exempt so chip-shaped and circular controls keep their shape.

## Font

A curated monospace picker (JetBrains Mono bundled, plus Cascadia Code, Fira Code, SF Mono, Menlo, Consolas, and System Monospace as system-dependent options) overrides the central `--font-mono` token. `terminal_bootstrap.mjs` reads its `fontFamily` from `--font-mono` and re-measures/re-fits (with a PTY resize) when the font changes.

## Theme catalog (32 themes)

The catalog (`src/theme/catalog.rs`) covers 16 dark + 16 light themes. The flagship **BLXCode** (`blxcode-dark`, default) is a Tokyo Night × Dracula fusion (night-blue base, purple accent `#bd93f9`, pink/cyan syntax/lane colors). The previous GitHub-blue default lives on under new ids:

| Slot | Id | Mode |
|---|---|---|
| BLXCode (default) | `blxcode-dark` | Dark |
| BLXCode Light | `blxcode-light` | Light |
| BLXCode Legacy | `blxcode-legacy-dark` | Dark |
| BLXCode Legacy Light | `blxcode-legacy-light` | Light |
| Dracula | `dracula` | Dark |
| Gruvbox Dark / Light | `gruvbox-dark` / `gruvbox-light` | Dark / Light |
| Solarized Dark / Light | `solarized-dark` / `solarized-light` | Dark / Light |
| Nord / Nord Light | `nord` / `nord-light` | Dark / Light |
| One Dark / One Light (Atom) | `one-dark` / `one-light` | Dark / Light |
| Catppuccin Mocha / Latte / Frappé | `catppuccin-mocha` / `catppuccin-latte` / `catppuccin-frappe` | Dark / Light / Dark |
| Tokyo Night / Tokyo Night Light | `tokyo-night` / `tokyo-night-light` | Dark / Light |
| Rosé Pine / Rosé Pine Dawn | `rose-pine` / `rose-pine-dawn` | Dark / Light |
| Everforest Dark | `everforest-dark` | Dark |
| Kanagawa | `kanagawa` | Dark |
| Claude Code | `claude-code` | Dark (coral accent, replaces `github-dark`) |
| Night Owl | `night-owl` | Dark |
| Ayu Mirage / Ayu Light | `ayu-mirage` / `ayu-light` | Dark / Light |
| GitHub Light | `github-light` | Light |
| Winter Light | `winter-light` | Light (cyan on frost) |
| Paper Light | `paper-light` | Light (cobalt on warm paper) |
| Alpine Light | `alpine-light` | Light (teal on fresh green-blue) |
| Frost Light | `frost-light` | Light (ice-blue on pure white) |
| Lilac Light | `lilac-light` | Light (violet on soft lavender) |

Every theme ships the complete token set: surfaces, borders, text scale, accent + accent-cool, six overlay levels, terminal, agent brand colors, six git graph lanes, and the `color-mix`-based follow-up tokens (`text-secondary`/`-hint`, `accent-control`, `success-surface`/`-border`/`-text`, `danger-surface`/`-border-soft`/`-text-soft`, `hook-brand-bg`/`-fg`).

## Adding a theme

1. Add an `AppTheme` entry in `src/theme/catalog.rs` (include `ThemePreviewColors`).
2. Add `ThemeName*` / `ThemeDesc*` keys to `src/i18n/keys.rs` and all locale files.
3. Add `theme_name_key` / `theme_desc_key` arms in `src/theme/i18n.rs`.
4. Add a `[data-theme="your-id"]` block in `themes/tokens.css` with **all** extended tokens (overlays, terminal, agent accents, git lanes, semantic colors).
5. Register the id in the boot script `VALID` map in `index.html`.
6. Extend xterm / graph theme maps if you add new token names (prefer reusing existing tokens).

Default theme id: `blxcode-dark` (`DEFAULT_THEME_ID`).

## Adding a Roundings or Font option

Roundings is a small enum (`Sharp` / `Default` / `Rounded` / `Extra`) on `AppearanceSettings` and is applied as an inline CSS custom property. Font is a typed enum of curated monospace faces plus a free-form `System Monospace` fallback. Both persist in `localStorage` and are dispatched through the same `blxcode-theme-changed` event, so JS bridges can refresh the same way they do for theme changes.

## Styling rules

- Use semantic tokens (`--accent`, `--overlay-2`, `--danger`, …) — not raw hex in component CSS.
- Use the `--radius-{xs,sm,md,lg,xl}` tokens for corner radii; do not write raw `border-radius` values.
- Component CSS lives beside components; global tokens live in `themes/tokens.css`.
- No `var(--token, #fallback)` literals in `src/**/*.css`.
- Run `scripts/lint_theme_tokens.sh` before merge.
- See `.agents/rules/rule-theme-tokens.md`.

## Tokenization tooling

`scripts/tokenize_styles_css.py` replaces hardcoded colors in `styles.css` with `var(--*)` references. Re-run after large CSS edits.

## Exceptions

Surfaces that cannot follow `data-theme` are documented in [THEME_EXCEPTIONS.md](../THEME_EXCEPTIONS.md).

## i18n

Appearance chrome uses keys such as `AppearanceHeroTitle`, `AppearanceFilterDark`, `AppearanceRoundingsLabel`, `AppearanceFontLabel`, etc. Theme catalog strings use paired `ThemeName*` / `ThemeDesc*` keys per theme id. Regenerate missing locale rows with:

```bash
python scripts/tools/render_i18n_locales_from_en.py
```
