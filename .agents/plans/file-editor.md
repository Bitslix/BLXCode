# Implementation Plan: File Preview → Lightweight Code Editor

**Status:** Plan only — not yet implemented.
**Branch:** `feat/fileeditor`
**Goal:** Extend the existing File Browser / file-preview flow so supported text/code/config
files can be **edited and saved** in-app, while fully **reusing the existing highlight.js
highlighting pipeline**. Add VS Code-style folding, a read-only-by-default policy for important
docs, and a safe save/conflict workflow — all aligned with current BLXCode conventions and the
binding rules in `.agents/rules/`.

> This plan was written after reading the actual code. Key fact: there is **no** `workspace.rs`
> or `file_diff_viewer.rs`. The real preview lives in `src/workbench/file_preview/` and the real
> backend filesystem layer is `src-tauri/src/fs_entries.rs`.

---

## 0. TL;DR

- **Reuse highlight.js as-is.** Edit mode = a transparent `<textarea>` overlaid on the existing
  highlighted, line-numbered backdrop produced by `code_view.rs` + `hljs_glue::highlight` +
  `util::split_highlighted_into_lines`. No Monaco, no CodeMirror (see §2 for the analysis + escape
  hatch). Re-highlight the backdrop on a debounce while typing.
- **Edit toggle, not a new tab type.** The center already uses tabs (`CenterTabKind::FilePreview {
  rel_path }`). Add a per-open-document view/edit mode + dirty state inside the existing preview
  components; keep one file per tab.
- **Backend: add a write + a richer read.** Extend `read_workspace_text_file` to return
  `modified_ms` + a content `hash`, and add `write_workspace_text_file` (atomic temp+rename,
  conflict guard, protected-dir denylist) in `fs_entries.rs`, registered in `lib.rs`. Local +
  remote(SSH) variants, same as the existing commands.
- **Read-only-by-default docs** map onto the existing `PolicyKind` (README/LICENSE/CONTRIBUTING/…):
  these already render through `MarkdownView`; add an **Edit** button that swaps to the raw editor.
- **Folding** is a custom, language-aware fold-range model computed in Rust, reusing the per-line
  row structure CodeView already renders; view-mode folding first, edit-mode folding phased.

---

## 1. Current architecture investigation (verified)

### 1.1 Frontend preview (`src/workbench/file_preview/`)

- **`mod.rs`** — `FilePreviewDock(workspace_id: u64, rel_path: String)`:
  - Calls `stat_workspace_file(root, rel, conn)` → `FileMeta { name, rel_path, byte_len,
    modified_ms, kind: FileKind, mime, policy_kind: Option<PolicyKind> }`.
  - Renders `FilePreviewHeader` (the title bar) + dispatches via `render_for_kind` to
    `ImageView` / `VideoView` / `MarkdownView` / `MermaidView` / `CodeView` / `UnsupportedView`
    (binary). Owns a `reload_tick` for refresh.
- **`code_view.rs`** — `CodeView(workspace_id, rel_path, reload_tick)`:
  - Reads via `read_workspace_text_file` → `TextFilePreview { content, truncated, byte_len }`.
  - `lang_for_path(rel_path)` → `Option<&'static str>` (hljs alias), incl. `Dockerfile`/`Makefile`
    special cases, else `util::hljs_lang_for_ext`.
  - `prepare_lines(content, lang)` → `(Vec<String> html_lines, Vec<String> plain_lines,
    Option<lang>)`. Highlights with `hljs_glue::highlight(content, lang)` then
    `split_highlighted_into_lines`; on error falls back to escaped plain text.
  - `render_code(...)` renders a gutter + one `<div class="code-view__row">` per line with
    `<span class="code-view__line" inner_html=html/>`, plus click/drag **row selection** and a
    **context menu** (snippet → terminal / agent / clipboard). Truncation notice banner via
    `<Show when=truncated>`. **Read-only today.**
- **`hljs_glue.rs`** — `ensure_hljs_loaded()` lazily injects the vendored bundle
  (`/public/vendor/highlight/highlight.min.js`, the *common* bundle); `highlight(code, language)
  -> Result<String,String>` calls `hljs.highlight(code,{language, ignoreIllegals:true})` and
  returns highlighted HTML.
- **`util.rs`** — `hljs_lang_for_ext` (broad extension→alias map), `html_escape`,
  `split_highlighted_into_lines` (span-balancing per line, UTF-8 safe, **tested**),
  `sanitize_markdown_html`, `sanitize_svg`, `format_bytes`, `format_mtime`, `icon_for_kind`,
  `build_file_snippet_block`. Has a `#[cfg(test)]` suite (lang map, escaping, line splitting).
- **`header.rs`** — `FilePreviewHeader(meta: Memo<Option<FileMeta>>, rel_path, on_refresh)`:
  name + icon, relative path (verbatim, `title=`), size + mtime chips, **Copy path** and
  **Refresh** buttons (`workbench-mini-btn`). **This is the title bar to extend.**
- **`markdown_view.rs`** — `MarkdownView(workspace_id, rel_path, reload_tick, policy_kind)`:
  renders markdown (pulldown-cmark) → sanitized HTML, inline Mermaid, and a **PolicyKind hero
  banner** (License/Contributing/Security/Readme/Support/…). Important docs land here.

### 1.2 Explorer + tab plumbing

- **`src/workbench/project_explorer/mod.rs`** — sidebar tree. A **file row single-click** calls
  `wb.open_center_file_tab(ws.id, rel_path)`; new-file draft also opens the tab on create. Folders
  toggle expand. There is **no** double-click handling today.
- **`src/workbench/state.rs`** — `WorkbenchService`, `WorkspaceEntry` with `center_tabs:
  Vec<CenterTab>` and `CenterTabKind { Terminals, Settings, FilePreview { rel_path }, FileDiff {…} }`.
  Tabs already exist and persist; `center_active_tab_id` tracks the active one. Workspace switch =
  `active_id` change. Snapshot auto-saves (debounced) and on `beforeunload`.

### 1.3 Backend filesystem (`src-tauri/src/fs_entries.rs`)

- Sandbox helpers: `canonical_root` (canonicalizes the root, must be a dir); `resolve_under_root`
  (joins, then **`fs::canonicalize` + `starts_with(root)`** — symlink-safe for existing targets);
  `resolve_new_under_root` + `ensure_under_root` (reject `..`/absolute, then canonicalize the
  created parent and re-check containment) for create operations.
- Commands (each with a **local** and a **remote/SSH** variant chosen by `connection_id`):
  `list_path_entries`, `create_workspace_file`, `create_workspace_dir`,
  `read_workspace_text_file` → `TextFilePreview { content, truncated, byte_len }`
  (`MAX_TEXT_PREVIEW_BYTES = 512 KiB`, rejects non-UTF-8 with "file is not valid UTF-8 text"),
  `stat_workspace_file` → `FileMeta`, `read_workspace_image_file`, `read_workspace_video_file`.
- `FileKind { Image, Video, Markdown, Mermaid, Code, Text, Binary }`, `PolicyKind { License,
  Contributing, Contributors, CodeOfConduct, Security, Authors, Changelog, Readme, Support,
  Agents, Claude, Codex, Gemini }`, `classify_kind(ext)`, `classify_policy(stem)`.
- **No write command, no mtime/hash in `TextFilePreview`, no explicit binary sniff** (binary is
  inferred by extension via `FileKind::Binary` → `UnsupportedView`, or by UTF-8 failure on read).
- Remote sandbox is intentionally weaker (string rules + shell guards, no `canonicalize`),
  documented + accepted in-code.
- Strong `#[cfg(test)]` suite already present (under-root reads, outside-root rejection, traversal
  rejection, policy classification, create dedupe, `remote_target` sandboxing).

### 1.4 Bridge (`src/tauri_bridge.rs`)

Typed wrappers mirror the backend: `read_workspace_text_file -> TextFilePreview`,
`stat_workspace_file -> FileMeta`, `create_workspace_file/dir`, `list_path_entries`, etc. All take
`(workspace_root, path, connection_id: Option<String>)`. `FileKind`/`PolicyKind`/`FileMeta`/
`TextFilePreview` are mirrored here. Generic `invoke_typed` / `invoke_unit_js`.

### 1.5 i18n (`src/i18n/`)

- `keys.rs` — `#[allow(dead_code)] enum I18nKey` (compile-time exhaustive). Existing namespaces
  include `FilePreview*`, `SbExplorer*`, `CodeViewToast*`, `Btn*`.
- `locales/*.rs` — **13 locales**: `de_de, en_us (source), es_es, fr_fr, hu_hu, it_it, ja_jp,
  ko_kr, pl_pl, pt_br, ru_ru, zh_cn, zh_tw`. Each is an exhaustive `match` returning `&'static str`.
- Usage: `i18n.tr(I18nKey::X)` returns a closure; call `i18n.tr(X)()`. Placeholders via
  `.replace("{name}", ..)`. Regenerate non-English with
  `scripts/tools/render_i18n_locales_from_en.py`.

### 1.6 Command registration (`src-tauri/src/lib.rs`)

`fs_entries::*` commands are registered in the single `invoke_handler!` list. `lib.rs` holds no
business logic (architecture rule). `PtyManager` + `RemoteExecManager` are managed state used by
the remote FS variants. App exit kills PTYs on `ExitRequested`/`Exit`.

### 1.7 Binding rules (`.agents/rules/`) that constrain this work

i18n (every string via `I18nKey` in all 13 locales; paths shown verbatim, never translated),
security (sandbox all FS to the workspace root; reject `..`/absolute/symlink escape; explicit
size + binary handling), testing (every command gets happy + escape `#[cfg(test)]` tests with
temp dirs; pure frontend logic tested too; run `cargo test --workspace`), ui-design (dark theme,
reuse components/CSS in `styles.css`, keyboard-accessible, no external UI frameworks), ui-text
(English source, sentence case, actionable errors), architecture (frontend↔backend only via
`tauri_bridge.rs`; mirror IPC types; thin commands), rust (no `unwrap/expect` outside tests/init;
typed errors; small modules), code-quality + workflow (fmt/clippy clean; update `CHANGELOG.md`;
Conventional Commits). Re-read each rule before implementing.

### 1.8 Open items to confirm during implementation

- [ ] Confirm `wb.open_center_file_tab` reuses an existing tab for the same `rel_path` or always
  opens a new one (affects "open while dirty" handling, §8.6).
- [ ] Confirm whether `CodeView` is mounted with `keep-alive` across tab switches or remounted
  (affects where the edit buffer lives — must survive tab switches without losing edits).
- [ ] Confirm `styles.css` class hooks available for an overlay textarea aligned to
  `.code-view__row`/`.code-view__line` metrics.

---

## 2. Proposed editor approach (decision + rationale)

### 2.1 Options

| Option | Highlighting | Folding | New deps | Fit |
|---|---|---|---|---|
| **A. `<textarea>` over the existing highlighted backdrop** *(recommended)* | reuses `hljs_glue::highlight` | custom fold model | none | ✅ reuses the whole pipeline; no UI framework |
| B. CodeMirror 6 (edit mode only) | CodeMirror's own (Lezer) | built-in | large JS dep | ⚠️ second highlighter; tension with "no external UI frameworks" |
| C. Monaco | Monaco's own | built-in | very large | ❌ heavy, CSP/bundle cost |

### 2.2 Recommendation: Option A

A transparent `<textarea>` is layered exactly over the existing line-rendered, hljs-highlighted
backdrop that `code_view.rs::render_code` already produces. The textarea carries the caret, IME,
native undo/redo, and selection; its text is transparent (`color: transparent; caret-color:
var(--…)`), so the user sees the highlighted backdrop underneath. On a debounced input, re-run
`prepare_lines(buffer, lang)` to refresh the backdrop.

Why this fits BLXCode specifically:
- **Hard requirement #2 is met literally** — the same `hljs_glue::highlight` +
  `split_highlighted_into_lines` produce the backdrop in both view and edit mode. View mode is
  unchanged.
- Honors `ui-design.md` ("no external UI frameworks", "reuse existing CSS"). Reuses
  `.code-view` gutter/row CSS.
- Zero new bundle weight or CSP change; works in the existing Trunk/WASM build.
- The line-based backdrop already exists, so the textarea overlay is the only new surface.

Costs / mitigations (details in §9, §15):
- Re-highlight cost on each keystroke → debounce (~80–120 ms idle); skip highlighting above a size
  threshold (the read path already exposes `truncated`/`byte_len`); never re-highlight per char.
- Backdrop/textarea **pixel alignment** must be exact (font, line-height, padding, tab-size,
  scroll sync). Main CSS task.
- **Folding while editing** is the genuinely hard part — phased; view-mode folding ships first.

### 2.3 Escape hatch

If edit-mode folding fidelity proves unacceptable after §9 Phase 2, adopt **CodeMirror 6 for edit
mode only**, keeping hljs for the read-only `CodeView`/`MarkdownView`. Documented as fallback, not
default (introduces a second highlighter). Decision gate in §15.

---

## 3. Required frontend changes

Keep changes inside `src/workbench/file_preview/` and add a small editor submodule, rather than a
new top-level feature (least disruption; reuses the dispatcher + header + hljs glue).

### 3.1 New/changed modules

- `file_preview/editor/mod.rs` — `EditableCodeView` (or extend `CodeView` with a `mode` signal).
  Hosts the view backdrop + the edit overlay + the fold gutter.
- `file_preview/editor/buffer.rs` — pure buffer/dirty/conflict model (`#[cfg(test)]`).
- `file_preview/editor/policy.rs` — pure editability policy (`#[cfg(test)]`), see §5/§6.
- `file_preview/editor/folding.rs` — pure fold-range computation (`#[cfg(test)]`), see §9.
- `file_preview/code_view.rs` — refactor `render_code` so the highlighted line backdrop is
  reusable by both read-only and edit mode; add the overlay textarea + fold gutter in edit mode.
- `file_preview/header.rs` — extend `FilePreviewHeader` with editor actions + status (see §7).
- `file_preview/markdown_view.rs` — add an **Edit** affordance that switches a policy/markdown doc
  into the raw editor (reuses `EditableCodeView` with `lang = markdown`).

### 3.2 Open-document state

Hold the edit state where it survives tab switches. Options: (a) per-tab state stored on
`WorkspaceEntry`/`WorkbenchService` keyed by `rel_path`; (b) component-local if CodeView is kept
alive. Confirm §1.8 then choose; **prefer service-held state** keyed by `(workspace_id, rel_path)`
so an accidental remount can't drop edits:

```rust
struct OpenDoc {
    disk_text: String,            // baseline for dirty + revert + conflict
    buffer: RwSignal<String>,     // editable text
    mode: RwSignal<EditMode>,     // View | Edit
    editability: Editability,     // Editable | ReadOnlyByDefault | NeverEdit (from policy)
    load_modified_ms: Option<i64>,
    load_hash: String,
    dirty: Memo<bool>,            // buffer != disk_text
    folds: RwSignal<FoldState>,
    status: RwSignal<DocStatus>,  // Loading | Ready | Error | TooLarge(truncated) | Saving | Saved | Conflict
}
enum EditMode { View, Edit }
enum Editability { Editable, ReadOnlyByDefault, NeverEdit }
```

`dirty` is derived, never a manual flag.

### 3.3 Bridge additions (`src/tauri_bridge.rs`)

Mirror new backend types and add wrappers (the only path to the backend, architecture rule):
- extend `TextFilePreview` with `modified_ms: Option<i64>` and `hash: String`;
- `write_workspace_text_file(workspace_root, path, content, expected_modified_ms, expected_hash,
  connection_id) -> WriteResult { modified_ms, hash, byte_len }`.

### 3.4 Keyboard

`Ctrl/Cmd+S` saves the active doc; `Esc` exits a clean edit mode / closes dialogs; fold toggles
(§9, §11). Must be keyboard-accessible (`ui-design.md`).

---

## 4. Required backend changes (`src-tauri/src/fs_entries.rs`)

### 4.1 Extend the read path (conflict inputs)

Add to `TextFilePreview`: `modified_ms: Option<i64>` (reuse `modified_ms(&meta)`) and `hash:
String` (fast non-crypto hash of the raw bytes, e.g. FNV-1a — avoid pulling a crypto dep). Compute
in `local_read_text_file`; for the remote path compute the hash from the fetched bytes and best-
effort mtime (remote currently returns `None` for mtime). Update the bridge + `code_view.rs`
callers. (No behavior change for existing readers; new fields are additive.)

### 4.2 New command: `write_workspace_text_file`

```rust
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult { pub modified_ms: Option<i64>, pub hash: String, pub byte_len: u64 }

#[tauri::command]
pub fn write_workspace_text_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    content: String,
    expected_modified_ms: Option<i64>,  // None ⇒ no conflict check (first save after force)
    expected_hash: Option<String>,      // primary conflict signal
    connection_id: Option<String>,
) -> Result<WriteResult, String> { /* local vs remote */ }
```

`local_write_text_file` behavior:
1. `resolve_under_root` (symlink-safe; file must already exist — we only edit opened files).
2. **Protected-path denylist** (§6/§8): reject if any path component is in the denylist.
3. **Conflict guard**: if `expected_hash` is `Some` and the current on-disk hash differs ⇒ return
   a `Conflict`-tagged error string; do **not** write. (Frontend re-calls with refreshed
   expectations to force-overwrite after user consent.)
4. **Atomic write**: write to a sibling temp file in the same dir, flush, then `fs::rename` over
   the target. `ensure_under_root` on the temp path before rename.
5. Return fresh `{ modified_ms, hash, byte_len }` so the frontend resets its baseline without a
   reload.

`remote_write_text_file`: write over the exec channel (base64 the content, `printf %s | base64 -d
> tmp && mv tmp target` with `mkdir -p` guarded), reusing `remote_target` sandboxing. Document the
weaker remote sandbox (already accepted in this file). Remote conflict check via a `wc -c`/hash
command best-effort; if unavailable, fall back to mtime-less write with an explicit warning path.

### 4.3 Optional pre-overwrite backup

Behind an app-setting flag (off by default): copy the target to `<file>.blxbak` before rename.

### 4.4 Registration

Add `fs_entries::write_workspace_text_file` to the `invoke_handler!` list in `lib.rs` (wiring only).

### 4.5 Notes on existing safety

`resolve_under_root` already canonicalizes and rejects symlink escape for existing files, so the
local write path inherits that. Keep using it; do not reintroduce a lexical-only resolver.

---

## 5. File type policy

Editability is derived from the already-computed `FileMeta.kind` + `policy_kind` plus a small
frontend policy module (`editor/policy.rs`), with the backend independently enforcing
size/protected-path rules (defense in depth).

- **Editable by default** — `FileKind::Code` and `FileKind::Text` (covers rs, ts/tsx, js/jsx,
  py, json, css/scss, html, yaml, toml, sh, ini/conf/env, sql, go, c/cpp, …; full list lives in
  `classify_kind`). These open in View (preview) and offer **Edit** (or open directly in Edit on
  double-click — §11).
- **Read-only by default (toggleable)** — any file with `policy_kind.is_some()` (README, LICENSE,
  CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, SUPPORT, CHANGELOG, AUTHORS, AGENTS/CLAUDE/…). These
  render through `MarkdownView`; the title bar shows **Edit** to switch to the raw editor.
  Plain Markdown (`FileKind::Markdown`, no policy) is **editable by default** but opens in the
  rendered view with an Edit toggle (preview-first).
- **Never edit (no Edit button)** —
  - `FileKind::Binary` (already → `UnsupportedView`): "binary file" panel, no editor.
  - Truncated reads (`truncated == true`) / over the size cap: open read-only with a "file too
    large to edit safely" notice (reuse the existing truncation banner) + optional "open in system
    editor".
  - Files under the protected/generated/vendor denylist (§6): read-only with an explanatory banner;
    backend rejects writes regardless.
- **Resolution order:** `NeverEdit` (binary / too-large / protected) **>** `ReadOnlyByDefault`
  (policy docs) **>** `Editable`.

Only add missing hljs language mappings if the editable set surfaces a gap; `hljs_lang_for_ext` is
already broad. Unknown text extension → `None` lang ⇒ editable as escaped plain text (existing
fallback in `prepare_lines`).

---

## 6. Read-only and protected file policy

### 6.1 Protected / generated / vendor denylist (never writable in-app)

Reject writes (backend + frontend gating) when any path component matches:

```
.git/   .agents/   .blxcode/   node_modules/   target/   dist/
build/  out/  .next/  .cache/  vendor/  __pycache__/  .venv/ venv/  coverage/
```

Rationale: protects VCS internals, the agent memory/architecture/skills store under `.agents/`, the
`.blxcode/` agent-context export dir, build outputs, and dependency trees. `.vscode/`/`.idea/` are
*soft* (editable only after an explicit Edit confirm), gated by an app setting.

### 6.2 Important-docs read-only (soft, via `PolicyKind`)

Policy docs render read-only (MarkdownView). The title-bar **Edit** button promotes the open
document to Edit (raw text). The promotion is per-open and **not persisted** — reopening returns to
the rendered read-only view.

### 6.3 Override path

`ReadOnlyByDefault` → Edit only via the explicit title-bar **Edit** button. `NeverEdit` has no
in-UI override. This yields a predictable policy: editable files open editable (or preview→edit);
policy docs preview with an Edit button; protected/binary/too-large are read-only with a reason
banner and no Edit button.

---

## 7. Editor title bar behavior (`header.rs`)

Extend `FilePreviewHeader` (reuse its layout + `workbench-mini-btn` styling). Threaded new props:
current `mode`, `editability`, `dirty`, and callbacks for edit/save/revert/close.

- **Left:** existing icon + **file name** + **relative path** (verbatim, `title=`, never
  translated — `ui-text.md`) + size/mtime chips (keep).
- **Status chips:** `View`/`Edit`, `Read-only`, `Modified` (dot/asterisk), `Truncated`, `Conflict`.
- **Actions (right):**
  - **Edit** — shown only when `mode == View && editability == ReadOnlyByDefault`. Switches to Edit.
  - **Save** — shown in Edit mode; enabled only when `dirty`. Mirrors `Ctrl/Cmd+S`.
  - **Revert** — enabled when `dirty`; restores `buffer = disk_text` (confirm if dirty).
  - **Close** — prompts on unsaved changes (§8.6); otherwise closes the center tab.
  - **Copy path** / **Refresh** — keep existing buttons.
  - **Open in system editor** (optional) — via the existing `open_external_url`/opener plugin
    pattern (resolve absolute path backend-side).
  - **Format** (optional, later) — only rendered when a formatter is wired for the language; hidden
    otherwise (no dead UI).

Buttons keyboard-focusable; Save/Revert disabled (not hidden) when N/A for stable layout.

---

## 8. Save, revert, and conflict handling

### 8.1 Load
`read_workspace_text_file` → set `disk_text = buffer = content`, store `load_modified_ms`,
`load_hash`, compute `lang` + folds. Truncated/over-cap ⇒ read-only "too large" state. Binary never
reaches here.

### 8.2 Dirty
`dirty = buffer != disk_text` (derived `Memo`).

### 8.3 Save
1. Enabled only when `dirty`, Edit mode, editability allows writes.
2. `write_workspace_text_file(root, rel, buffer, expected_modified_ms=load_modified_ms,
   expected_hash=load_hash, conn)`.
3. Success ⇒ `disk_text = buffer`; update `load_modified_ms`/`load_hash` from `WriteResult`; clear
   dirty; brief "Saved" status (reuse `ToastService`).
4. `Conflict` ⇒ open conflict dialog (§8.5).
5. Other errors (protected / permission / io) ⇒ actionable toast/banner; buffer untouched (no loss).

### 8.4 Revert
`buffer = disk_text` after confirm if dirty. Optionally re-stat and offer "reload from disk" if the
file changed.

### 8.5 Conflict detection
- Primary: `expected_hash` mismatch on write ⇒ `Conflict`, no overwrite.
- Proactive: on the existing refresh action / window focus, `stat_workspace_file`; if disk
  `modified_ms` differs from `load_modified_ms` while dirty, show a non-blocking "changed on disk"
  banner.
- Conflict dialog (reuse `ConfirmDialog`/a small modal): **Overwrite** (re-call write with refreshed
  expectations = explicit consent), **Reload from disk** (discard buffer after confirm), **View
  diff** (feed `disk_text` vs `buffer` into the existing `file_diff`/`FileDiff` view), **Cancel**.
- The save path never silently overwrites external changes.

### 8.6 Data-loss prevention
Prompt (reuse `HarnessUiService::request_confirm` / `ConfirmRequest`) on:
- **Close tab** with a dirty doc → Save / Discard / Cancel.
- **Switch workspace** (`active_id` change) with any dirty doc → confirm; offer save-all.
- **Open another file into a dirty tab** (if tabs reuse by `rel_path`) → same prompt.
- **App exit** — the existing `beforeunload` flush saves the *workbench snapshot*, not file buffers.
  Add a dirty-docs check: either block close via Tauri `ExitRequested` round-tripping to the
  frontend prompt, or (simpler v1) surface a `beforeunload` warning when any doc is dirty.
  (Treat the `ExitRequested` interception as its own milestone — it touches window lifecycle.)

---

## 9. Folding support (`editor/folding.rs`)

highlight.js does not fold. Compute a language-aware fold-range model in Rust, reusing the per-line
row structure `render_code` already emits. Pure + unit-tested.

### 9.1 Model
```rust
struct FoldRange { start_line: usize, end_line: usize, kind: FoldKind }
enum FoldKind { Braces, Indent, Imports, Region, MarkdownSection, FencedCode }
struct FoldState { ranges: Vec<FoldRange>, collapsed: HashSet<usize> } // keyed by start_line
```

### 9.2 Detection (heuristic, no full parser)
- **Brace languages** (rs, ts/tsx, js/jsx, json, css/scss, html, c-family, go, java, kt): balanced
  `{ }` (and `[ ]` for JSON) spanning ≥2 lines ⇒ a range. Covers functions/classes/interfaces/blocks.
- **Imports**: contiguous leading `use` / `import` / `from … import` lines ⇒ one `Imports` range.
- **Indent languages** (py, yaml): a header line whose following lines are more-indented ⇒ a range.
- **Regions**: `// #region`/`#endregion`, `// region`, `#pragma region`, `// MARK:`.
- **Markdown**: heading hierarchy (`#`..`######`) folds to the next heading of equal/higher level
  (`MarkdownSection`); fenced ```` ``` ```` blocks (`FencedCode`).

### 9.3 Rendering + phasing
- A **gutter** column (next to the existing `code-view__lineno`) shows a chevron on fold-start
  lines; click + keyboard toggle `collapsed`.
- **Phase 1 — view mode:** collapsing hides the row `<div>`s for `start+1..=end` and shows a `…`
  placeholder row (straightforward; the rows are already individual elements).
- **Phase 2 — edit mode:** a `<textarea>` cannot natively hide lines. v1: **disable folding in Edit
  mode** (entering Edit expands all; returning to View restores). Full edit-mode folding
  (virtualized line model) is deferred and, if required, is the strongest reason to take the
  CodeMirror escape hatch (§2.3).

---

## 10. Existing syntax highlighting integration

### 10.1 Reuse, don't replace
- Edit mode keeps using `hljs_glue::highlight` + `util::split_highlighted_into_lines` exactly as
  `prepare_lines` does today. The highlighted line backdrop is the same in view and edit mode.
- One language source of truth stays `lang_for_path` / `util::hljs_lang_for_ext` (already broad,
  already tested). No duplication.

### 10.2 Edit-mode highlighting flow
- The overlay textarea owns the text; on debounced `input`, recompute `prepare_lines(buffer, lang)`
  and swap the backdrop rows. Keep the gutter/selection structure.
- Keep the textarea and backdrop **pixel-aligned**: identical `font-family`, `font-size`,
  `line-height`, `tab-size`, `white-space: pre`, padding; sync `scrollTop/scrollLeft`
  (textarea scroll → backdrop transform). Primary CSS work, added to `styles.css` under the
  existing `.code-view` rules.

### 10.3 Fallback for unknown/unsupported
- Unknown extension / `lang == None` ⇒ editable as escaped plain text (existing `escape_lines`
  path). Non-UTF-8 / binary never enters edit mode (backend rejects / `FileKind::Binary`).

### 10.4 Missing mappings
Add only if the editable flow needs them. `hljs_lang_for_ext` already covers the required set
(ts→typescript, tsx→typescript, jsx→javascript, toml→ini, html→xml, scss→scss, yaml, json, rust,
python, bash, …). Vendor an extra single-language hljs file into
`public/vendor/highlight/languages/` only if a concretely required language is absent from the
common bundle.

---

## 11. UX behavior and keyboard shortcuts

### 11.1 Click / open semantics
- **Single click** in the explorer → opens the `FilePreview` tab in **View** (today's behavior;
  preview-first). Reuses `wb.open_center_file_tab`.
- **Double click** (or Enter on a focused row) → open editable-by-default files directly in **Edit**.
  Add a double-click handler to the file row in `project_explorer/mod.rs` (currently only
  single-click exists). Policy docs still open in View with an Edit button.
- Title-bar **Edit** promotes View→Edit for `ReadOnlyByDefault`.

### 11.2 Tabs vs single document
Tabs already exist (`CenterTabKind::FilePreview`). Keep one file per tab; dirty state is per open
doc. No new tab model needed. (Multi-pane split is out of scope.)

### 11.3 Shortcuts
- `Ctrl/Cmd+S` → save active doc (no-op if clean / read-only).
- `Esc` → close dialog; if Edit on a `ReadOnlyByDefault` doc and clean, return to View.
- `Tab` inside the editor inserts indentation (configurable spaces/tab; default per file type),
  with an a11y escape (e.g. `Esc` then `Tab` to leave focus).
- Fold toggle at cursor (view mode): `Ctrl/Cmd+Shift+[` / `]`; chevron click otherwise.

### 11.4 States to render (reuse existing classes/banners)
- **Loading** (`file-preview__status` "Loading…"), **Error** (`render_load_error`, path verbatim),
  **Empty** (empty editable buffer, not an error), **Read-only banner** (policy/protected, with
  Edit hint where applicable), **Binary** (`UnsupportedView`), **Too large / truncated** (existing
  `file-preview__notice` truncation banner, read-only), **Conflict** (banner + dialog),
  **Dirty** (Modified chip + close-dot), **Saved** (toast).

---

## 12. i18n / localization keys

All new strings via `I18nKey`, added to **all 13 locales** (compile-time exhaustive). Source in
`en_us.rs`; regenerate others with `scripts/tools/render_i18n_locales_from_en.py`, then review.
Namespace: `FilePreviewEditor*` (sibling of existing `FilePreview*`). Paths/filenames never
translated. Use `i18n.tr(K)()` and `.replace("{detail}", …)` for placeholders.

| Key | en-US source |
|---|---|
| `FilePreviewEditorEdit` | "Edit" |
| `FilePreviewEditorSave` | "Save" |
| `FilePreviewEditorSaved` | "Saved" |
| `FilePreviewEditorRevert` | "Revert changes" |
| `FilePreviewEditorClose` | "Close" |
| `FilePreviewEditorOpenInSystem` | "Open in system editor" |
| `FilePreviewEditorModeView` | "View" |
| `FilePreviewEditorModeEdit` | "Edit" |
| `FilePreviewEditorReadOnly` | "Read-only" |
| `FilePreviewEditorModified` | "Modified" |
| `FilePreviewEditorReadOnlyBanner` | "Opened read-only. Click Edit to make changes." |
| `FilePreviewEditorProtectedBanner` | "This file is in a protected folder and can't be edited here." |
| `FilePreviewEditorTooLargeBanner` | "This file is too large to edit safely and is shown read-only." |
| `FilePreviewEditorSaveError` | "Couldn't save the file: {detail}" |
| `FilePreviewEditorConflictTitle` | "File changed on disk" |
| `FilePreviewEditorConflictBody` | "This file changed outside the editor since you opened it." |
| `FilePreviewEditorConflictOverwrite` | "Overwrite" |
| `FilePreviewEditorConflictReload` | "Reload from disk" |
| `FilePreviewEditorConflictViewDiff` | "View diff" |
| `FilePreviewEditorUnsavedTitle` | "Unsaved changes" |
| `FilePreviewEditorUnsavedBody` | "You have unsaved changes. Save before closing?" |
| `FilePreviewEditorDiscard` | "Discard" |
| `FilePreviewEditorFoldRegion` | "Fold region" |
| `FilePreviewEditorUnfoldRegion` | "Unfold region" |

(Reuse existing `BtnSave`/`BtnClose`/`Cancel`-type keys where they already exist rather than
duplicating.)

---

## 13. Testing strategy

Per `.agents/rules/testing.md`: every backend command gets happy + error/escape `#[cfg(test)]`
tests with temp dirs (mirror the existing `fs_entries.rs` suite); pure frontend logic gets
`#[cfg(test)]`; run `cargo test --workspace`.

### 13.1 Backend (`fs_entries.rs` tests, extend the existing module)
- **Write happy path**: write under root; round-trips content; atomic (no leftover temp file).
- **Write conflict**: stale `expected_hash` ⇒ Conflict, file unchanged; matching hash ⇒ success;
  `expected_hash = None` ⇒ force-writes.
- **Write protected path**: into `.git/`, `.agents/`, `.blxcode/`, `node_modules/`, `target/`,
  `dist/` ⇒ rejected.
- **Write outside root / `..` / symlink escape**: rejected (extend existing escape tests to writes).
- **Read returns mtime + hash**; hash changes when content changes; truncated read flagged.
- **Remote `remote_target` sandbox** for the write path (reuse the existing pattern).

### 13.2 Frontend pure logic (`#[cfg(test)]`)
- `editor/policy.rs`: editable vs read-only-by-default (policy_kind) vs never-edit (binary /
  truncated / protected) resolution order; denylist component matching.
- `editor/buffer.rs`: dirty derivation; revert restores baseline; save updates baseline + clears
  dirty; conflict transitions.
- `editor/folding.rs`: brace ranges, import grouping, indent (py/yaml), region markers, markdown
  heading sections + fenced blocks; nested ranges; collapse/expand set.
- (Existing `util.rs` highlighting/line-split tests remain green — no regressions.)

### 13.3 Manual QA
- Edit a `.rs`/`.ts`/`.json` → `Ctrl+S` → reopen → persisted; highlighting identical view vs edit.
- `README.md` opens rendered + Edit button → Edit → save.
- Modify a file externally, then save → conflict dialog → each branch (overwrite / reload / diff).
- Close tab / switch workspace / exit with unsaved changes → prompt; Discard/Cancel/Save correct.
- Binary, too-large/truncated, empty, unknown-extension text file.
- Folding: collapse/expand functions, imports, markdown sections, fenced code (view mode);
  entering Edit expands all.
- Remote (SSH) workspace: read + (if enabled) write path, or confirm local-only gating message.
- Keyboard-only flow (open, edit, save, fold, close, dialogs).

### 13.4 Validation commands (before "done")
```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check -p blxcode-ui --target wasm32-unknown-unknown
cargo check -p blxcode
trunk build
```

---

## 14. Step-by-step implementation milestones

1. **M1 — Backend read+write.** Extend `TextFilePreview` (mtime+hash); add
   `write_workspace_text_file` (atomic, conflict guard, protected denylist) local + remote; register
   in `lib.rs`. Full `fs_entries.rs` tests.
2. **M2 — Bridge.** Mirror new fields/types and add the `write_workspace_text_file` wrapper in
   `tauri_bridge.rs`. Update existing `read_workspace_text_file` callers for the additive fields.
3. **M3 — Editor module + state.** `editor/{buffer,policy}.rs` + `OpenDoc` state (service-held,
   keyed by workspace+rel_path), dirty `Memo`. Unit tests.
4. **M4 — Edit overlay.** Refactor `code_view.rs` so the highlighted backdrop is reusable; add the
   transparent textarea overlay + debounced re-highlight + scroll sync (CSS in `styles.css`). View
   mode unchanged.
5. **M5 — Title bar.** Extend `header.rs`: View/Edit + dirty/read-only chips, Edit/Save/Revert/Close
   (+ optional open-in-system). i18n keys.
6. **M6 — Markdown/policy Edit toggle.** `markdown_view.rs` Edit button → raw `EditableCodeView`
   (lang=markdown) for `ReadOnlyByDefault`/plain markdown.
7. **M7 — Save / revert / conflict.** Wire save flow, conflict dialog (reuse ConfirmDialog),
   focus/refresh stat check, "View diff" via existing `file_diff`.
8. **M8 — Data-loss guards.** Close-tab / workspace-switch prompts via `request_confirm`; decide
   exit interception (`beforeunload` warning v1, `ExitRequested` later).
9. **M9 — Folding (view mode).** `editor/folding.rs` + gutter chevrons + static row hide for code +
   markdown.
10. **M10 — Folding (edit) decision.** "Expand-all on Edit" baseline; evaluate virtualized
    edit-mode folding vs CodeMirror escape hatch; record the decision.
11. **M11 — Polish + i18n regen + docs.** Run `render_i18n_locales_from_en.py`, review translations;
    update `CHANGELOG.md`; loading/empty/error/banners; perf threshold; a11y pass; update
    `docs/user/file-preview.md`.
12. **M12 — Validation.** Full §13.4 suite + manual QA matrix.

---

## 15. Risks and open questions

### Risks
- **Edit-mode folding fidelity (highest):** overlay editors can't natively hide lines. Mitigation:
  view-mode folding first; expand-all in Edit; CodeMirror escape hatch (§2.3) if required.
- **Backdrop/textarea misalignment** across fonts/zoom/IME/tabs. Mitigation: shared CSS metrics +
  scroll sync; cross-platform QA (win32 primary, Linux WebKitGTK secondary).
- **Re-highlight perf** on large files. Mitigation: debounce + size threshold (read path already
  exposes `byte_len`/`truncated`); plain-text fallback above threshold.
- **Edit state lifetime across tab switches/remounts.** Mitigation: hold `OpenDoc` in the service
  keyed by `(workspace_id, rel_path)` (confirm §1.8) so remounts don't drop edits.
- **Remote (SSH) write correctness/sandbox** is weaker than local. Mitigation: explicit conflict
  handling + documented limitation; consider local-only writes in v1 (open question 1).
- **App-exit interception** touches native window lifecycle. Mitigation: ship `beforeunload`
  warning first; isolate `ExitRequested` interception as a later milestone.

### Open questions
1. **Remote editing in v1?** Recommend local-only writes first; mirror write/conflict over SSH FS
   in a follow-up. (Confirm with the SSH/remote-workspace owner.)
2. **Tab reuse:** does `open_center_file_tab` reuse a tab for the same `rel_path`? Drives the
   "open into a dirty tab" prompt.
3. **Pre-overwrite `.blxbak` backup** — on, opt-in via settings, or omit for v1?
4. **Formatter integration** (rustfmt/prettier) — which languages, when? (Hide the button until
   wired.)
5. **Indentation defaults** — tabs vs spaces per language; expose in settings?
6. **Edit size cap** — keep the 512 KiB read cap as the edit cap, or a separate (smaller) one?
7. **Agent handoff** — the agent reads files from disk; is saving sufficient, or should a dirty
   buffer be surfaced to agent context? (Recommend: disk is the source of truth; saving is enough.)
```

