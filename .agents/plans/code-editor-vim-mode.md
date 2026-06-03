# Code Editor Vim Mode + Code Editor Settings Tab

## Summary

Add built-in Vim key control to the in-app CodeMirror 6 surface — used by both the
file **editor** (edit mode) and the file **preview** (read-only). Add a new
**"Code Editor"** settings tab (same header/style as existing panes) whose first
control is a switch to enable/disable Vim mode (default **on**); the tab is built
to host further code-editor settings later. Show a **Vim indicator icon** in the
status bar's **left** slot, visible only when Vim is enabled **and** the user is
actively in a file editor/preview.

Editor and preview share the same `CodeMirrorEditor` component, so Vim applies to
both. In preview (`EditorState.readOnly`) Vim motions/search work for navigation
while edit commands stay blocked.

## Decisions

- **Vim package**: `@replit/codemirror-vim` (the standard CM6 vim implementation),
  added to the vendored bundle (`scripts/codemirror-bundle/`).
- **Live toggle, no remount**: Vim is wrapped in a CodeMirror `Compartment` so it
  can be enabled/disabled via `view.dispatch({ effects: compartment.reconfigure(...) })`
  without re-mounting the editor (otherwise the user would need to reopen the file).
- **Vim ordering**: `vim()` must precede `basicSetup` in the extension list (CM6 vim
  requirement).
- **Persistence**: localStorage via a dedicated `EditorSettingsService`, mirroring
  the existing `ThemeService` pattern; key in `app.config.rs`. Default = enabled.
- **"Actively in editor/preview" signal**: reuse `CoreStatusService::active_editor_status`
  (already returns `Some` only for `CenterTabKind::FilePreview`).
- **Preview gets Vim too** (navigation), since editor and preview share the component.
- Follows active rules: branch-and-phase-commits, no-monolith-structure
  (own module folder per pane/service), reusable-components, theme-tokens (status
  indicator + pane styled only via theme CSS vars), use-enabled-rust-skills.

## Implementation Notes

### JS bundle (`scripts/codemirror-bundle/`)
- Add `@replit/codemirror-vim` to `package.json`; `bun install`.
- `cm-entry.js`: import `{ vim }`; create a `Compartment` for the vim extension;
  in `create()` read `o.vim` and seed the compartment (`o.vim ? vim() : []`) placed
  **before** `basicSetup`. Export `setVim(view, enabled)` that reconfigures the
  compartment live. Rebuild via `bun run build` → `public/vendor/codemirror/codemirror.min.js`.

### Rust glue (`src/workbench/file_preview/codemirror_glue.rs`)
- Extend `create_editor(...)` with `vim: bool` (set `opts.vim`).
- Add `set_vim(view: &JsValue, enabled: bool)` analogous to `set_doc`.

### Editor settings service (new module)
- `src/config/app.config.rs`: `CODE_EDITOR_VIM_KEY = "blxcode_code_editor_vim_v1"`.
- `src/workbench/editor_settings_service.rs`: `#[derive(Clone, Copy)] EditorSettingsService`
  with `vim_enabled: RwSignal<bool>`; `new()` loads from localStorage (default `true`),
  `set_vim_enabled(bool)` writes back. Reuse the existing `read_string`/`write_string`
  helper pattern (extract/share if practical). Provide via `provide_context` in
  `app.rs`; re-export from `workbench/mod.rs`.

### Editor component (`src/workbench/file_preview/editor/code_mirror.rs`)
- `expect_context::<EditorSettingsService>()`; pass `vim_enabled.get_untracked()`
  into `create_editor`.
- `Effect` watching `vim_enabled` → `cm::set_vim(view, enabled)` on the live view
  (guarded on an existing `view_handle`). Applies to edit + read-only.

### Settings tab (new module + CSS)
- `state.rs`: add `HarnessSettingsCategory::CodeEditor`; cover it in `settings_tab_title`.
- `harness_ui.rs`: icon in `harness_settings_cat_icon` (`icondata::LuCode`); add
  `HarnessCatBtn` (alphabetically) and a routing arm rendering the new pane.
- `src/workbench/code_editor_settings_pane/mod.rs` + `.css`: `SettingsPaneHeader`
  + one switch sub-control ("Vim key bindings", default on) bound to
  `EditorSettingsService`. Structured like an `appearance-control`/`appearance-subcontrol`
  section so further settings drop in cleanly. Re-export from `workbench/mod.rs`.

### Status bar indicator (`src/app.rs` left slot)
- New `VimStatusIndicator` component: show only when
  `EditorSettingsService.vim_enabled` is true **and**
  `CoreStatusService::active_editor_status(wb).is_some()`. Render a vim icon
  (icondata, else "VIM" badge) with i18n tooltip. Style via theme tokens
  consistent with other `app-statusline__item`s.

### i18n
- Add keys to `src/i18n/keys.rs` and **every** `src/i18n/locales/*.rs`
  (compile-time exhaustiveness): `HsCatCodeEditor`, `CodeEditorHeading`,
  `CodeEditorDescription`, `CodeEditorVimTitle`, `CodeEditorVimDesc`,
  `CodeEditorVimStatusTip`. Use `scripts/render_i18n_locales_from_en.py` to fill
  non-English tables after `en_us.rs`.

## Commit / verification discipline

- A commit is made **after every phase** (rule-branch-and-phase-commits).
- To **save energy, `cargo check` runs only at the very end** (Phase 7), not per phase.

## Tests

- Manual: open a code file in edit mode → vim normal/insert/visual + `:w`-style save
  via existing `Mod-s` still works; `hjkl`, `/search`, `dd`, `yy`, `p` behave.
- Manual: open a file in preview (read-only) → vim navigation/search work, edits blocked.
- Manual: toggle the Code Editor → Vim switch off/on → editor updates **live**
  (no reopen); setting persists across app restart (localStorage).
- Manual: status bar left slot shows the vim icon only while a file editor/preview
  tab is active and Vim is on; hides on other tabs and when Vim is off.
- Build: `bun run build` produces an updated `codemirror.min.js`.
- Final: `cargo check -p blxcode-ui --target wasm32-unknown-unknown` and
  `cargo check -p blxcode` pass.

## Tasks

- [ ] `vim-bundle` - Add @replit/codemirror-vim to bundle, wire compartment + setVim in cm-entry.js, rebuild (commit)
- [ ] `vim-glue` - Extend codemirror_glue: vim param in create_editor + set_vim helper (commit)
- [ ] `editor-settings-service` - Add CODE_EDITOR_VIM_KEY + EditorSettingsService, provide in app.rs, re-export (commit)
- [ ] `editor-vim-bind` - CodeMirrorEditor reads service, passes vim, live Effect → set_vim (commit)
- [ ] `code-editor-settings-tab` - New CodeEditor category + tab button/routing + code_editor_settings_pane (header + vim switch) + i18n keys (commit)
- [ ] `vim-status-indicator` - VimStatusIndicator in status bar left slot, gated on vim_enabled && active_editor_status, theme-token CSS + i18n tooltip (commit)
- [ ] `final-checks` - Run cargo check for both crates + bundle build, fix fallout, final commit
