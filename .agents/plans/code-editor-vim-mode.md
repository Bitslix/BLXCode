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

Additionally, add a new group of **file editor / preview shortcuts** to
Settings → Shortcuts (rebindable like the existing actions). These are **disabled
while Vim mode is active** (Vim owns the keymap) — the shortcut rows render
disabled with an **inline hint** pointing to the Code Editor → Vim toggle.

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

### Editor/preview shortcuts (Settings → Shortcuts)
- **Separate action set** from the global `ShortcutAction` enum: a new
  `EditorShortcutAction` enum (own concern — these only fire when the editor is
  focused and are dispatched through CodeMirror, not the global harness handler).
- **Dispatch site = CodeMirror keymap**, not the global key handler: editor commands
  (save, find, replace, go-to-line, toggle comment, fold/unfold, move/duplicate
  line, format/indent) map to CM6 commands already shipped by `basicSetup` +
  `@codemirror/commands`/`@codemirror/search`. The bindings are passed into the
  editor at create time and live in their **own CM `Compartment`**.
- **Vim gating**: when Vim is enabled, the editor-shortcuts keymap compartment is
  reconfigured to **empty** (Vim owns the keys) — same live-reconfigure mechanism
  as the vim compartment, no remount. In the Shortcuts pane the whole editor group
  renders **disabled** with an inline hint linking to the Code Editor Vim toggle.
- **Rebinding** reuses the existing `ShortcutsSettingsPane` capture flow and the
  `KeyChord`/`Binding::Combo` model; editor actions are always direct combos
  (never tmux chords). Persisted via `AppPrefsService` as a second JSON map
  (`editor_bindings`), separate localStorage key, defaults independent of the
  tmux/legacy preset.
- **Default editor combos** (illustrative, finalize in impl, avoid clobbering CM
  defaults): Save `Mod-s` (already wired), Find `Mod-f`, Replace `Mod-Alt-f`,
  Go to Line `Ctrl-g`, Toggle Comment `Mod-/`, Fold `Ctrl-Shift-[`, Unfold
  `Ctrl-Shift-]`, Move Line Up/Down `Alt-Up`/`Alt-Down`, Duplicate Line
  `Mod-Shift-d`, Format/Indent `Mod-Alt-l`.

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

### Editor/preview shortcuts
- `src/workbench/shortcut_config.rs` (or a sibling `editor_shortcut_config.rs` to
  honor no-monolith): `EditorShortcutAction` enum (`Save`, `Find`, `Replace`,
  `GoToLine`, `ToggleComment`, `Fold`, `Unfold`, `MoveLineUp`, `MoveLineDown`,
  `DuplicateLine`, `Format`) with `ALL`, `label_key()`, `default_combo()`, and a
  stable string `id()` used as the JS command key. Map each id → CM command name
  in `cm-entry.js`.
- `src/config/app.config.rs`: `EDITOR_SHORTCUT_BINDINGS_KEY = "blxcode_editor_shortcut_bindings_v1"`.
- `AppPrefsService`: add `editor_bindings: RwSignal<BTreeMap<EditorShortcutAction, KeyChord>>`
  loaded from/saved to localStorage; `set_editor_binding`, `reset_editor_binding`,
  `reset_all_editor_bindings`. Mirror the existing shortcut persistence helpers.
- `cm-entry.js`: accept `o.editorKeymap` (array of `{ key, command }`); build a
  CM keymap mapping command ids → imported commands
  (`@codemirror/commands`: `toggleComment`, `moveLineUp`, `moveLineDown`,
  `copyLineDown`, `indentSelection`; `@codemirror/search`:
  `openSearchPanel`, `gotoLine`, `findNext`/replace panel). Put it in its own
  `Compartment`; `setEditorKeymap(view, list)` reconfigures live. When vim on,
  pass/reconfigure to `[]`.
- `codemirror_glue.rs`: pass `editor_keymap` into `create_editor`; add
  `set_editor_keymap(view, &JsValue)` (serialize the binding list to a JS array).
- `code_mirror.rs`: build the keymap list from `AppPrefsService.editor_bindings`
  (only when Vim is **off**); reactive `Effect` rebuilds it when bindings change or
  vim toggles (`set_editor_keymap` / `set_vim` together).
- `ShortcutsSettingsPane`: new section "File Editor / Preview" listing
  `EditorShortcutAction::ALL` with the same `ActionRow` capture/rebind/reset UX
  (generalize `ActionRow` or add an `EditorActionRow`, preferring reuse per
  reusable-components rule). When `EditorSettingsService.vim_enabled` is true,
  render the section disabled (rebind/reset buttons disabled, rows dimmed) plus an
  inline hint banner ("Disabled while Vim mode is active") with a pointer to the
  Code Editor settings. Theme-token styling only.

### i18n
- Add keys to `src/i18n/keys.rs` and **every** `src/i18n/locales/*.rs`
  (compile-time exhaustiveness): `HsCatCodeEditor`, `CodeEditorHeading`,
  `CodeEditorDescription`, `CodeEditorVimTitle`, `CodeEditorVimDesc`,
  `CodeEditorVimStatusTip`, plus the editor-shortcuts section
  (`ShortcutsEditorHeading`, `ShortcutsEditorVimDisabledHint`, and one label per
  `EditorShortcutAction`, e.g. `EdKwSave`, `EdKwFind`, `EdKwReplace`,
  `EdKwGoToLine`, `EdKwToggleComment`, `EdKwFold`, `EdKwUnfold`, `EdKwMoveLineUp`,
  `EdKwMoveLineDown`, `EdKwDuplicateLine`, `EdKwFormat`). Use
  `scripts/render_i18n_locales_from_en.py` to fill non-English tables after
  `en_us.rs`.

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
- Manual: with Vim **off**, each editor shortcut fires in editor and (where
  applicable) preview; rebinding one persists and takes effect live.
- Manual: enabling Vim disables the editor shortcuts live (keys go to Vim) and the
  Shortcuts pane shows the editor group dimmed with the inline hint; disabling Vim
  re-enables them.
- Unit: `EditorShortcutAction` defaults/serde round-trip (`from_json`/`to_json` of
  `editor_bindings`), like the existing `shortcut_config` tests.
- Build: `bun run build` produces an updated `codemirror.min.js`.
- Final: `cargo check -p blxcode-ui --target wasm32-unknown-unknown` and
  `cargo check -p blxcode` pass.

## Tasks

- [>] `vim-bundle` - Add @replit/codemirror-vim to bundle, wire compartment + setVim in cm-entry.js, rebuild (commit)
- [ ] `vim-glue` - Extend codemirror_glue: vim param in create_editor + set_vim helper (commit)
- [ ] `editor-settings-service` - Add CODE_EDITOR_VIM_KEY + EditorSettingsService, provide in app.rs, re-export (commit)
- [ ] `editor-vim-bind` - CodeMirrorEditor reads service, passes vim, live Effect → set_vim (commit)
- [ ] `code-editor-settings-tab` - New CodeEditor category + tab button/routing + code_editor_settings_pane (header + vim switch) + i18n keys (commit)
- [ ] `vim-status-indicator` - VimStatusIndicator in status bar left slot, gated on vim_enabled && active_editor_status, theme-token CSS + i18n tooltip (commit)
- [ ] `editor-shortcut-model` - EditorShortcutAction enum (ALL/labels/defaults/id), EDITOR_SHORTCUT_BINDINGS_KEY, AppPrefsService editor_bindings persistence + serde test (commit)
- [ ] `editor-shortcut-keymap` - cm-entry editorKeymap compartment + command map, glue set_editor_keymap + create_editor param, CodeMirrorEditor builds/reconfigures keymap (empty when vim on) (commit)
- [ ] `editor-shortcut-pane` - Shortcuts pane "File Editor / Preview" section (reuse ActionRow), disabled+inline-hint when vim active, i18n keys (commit)
- [ ] `final-checks` - Run cargo check for both crates + bundle build, fix fallout, final commit
