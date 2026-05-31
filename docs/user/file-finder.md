# Quick Open (Fuzzy File Finder)

Quick Open lets you navigate the active workspace by **fuzzy-matching a file path** from the keyboard — no sidebar scrolling required.

## Open

| Preset | Shortcut |
|--------|----------|
| **Tmux** | `Ctrl+B` then `o` |
| **Classic** | `Ctrl+O` |

The Quick Open dialog opens as a centered overlay above the workbench.

## How it works

Type a **fuzzy subsequence** of the file path — characters in order but not necessarily contiguous. The finder searches every file under the workspace root and ranks results by proximity:

| Input | Matches |
|-------|---------|
| `fpr` | `src/foo/parser.rs`, `docs/file-preview.md`, `scripts/format/prettier.cjs` |
| `stmd` | `src/lib/storage/memory.rs`, `scripts/tools/render_i18n_locales_from_en.py` |
| `rs` | All `.rs` files — the `r` then `s` appears in the extension of every Rust source |

The match score prefers **exact filename matches**, then path-segment starts, then consecutive characters, then broader subsequence matches. The top 20 results are shown.

## Controls

| Key | Action |
|-----|--------|
| **Arrow Up / Down** | Navigate the result list |
| **Enter** | Open the selected file in the preview tab |
| **Esc** | Close the finder without selecting |

Opening a file replaces the current preview tab content (if any) — same behavior as clicking a file in the sidebar.

## Scope

The finder indexes all non-hidden files under the workspace root at the moment the dialog opens. Hidden files (dot-prefixed entries) and protected folders (`.git/`, `.agents/`, `node_modules/`, `target/`, etc.) are excluded. Reopening the finder re-indexes the tree, so newly created files appear immediately.

## See also

- [File Preview](file-preview.md) — the editor/viewer that opens when you select a file
- [Keyboard Shortcuts](keyboard-shortcuts.md) — shortcut presets and rebinding
- [Workspaces](workspaces.md) — sidebar explorer and project files
