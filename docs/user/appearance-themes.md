# Appearance & Themes

BLXCode ships **30 app themes** (15 dark + 15 light). Colors apply across the workbench — sidebar, settings, terminals, memory graph, and agent panels — via a shared token system.

## Open the theme picker

1. Open **Settings** (command palette → **Open Settings**, or your configured shortcut).
2. Select **Appearance** in the left sidebar.

The pane shows:

- A **hero preview** of the active theme (top right)
- A **search** field (filters by translated name and description)
- **All / Dark / Light** filters with counts (`All (30)`, `Dark (15)`, `Light (15)`)
- A **grid of theme cards** with mini layout previews

Click a card to apply the theme immediately. The active card shows an **ACTIVE** badge and accent border.

<p align="center">
  <img src="../images/settings-appearance-themes.png" alt="Settings → Appearance pane titled 'Make it yours.' with 20 theme cards in a 4-column grid: BLXCode (active), BLXCode Light, Dracula, Gruvbox Dark/Light, Solarized Dark/Light, Nord, One Dark, Catppuccin Mocha/Latte/Frappé, Tokyo Night, Rosé Pine, Rosé Pine Dawn, Everforest Dark, Kanagawa, GitHub Dark, Night Owl, Ayu Mirage" />
</p>

## Default theme

**BLXCode** (`blxcode-dark`) is the default — the same deep dark look BLXCode used before themes shipped. First launch and a cleared `localStorage` always fall back to this theme.

## Available themes

| Theme | Mode |
|-------|------|
| **BLXCode** | Dark |
| BLXCode Light | Light |
| Dracula | Dark |
| Gruvbox Dark / Light | Dark / Light |
| Solarized Dark / Light | Dark / Light |
| Nord | Dark |
| Nord Light | Light |
| One Dark | Dark |
| One Light (Atom) | Light |
| Catppuccin Mocha / Latte / Frappé | Dark / Light / Dark |
| Tokyo Night | Dark |
| Tokyo Night Light | Light |
| Rosé Pine | Dark |
| Rosé Pine Dawn | Light |
| Everforest Dark | Dark |
| Kanagawa | Dark |
| Claude Code | Dark |
| Night Owl | Dark |
| Ayu Mirage | Dark |
| Ayu Light | Light |
| GitHub Light | Light |
| Winter Light | Light |
| Paper Light | Light |
| Alpine Light | Light |
| Frost Light | Light |
| Lilac Light | Light |

> **Claude Code** is a warm-charcoal dark theme modeled on the Claude Code CLI with a coral accent. It replaces the older `github-dark` slot, whose blue tones overlapped the default BLXCode dark theme.

Theme names and descriptions follow your **Settings → App → Language** choice.

## Persistence

The selected theme is stored in browser `localStorage` under `blxcode_theme_v1` and restored on reload. A small inline script in `index.html` applies the saved theme before CSS loads to avoid a flash of the wrong colors.

## What themes do not change

Some surfaces are intentionally outside the theme selector:

- **Embedded browser page content** (Linux iframe) — only the app chrome around the page follows the theme.
- **Native child webviews** on Windows/macOS — outside SPA styling.
- **Memory category swatches** you set under Workspace → Category colors — user data, not app chrome.
- **Flag icons** in the language picker — national colors stay accurate.

See [Theme exceptions](../THEME_EXCEPTIONS.md) for the full list.

## See also

- [Settings](settings.md) — all settings categories
- [UI Language](language.md) — locale picker (App tab)
