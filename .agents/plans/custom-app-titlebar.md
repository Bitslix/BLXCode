# Custom Cross-Platform App Title Bar (BLXCode)

## Summary

Replace the OS-default window decorations with a fully custom, cross-platform
BLXCode title bar (Windows/macOS/Linux), modeled on the reference screenshots
(BridgeMind) but branded for **BLXCode**. The bar hosts: the app brand
(logo + "BLXCode" + version badge), a center breadcrumb (`Workspace › context`),
the **left and right sidebar open/close toggles** (moved out of the panels and
into the bar), a **NAVIGATE** quick popover (switch views + create a terminal in
the active workspace + toggle fullscreen), a **Notifications** popover, a Settings
gear, and the **window controls** (minimize / maximize-restore / close).

Today there is no global top bar: `WorkbenchShell` renders `Sidebar` +
`workbench-main` directly under `<main class="workbench-root">`, and the window
uses native decorations (`tauri.conf.json` has no `decorations:false`). The left
collapse toggle lives in `sidebar.rs`; the right collapse toggle lives in
`right_panel.rs` (`workbench-gutter-bar`). Both move into the new bar.

## Decisions

- **Decorations off everywhere.** Set `app.windows[0].decorations: false` in
  `tauri.conf.json` and render our own bar on all platforms for one consistent
  layout. Window controls sit on the **right** (Windows-style) on every OS in v1;
  macOS left-side traffic-light mirroring is an explicit follow-up, not v1 scope.
- **Dragging via `data-tauri-drag-region`.** Tauri intercepts this attribute when
  decorations are off (no Rust round-trip for drag). Double-click on the drag
  region toggles maximize. Requires `core:window:allow-start-dragging` and
  `core:window:allow-internal-toggle-maximize` in capabilities.
- **Min/Max/Close/Fullscreen via Rust commands**, not the JS window API, so the
  privileged calls stay server-side and we avoid widening JS window permissions
  beyond drag. Commands read/operate on the `main` `WebviewWindow`.
- **Single title bar mounted at the App root** so the window is draggable and
  closable during boot and the EULA gate too. Workspace-scoped controls (sidebar
  toggles, breadcrumb, NAVIGATE, Notifications, Settings) render only when the
  `WorkbenchService` context is present (`use_context`, not `expect_context`).
- **Brand, not BridgeMind.** Reuse `/public/blxcode.png` + the literal `BLXCode`
  and the version from the build (start with the hardcoded crate/conf version,
  matching how the app already surfaces `v0.3.3`).
- **Notifications v1 = empty-state popover** ("No notifications yet"), wired to a
  small store so the agent-done/toast feed can populate it later (tagged as a
  follow-up task, not blocking the bar).
- **Component layout follows `rule-no-monolith-structure` + `rule-reusable-components`:**
  one folder `src/workbench/app_titlebar/` with a focused file per piece and one
  co-located CSS file. **Tokens only** per `rule-theme-tokens` — no literal colors.

## Implementation Notes

### Backend (`src-tauri/`)

- New `src-tauri/src/window_controls.rs` with `#[tauri::command]`s operating on the
  `main` window via `AppHandle::get_webview_window("main")`:
  - `window_minimize()`
  - `window_toggle_maximize()` (maximize if not maximized, else unmaximize)
  - `window_is_maximized() -> bool`
  - `window_close()` (best-effort flush already handled by existing `beforeunload`)
  - `window_toggle_fullscreen()` / `window_is_fullscreen() -> bool`
  - Emit a `blxcode://window-state` event (or reuse Tauri's resize events) so the
    maximize/restore icon can react; otherwise poll `is_maximized` on click.
- Register the new commands in `lib.rs` (`tauri::generate_handler![...]`), next to
  `exit_app`.
- `src-tauri/tauri.conf.json`: add `"decorations": false` to the `main` window.
  Note macOS: with decorations off the native traffic lights disappear (expected —
  we draw our own). Keep `transparent:false`.
- `src-tauri/capabilities/default.json`: add `core:window:allow-start-dragging` and
  `core:window:allow-internal-toggle-maximize` (drag + double-click maximize). The
  Rust commands themselves need no JS permission.

### Frontend bridge

- `src/tauri_bridge.rs`: add thin async wrappers `window_minimize`,
  `window_toggle_maximize`, `window_is_maximized`, `window_close`,
  `window_toggle_fullscreen`, `window_is_fullscreen` (same `invoke()` pattern as
  the existing wrappers; guard with `is_tauri_shell()`).

### Title bar components (`src/workbench/app_titlebar/`)

- `mod.rs` — `AppTitleBar`: top-level flex bar. Left cluster (sidebar-left toggle +
  brand), center drag region with breadcrumb (`data-tauri-drag-region`), right
  cluster (NAVIGATE, Notifications, Settings gear, sidebar-right toggle, window
  controls). Resolves `WorkbenchService` via `use_context` for workspace-scoped bits.
- `brand.rs` — logo (`/public/blxcode.png`) + `BLXCode` + version badge.
- `window_controls.rs` — minimize (`LuMinus`), maximize/restore (`LuSquare` /
  restore glyph), close (`LuX`); reactive maximized state from `window_is_maximized`.
- `navigate_menu.rs` — popover toggled by a grid button (`LuLayoutGrid`). Items with
  shortcut hints:
  - Terminal → `wb.open_center_terminals_tab(active_ws)` + focus
  - New terminal → `wb.append_terminal_slot(...)` in the active workspace (explicit
    user request: quick-create a terminal)
  - Plans (board) → `set_right_tab(Plans)` (+ expand if collapsed)
  - Memory → `set_right_tab(Memory)`
  - Skills → `set_right_tab(Skills)`
  - Settings → `open_center_settings_tab(HarnessSettingsCategory::App)`
  - Toggle fullscreen → `window_toggle_fullscreen`
  Closes on outside-click and `Esc`.
- `notifications_menu.rs` — bell button (`LuBell`) + popover. v1 renders the
  empty-state ("No notifications yet / Agents will post here when they finish or
  need input."). Backed by a tiny signal/store for future entries.
- Sidebar toggles in the bar: `LuPanelLeftClose`/`LuPanelLeftOpen` bound to
  `wb.toggle_sidebar()`; `LuPanelRight` bound to `wb.toggle_right_panel()`. Reflect
  `sidebar_collapsed()` / `right_collapsed()` in icon + aria-label.
- `app-titlebar.css` — bar height (~40px), clusters, hover/active states, popover
  surfaces; tokens only. Add `<link data-trunk rel="css" ...>` to `index.html`.

### Shell integration & removing the old toggles

- Mount `<AppTitleBar/>` at the App root (above the boot/EULA/`WorkbenchShell`
  switch) so it persists across boot and the EULA gate.
- Wrap the shell in a vertical flex: title bar (fixed) + body; set
  `workbench-root`/`app-shell` height to `100vh - var(--titlebar-h)` (introduce a
  `--titlebar-h` token/value). Verify terminal `xterm.fit()` still gets correct
  dimensions (there is existing 0×0 handling — re-check after the height change).
- `sidebar.rs`: remove the in-panel expand/collapse button (the bar now owns it);
  keep the collapsed-state "+ add workspace" affordance and workspace list. Adjust
  the sidebar header layout so nothing looks orphaned.
- `right_panel.rs`: remove the `workbench-right-panel-toggle` from the
  `workbench-gutter-bar` (bar owns it); keep the tab rail. Adjust gutter layout.
- i18n: add titlebar tooltip/menu keys to `keys.rs` and **all 14** locale files
  (compile-time exhaustiveness) — reuse existing keys (`SbCollapse`, `SbExpand`,
  `RpCollapse`, `RpExpand`, `CmdSetTitle`, tab labels) where they already fit.

### Platform nuances

- Windows: snap-assist on the maximize button is lost with custom controls
  (acceptable v1). Ensure controls have adequate hit-targets.
- macOS: no native traffic lights when decorations are off; our controls on the
  right are the only set in v1. Note potential follow-up for left placement.
- Linux: `data-tauri-drag-region` works on common WMs given the added
  `allow-start-dragging` permission; verify on the dev target.

## Tests

- Frontend: `cargo check -p blxcode-ui --target wasm32-unknown-unknown` clean.
- Backend: `cargo check -p blxcode` and `cargo test --workspace` clean; capabilities
  schema validates (build succeeds).
- Manual (desktop app):
  - Drag the window by the bar; double-click toggles maximize/restore.
  - Minimize, maximize/restore, close all work; restore icon reflects state.
  - Fullscreen toggles (and the NAVIGATE item) work and restore correctly.
  - Left toggle collapses/expands the sidebar; right toggle the right panel; no
    duplicate toggles remain in the panels; icons/aria reflect state.
  - NAVIGATE: Terminal focuses/opens the terminals tab; New terminal adds a slot in
    the active workspace; Plans/Memory/Skills switch the right panel; Settings opens
    the settings center tab; popover closes on outside-click/Esc.
  - Notifications popover opens with the empty state and closes cleanly.
  - Brand shows the BLXCode logo + name + version — **never** BridgeMind.
  - Bar renders and the window stays draggable/closable during boot and the EULA
    gate.
  - Verify across at least two themes (dark + light) that only tokens are used.

## Tasks

- [x] `titlebar-config` - tauri.conf.json `decorations:false` + capabilities window perms (start-dragging, internal-toggle-maximize)
- [x] `titlebar-window-cmds` - Backend `window_controls.rs` (minimize/toggle_maximize/is_maximized/close/toggle_fullscreen/is_fullscreen) registered in lib.rs
- [x] `titlebar-bridge` - tauri_bridge.rs JS wrappers for the window commands
- [ ] `titlebar-shell` - `app_titlebar/mod.rs` AppTitleBar layout + drag region, mount at App root, CSS scaffold + index.html link
- [ ] `titlebar-brand` - `brand.rs`: BLXCode logo + name + version badge
- [ ] `titlebar-window-controls` - `window_controls.rs` UI: minimize/maximize-restore/close with reactive maximized state
- [ ] `titlebar-sidebar-toggles` - Left/right sidebar toggles in the bar bound to toggle_sidebar/toggle_right_panel (context-gated)
- [ ] `titlebar-remove-old-toggles` - Remove the old collapse button from sidebar.rs and the right-panel toggle from right_panel.rs; adjust layouts
- [ ] `titlebar-breadcrumb` - Center breadcrumb (workspace name › active center tab) in a draggable region
- [ ] `titlebar-navigate-menu` - `navigate_menu.rs` popover: Terminal, New terminal, Plans, Memory, Skills, Settings, Toggle fullscreen + shortcut hints, outside-click/Esc close
- [ ] `titlebar-notifications-menu` - `notifications_menu.rs` popover with empty state + bell button, backed by a feed store stub
- [ ] `titlebar-settings-gear` - Settings gear button opening the settings center tab
- [ ] `titlebar-layout-css` - App-shell vertical flex, `--titlebar-h` offset, terminal fit re-check, tokens-only styling
- [ ] `titlebar-boot-eula` - Ensure the bar (drag + window controls) is present during boot and the EULA gate
- [ ] `titlebar-i18n` - Title bar tooltip/menu i18n keys across keys.rs + all 14 locale files (reuse existing keys where they fit)
- [ ] `titlebar-platform` - Verify platform nuances (Windows hit-targets, macOS no-traffic-lights, Linux drag) on the dev target
- [ ] `titlebar-notifications-feed` - Follow-up: wire real notifications (agent-done/needs-input, toasts) into the popover store
- [ ] `titlebar-verify` - cargo check (wasm) + cargo check/test (backend) + the manual cross-platform checklist above
