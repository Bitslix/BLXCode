# Theme-Roundings + Font in Appearance

> Status: **planned**

## Summary

Der Nutzer soll unter **Settings → Appearance** zwei neue, theme-übergreifende
Darstellungs-Knöpfe bekommen — zusätzlich zur bestehenden Theme-Auswahl:

1. **Roundings** — eine **globale Eckenradius-Stufe** (Sharp / Default /
   Rounded / Extra), die alle UI-Radien über einen einzigen
   `--radius-scale`-Multiplikator skaliert. Pills (`999px`) und Kreise (`50%`)
   skalieren bewusst **nicht**.
2. **Font** — ein **kuratierter Schrift-Picker** (geliefert: JetBrains Mono;
   System-Fallbacks: Cascadia, Fira Code, SF Mono, Menlo, Consolas, System-Mono).
   Die Auswahl überschreibt das zentrale `--font-mono`-Token und wird zusätzlich
   in die **xterm-Terminals** verdrahtet (inkl. Refit), optional mit einer
   Schriftgröße.

Beide Einstellungen sind **app-weit** (nicht pro Workspace), werden in
`localStorage` persistiert und über denselben DOM-Apply-/Event-Pfad angewandt
wie das Theme (`ThemeService` + `blxcode-theme-changed`). Sie sind orthogonal
zum gewählten Theme: jede Theme-Wahl behält Roundings/Font.

## Kontext / Ist-Zustand (verifiziert)

- **Kein Radius-Token** vorhanden. `styles.css` enthält **~245 hartkodierte
  `border-radius`-Werte**; Häufigkeiten: `4px`×96, `6px`×25, `999px`×22,
  `3px`×17, `8px`×15, `0`×9, `50%`×8, `0.4rem`×8, `5px`×6, `0.5rem`×6,
  `inherit`×5, `0.45rem`×4, Rest vereinzelt. Komponenten-CSS hat weitere.
- **Ein einziges Font-Token** `--font-mono` (Definition in `themes/tokens.css`
  `:root`, Z. 35). `styles.css :root` setzt `font-family: var(--font-mono)` und
  `font-size: 15px`. JetBrains Mono ist via `@font-face` in `styles.css`
  gebündelt; andere Fonts sind system-abhängig.
- **Terminal** (`public/terminal_bootstrap.mjs`): liest CSS-Vars via
  `getComputedStyle(document.documentElement)` und hört auf
  `blxcode-theme-changed` (`applyThemeToAllTerminals`, Z. 54). **Aber** die
  `fontFamily` ist hartkodiert (Z. ~229) und liest **nicht** `--font-mono` →
  muss explizit verdrahtet + nach Font-Wechsel neu vermessen/gefittet werden.
- **Persistenz-Muster**: `AppPrefsService` (`src/workbench/app_prefs.rs`) hält
  `RwSignal`s + `read_/write_string_storage`-Helfer; Keys zentral in
  `src/config/app.config.rs`. `ThemeService`
  (`src/workbench/theme_service.rs`) ist bereits Owner von DOM-Apply
  (`data-theme` auf `<html>`) + `blxcode-theme-changed`-Dispatch.
- **Regel** `rule-theme-tokens.md`: keine hartkodierten **Farben** in
  Komponenten-CSS, keine `var(--token, #literal)`-Fallbacks; `ThemeService` ist
  der Wechsel-Pfad; JS-Bridges lesen via `getComputedStyle`. Radien fallen nicht
  unter den Farb-Lint, der Token-Ansatz hält sich aber an denselben Geist.
  `scripts/lint_theme_tokens.sh` vor Merge.

## Decisions

- **Roundings = globale Stufe**, kein Pro-Token-Advanced-Modus (kann Follow-up
  werden). Umsetzung über **einen** `--radius-scale`-Multiplikator + eine kleine
  Menge semantischer `--radius-*`-Tokens, die per `calc()` skalieren.
- **Stufen**: `Sharp ×0`, `Default ×1`, `Rounded ×1.5`, `Extra ×2` (Enum
  `RadiusScale`, Default = `Default`). `--radius-scale` wird als **Inline-Style
  auf `document.documentElement`** gesetzt (überschreibt den `:root`-Default).
- **Pill/Kreis skalieren nicht**: `--radius-pill: 999px`, `--radius-circle: 50%`
  bleiben konstant; nur `--radius-xs/sm/md/lg/xl` tragen `* var(--radius-scale)`.
- **Migration pragmatisch**: nur die häufigsten Literale auf Tokens mappen
  (`3px→xs`, `4px→sm`, `6px→md`, `8px→lg`, `12px→xl`, `999px→pill`, `50%→circle`).
  `0`, `inherit` und seltene Spezialwerte bleiben unangetastet. `rem`-Radien
  optional in einer Folgephase. **Vollständigkeit ist kein Ziel** — Stufe wirkt
  auf alles Migrierte sofort sichtbar.
- **Font = kuratierte Liste, App + Terminal**. Auswahl überschreibt
  `--font-mono` (kompletter Fallback-Stack pro Eintrag, damit nicht installierte
  Fonts sauber degradieren). Terminal-`fontFamily` wird auf `--font-mono`
  umgestellt.
- **Ein Owner**: `ThemeService` wird um `radius_scale` + `font_choice`
  (+ optional `font_size`) erweitert (Signale, Persistenz, DOM-Apply). Jede
  Appearance-Änderung dispatcht `blxcode-theme-changed`, sodass Terminal-/
  Graph-Bridges einmalig neu lesen. Kein neuer Service, kein neues Event.
- **App-weit**, nicht pro Workspace (wie Theme/Shortcut-Mode).

## Technischer Plan

### 1. Tokens (`themes/tokens.css`, `:root`)
```css
--radius-scale: 1;
--radius-xs: calc(3px * var(--radius-scale));
--radius-sm: calc(4px * var(--radius-scale));
--radius-md: calc(6px * var(--radius-scale));
--radius-lg: calc(8px * var(--radius-scale));
--radius-xl: calc(12px * var(--radius-scale));
--radius-pill: 999px;   /* skaliert nicht */
--radius-circle: 50%;   /* skaliert nicht */
```
`--font-mono` bleibt der zentrale Font-Hebel (bereits vorhanden).

### 2. Migration der Radius-Literale
Sweep über `styles.css` + Komponenten-CSS, häufigste Werte → Tokens. Mehrwert-
Schreibweisen (`4px 4px 0 0` → `var(--radius-sm) var(--radius-sm) 0 0`) bleiben
gültig. In Phasen committen, damit Diffs reviewbar bleiben.

### 3. Service + Persistenz
- Neue Keys in `app.config.rs`: `RADIUS_SCALE_KEY = "blxcode_radius_scale_v1"`,
  `FONT_FAMILY_KEY = "blxcode_font_family_v1"` (+ optional
  `FONT_SIZE_KEY = "blxcode_font_size_v1"`).
- `RadiusScale`-Enum (+ `from_storage`/`storage_value`/`multiplier`),
  `FontChoice`-Enum/Struct (id, Anzeigename, vollständiger CSS-Stack).
- `ThemeService`: Signale + Setter, die `localStorage` schreiben,
  `document.documentElement.style.setProperty("--radius-scale", …)` bzw.
  `setProperty("--font-mono", stack)` (und `font-size` auf root) setzen und
  `blxcode-theme-changed` dispatchen. Initial-Apply im `new()` analog zum Theme.

### 4. Appearance-UI (`appearance_settings_pane/`)
- Neue Sektion **Roundings**: Segment-Control (4 Stufen) mit Mini-Preview-
  Kacheln, die den aktuellen Radius zeigen.
- Neue Sektion **Font**: Listbox/Dropdown; jeder Eintrag in der jeweiligen
  Schrift gerendert mit Beispieltext (`AaBbCc 123 () => {}`); optional
  Größen-Auswahl (S/M/L). Reaktiv → `ThemeService`-Setter.
- Beide oberhalb des bestehenden Theme-Grids; bestehende Such-/Filter-Leiste
  unverändert.

### 5. Terminal-/Graph-Verdrahtung
- `terminal_bootstrap.mjs`: `fontFamily` aus
  `getComputedStyle(...).getPropertyValue('--font-mono')` (mit Hard-Fallback)
  beziehen; `applyThemeToAllTerminals` setzt `term.options.fontFamily`, ruft
  `fitAddon.fit()` + `term.refresh()` und re-misst nach `document.fonts.ready`.
- Memory-Graph 2D/3D: prüfen, ob Font relevant ist (meist nur Farben). Falls
  Text gerendert wird, `--font-mono` via `read_css_var()` nachziehen — sonst
  Follow-up.

### 6. i18n
Neue Keys (Section-Titel + Beschreibung, Stufen-Labels Sharp/Default/Rounded/
Extra, Font-Label/Aria, optional Größen-Labels) in `keys.rs` + **allen 13
Locales** (en/de hand-übersetzt, Rest via `scripts/render_i18n_locales_from_en.py`
oder engl. Fallback). Compile-Time-Exhaustiveness beachten.

### 7. Verifikation
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown` + `cargo check -p blxcode`.
- `scripts/lint_theme_tokens.sh` (keine neuen Farb-Literale).
- Manueller Live-Test im Tauri-Fenster: Stufe wechseln (UI + Terminal-Ecken),
  Font wechseln (UI + Terminal-Glyphen + Refit), Reload-Persistenz, Theme-Wechsel
  behält Roundings/Font.

## Tasks

- [ ] `radius-tokens` - `--radius-scale` + `--radius-{xs,sm,md,lg,xl,pill,circle}` in `tokens.css :root` einführen.
- [ ] `radius-migrate-core` - Häufigste `border-radius`-Literale in `styles.css` auf Tokens mappen (4/6/8/3/12px, 999px→pill, 50%→circle).
- [ ] `radius-migrate-components` - Restliche Komponenten-CSS-Radien (häufige Werte) auf Tokens nachziehen; `0`/`inherit`/Spezialfälle unangetastet.
- [ ] `config-keys` - `RADIUS_SCALE_KEY`/`FONT_FAMILY_KEY` (+ optional `FONT_SIZE_KEY`) in `app.config.rs`.
- [ ] `radius-enum` - `RadiusScale`-Enum (Stufen, Multiplikator, Storage-Mapping).
- [ ] `font-catalog` - `FontChoice`-Liste (id, Anzeigename, vollständiger CSS-Fallback-Stack) als zentrale Quelle.
- [ ] `theme-service-ext` - `ThemeService` um `radius_scale`/`font_choice`(/`font_size`) erweitern: Signale, Setter, `localStorage`, root-`setProperty`, Event-Dispatch, Initial-Apply.
- [ ] `appearance-roundings-ui` - Roundings-Sektion (Segment-Control + Preview-Kacheln) in der Appearance-Pane.
- [ ] `appearance-font-ui` - Font-Picker-Sektion (in-Schrift-Preview, optional Größe) in der Appearance-Pane.
- [ ] `terminal-font-wire` - `terminal_bootstrap.mjs` auf `--font-mono` umstellen + Refit/Re-Measure nach Font-Wechsel.
- [ ] `i18n` - Neue Keys in `keys.rs` + allen 13 Locales (en/de übersetzt).
- [ ] `verification` - `cargo check` (UI + Tauri), Theme-Token-Lint, manueller Live-Test (Roundings/Font/Persistenz/Theme-Wechsel).

## Follow-ups (offen, optional)

- **Advanced-Roundings**: aufklappbarer Pro-Token-Modus (`sm/md/lg/pill` einzeln).
- **Weitere Fonts bündeln** (`@font-face` für Fira Code/Cascadia), damit die Liste
  nicht system-abhängig ist; plus optionaler Freitext-Familie für Power-User.
- **Schriftgröße** als eigenständiges Setting (falls nicht in MVP).
- **`rem`-basierte Radien** (`0.4rem`, `0.5rem`, …) in Tokens überführen.
- **Memory-Graph 3D**: Font/Radius-Reaktion, falls dort Text/Karten gerendert werden.
- Nicht-deutsche i18n-Strings via `render_i18n_locales_from_en.py` nachziehen.
