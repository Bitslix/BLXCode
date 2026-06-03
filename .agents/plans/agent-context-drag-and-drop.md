# Agent-Tab Context Drag & Drop (File / Diff / Commit)

> Status: **done** (umgesetzt & verifiziert: `cargo check` wasm+tauri grün,
> `cargo test --workspace` 401 Tests grün)

## Summary

Die bestehende Drop-Zone des Agent-Tabs (rechtes Panel) wird so erweitert, dass
man **Files** aus dem Filebrowser (linke Sidebar), **File-Diffs** aus der
Diff-Sektion und **Git-Commits** aus dem Commit-Graph per Drag & Drop ablegen
und damit als Agent-Kontext anhängen kann. Das spiegelt das bereits vorhandene
Terminal-Slot-DnD: ein eigener Drag-Service plus eine cursor-folgende,
typ-spezifische Overlay-Karte (Icon + Farbe je File/Diff/Commit), damit es
genauso „cool" aussieht wie beim Terminal-Drag.

## Design-Entscheidungen

- **File** → neuer Kind `FileRef`: nur **Pfad-Referenz** (kein Inline-Content),
  der Agent liest den Inhalt bei Bedarf selbst über seine Tools. **Persistent**.
- **Diff** → neuer Kind `GitDiff`: **Inline-Content** (Diff-Text via
  `git_file_diff`). **Persistent** (manuell über `×` entfernbar).
- **Commit** → neuer Kind `GitCommit`: **Inline-Content** (Subject/Body +
  geänderte Files via `git_commit_details`), `paths` = geänderte Dateien.
  **Persistent**.
- Das Terminal-DnD bleibt **unangetastet** — paralleler Service, gleiche
  Drop-Zone. Drop-Messages bleiben (wie `AcceptImage`) hartkodiert, kein i18n.

## Ist-Zustand (verifiziert)

- **Drop-Zone**: `src/workbench/agent_panel/image_context.rs` (`DropZoneState`
  Inactive/AcceptImage/AcceptTerminal/Reject; `handle_dom_drag_event` /
  `handle_dom_drop`), Markup in `agent_panel/mod.rs:512-538`
  (`.agent-drop-overlay`).
- **Terminal-DnD-Vorbild**: `src/workbench/terminal_slot_dnd.rs` (Service mit
  `active`/`overlay_pos`/`session`/`session_gen`) +
  `src/workbench/terminal_slot_drag_overlay.rs` (Vorschau-Karte). Drag-Start in
  `terminal_cell.rs:563-599`.
- **Drag-Quellen** (aktuell nicht draggable):
  - File-Rows: `project_explorer/mod.rs:604-637` (`!is_dir`-Zweig)
  - Diff-Rows: `file_diff_section/mod.rs:687+` (`FileDiffRow`, hat `rel_path`,
    `staged`)
  - Commit-Rows: `git_graph/mod.rs:404+` (`GitGraphRow`, hat `oid`, `subject`)
- **Context-Pipeline**: `AgentContextItem`/`AgentContextKind` in
  `agent_wire.rs:36` **gespiegelt** in `src-tauri/src/agent/protocol.rs:54`.
  Rendering an drei Stellen:
  1. `session_orchestrator.rs:226` `render_context_prompt` (echter Agent-Prompt)
  2. `agent_context_handoff.rs:81` `render_agent_context_block` (Terminal-Handoff)
  3. `context_list.rs:97` `ContextRow` (UI, generisch — `FileSnippet` als Muster)
- Bridge-Funktionen vorhanden: `git_file_diff`, `git_commit_details`,
  `read_workspace_text_file`.

## Phasen

| # | Status | Scope |
|---|--------|-------|
| 1 | **done** | **Datenmodell**: Kinds `FileRef`, `GitDiff`, `GitCommit` in `agent_wire.rs` + `protocol.rs` |
| 2 | **done** | **DnD-Service**: neu `src/workbench/context_drag.rs` (MIME `application/x-blxcode-context`, `ContextDragKind`, `ContextDragPayload`, `ContextDragService`, `ContextDragMeta`); im Workbench-Context bereitstellen |
| 3 | **done** | **Drag-Quellen**: `draggable`+`dragstart/drag/dragend` an File-/Diff-/Commit-Rows (Muster `terminal_cell.rs`) |
| 4 | **done** | **Drop-Zone**: `DropZoneState` um `AcceptFile/AcceptDiff/AcceptCommit`; Drag/Drop-Handler für neuen MIME; Diff/Commit async via `git_file_diff`/`git_commit_details`. Konstruktoren `file_ref_context_item`/`git_diff_context_item`/`git_commit_context_item` in `agent_context_handoff.rs` + Tests |
| 5 | **done** | **Overlay**: neu `src/workbench/context_drag_overlay.rs`, cursor-folgende Karte, Icon+Farbe je Kind (analog `terminal_slot_drag_overlay.rs`) |
| 6 | **done** | **Rendering**: neue Sektionen in `session_orchestrator.rs` (Backend-Prompt) + `render_agent_context_block` (Handoff): FileRef→Pfad-Liste, Diff/Commit→Inline-Blöcke |
| 7 | **done** | **CSS**: Overlay-Varianten `.context-drag-preview--{file,diff,commit}` + Dropzone-Tönung je Kind (`styles.css`) |
| 8 | **done** | **Verify**: `cargo check` (wasm + tauri) + `cargo test --workspace` (401 passed) |

## Verifikation

```bash
cargo check -p blxcode-ui --target wasm32-unknown-unknown
cargo check -p blxcode
cargo test --workspace
```
