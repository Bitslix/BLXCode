# Handoff: Memory System — Phases 5 & 6

## Context

This is BLXCode (Tauri 2 + Leptos 0.8 CSR), a desktop IDE.  
Phases 1–4 of the memory-parity plan are **complete and compiling**.  
This document tells you exactly what is left for Phases 5 and 6 and where every relevant piece of code lives.

---

## What Was Done (Phases 1–4)

### Backend (`src-tauri/src/memory/`)
- `memory.rs` was replaced by a full module: `types.rs`, `paths.rs`, `frontmatter.rs`, `wikilinks.rs`, `graph.rs`, `store.rs`, `mod.rs`
- `MemoryScope { Workspace, Global }` (serde: `"workspace"` / `"global"`) runs everywhere
- Global roots: `~/.blxcode/memory/` and `~/.blxcode/learnings/` (via `dirs::home_dir()`)
- New commands registered in `src-tauri/src/lib.rs`: `memory_status`, `memory_bootstrap`
- All CRUD commands have `scope: MemoryScope` parameter
- `memory_list` returns `MemoryListResponse { notes: Vec<NoteMeta>, memory_subcategories: MemorySubcategories { workspace, global } }`
- `memory_backlinks` returns `Vec<BacklinkRef { scope, path }>`
- Graph builder creates hub nodes (`id: "hub:{category}"`, `is_category_hub: true`) and cross-scope edges
- Legacy migration (`migrate_legacy_memory`) removed from `agents_layout.rs`

### Frontend (`src/`)
- `src/tauri_bridge.rs`: all memory types updated, `MemoryScope` enum, `note_key(scope, path)`, `parse_note_key(key)`, `memory_bootstrap`, `memory_status`
- `src/workbench/memory_panel.rs`: dual-scope sidebar implemented
  - `MemoryState` has `active_scope`, `global_subcategories`, `global_bootstrapped`, `backlinks: Vec<BacklinkRef>`
  - Files view has two sections: "Projekt" (workspace) and "Global"
  - Global section shows "Create Global Memory" button when not bootstrapped
  - All CRUD ops (create note/category, rename, delete, backlink click, context menu) use `note.scope`
  - Workspace bootstrap called on workspace change
- `src/workbench/memory_graph/mod.rs`: still uses **hardcoded `MemoryScope::Workspace`** in two places (intentional Phase 4 deferral)

---

## Current State of `memory_graph/mod.rs`

File: `src/workbench/memory_graph/mod.rs`

Two hardcoded workspace fallbacks that need to be fixed in Phase 5:

**Line ~810** (in the "Open in Files" button click handler):
```rust
expand_files_group_for_path(state.clone(), &MemoryScope::Workspace, &path);
load_note(state.clone(), ws, MemoryScope::Workspace, path);
```

**Line ~858** (inside `open_graph_preview`):
```rust
match tauri_bridge::memory_read(&ws, &MemoryScope::Workspace, &path).await {
```

The `GraphPreviewState.path` signal currently holds just the bare API path (`"decisions/note.md"`), without scope. The graph node ID is `"{scope}:{path}"` (see `GraphNode.id` in `tauri_bridge.rs`). The scope must be recovered from the node ID when a node is clicked, so it can be threaded through to `load_note`, `memory_read`, etc.

---

## Phase 5 — Graph-Tab: Scope-Aware Clicks + Hub-Node Styling

### Goal
1. Node clicks carry the correct scope (workspace or global)
2. Hub nodes are visually distinct from regular note nodes
3. Cross-scope edges are optionally styled differently

### Key Types (already defined in `src/tauri_bridge.rs`)

```rust
pub struct GraphNode {
    pub id: String,           // "workspace:decisions/note.md" or "hub:decisions"
    pub scope: MemoryScope,
    pub path: String,         // bare API path, e.g. "decisions/note.md"
    pub label: String,
    pub tags: Vec<String>,
    pub orphan: bool,
    pub category: String,
    pub is_category_hub: Option<bool>,  // true for hub nodes
    pub hub_scopes: Option<Vec<MemoryScope>>,
    pub color: Option<String>,
}

pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub cross_scope: bool,
}
```

### Changes Required in `src/workbench/memory_graph/mod.rs`

#### 1. `GraphPreviewState` — add scope field

```rust
struct GraphPreviewState {
    open: RwSignal<bool>,
    scope: RwSignal<MemoryScope>,      // ADD THIS
    path: RwSignal<Option<String>>,
    label: RwSignal<String>,
    content: RwSignal<String>,
    loading: RwSignal<bool>,
}
```

Initialize `scope: RwSignal::new(MemoryScope::Workspace)` in `GraphPreviewState::new()`.

#### 2. `open_graph_preview` — derive scope from node ID

The function signature is `fn open_graph_preview(state: MemoryState, preview: GraphPreviewState, path: String)`.  
The `path` argument is currently the bare API path. Change the call site to pass the node **ID** (which includes the scope prefix), then parse it:

```rust
fn open_graph_preview(state: MemoryState, preview: GraphPreviewState, node_id: String) {
    // node_id is either "workspace:some/path.md" or "hub:category" or "global:some/path.md"
    let (scope, path) = tauri_bridge::parse_note_key(&node_id)
        .unwrap_or((MemoryScope::Workspace, node_id.clone()));

    state.graph_selected_node.set(Some(node_id));
    preview.open.set(true);
    preview.scope.set(scope.clone());
    preview.path.set(Some(path.clone()));
    // ... rest unchanged, but use `scope` instead of MemoryScope::Workspace for memory_read
    match tauri_bridge::memory_read(&ws, &scope, &path).await { ... }
}
```

For hub nodes (`parse_note_key` returns `None` because the ID is `"hub:category"`, not `"scope:path"`): show a hub summary panel (list of notes in this category) instead of reading a file. Check: `if node_id.starts_with("hub:")`.

#### 3. "Open in Files" button — pass scope correctly

```rust
expand_files_group_for_path(state.clone(), &preview.scope.get_untracked(), &path);
load_note(state.clone(), ws, preview.scope.get_untracked(), path);
```

#### 4. `navigate_to_graph_node` in `src/workbench/memory_graph/mod.rs`

This public function is called from `memory_panel.rs` when the user clicks "show in graph" for the active note. It currently takes a plain path. Update to also accept scope so it can set the correct selected node:

Current signature (approx.):
```rust
pub(crate) fn navigate_to_graph_node(state: MemoryState, path: String)
```

Change to:
```rust
pub(crate) fn navigate_to_graph_node(state: MemoryState, scope: MemoryScope, path: String)
```

And form the node ID: `let node_id = tauri_bridge::note_key(&scope, &path);`  
Then set `state.graph_selected_node.set(Some(node_id))`.

Update all call sites in `memory_panel.rs` — search for `navigate_to_graph_node` to find them.

#### 5. Hub Node Visual Styling

In the graph renderer (JavaScript side in `public/` or wherever the 3D/2D graph is rendered), hub nodes can be detected by their ID prefix `"hub:"`.

On the Leptos side, the node list passed to the JS graph should tag hub nodes. `GraphNode.is_category_hub` is already `Some(true)` for hubs. The existing graph bridge in `memory_graph/mod.rs` serializes the graph data to JS — check how nodes are passed and add a `hub` CSS class or color override.

Look for the JS graph integration in `public/` (likely `public/memory_graph.js` or similar) and check if there is already a hub-specific rendering path. The `GraphNode.color` field can be set by the backend but may also be overridden in JS. Hub nodes should be rendered larger and with a folder/category icon.

If the graph rendering is done entirely in WASM/Leptos without JS: add a `class:memory-graph-node--hub=move || node.is_category_hub.unwrap_or(false)` to the node element and style in `styles.css`.

#### 6. Cross-scope Edge Styling

`GraphEdge.cross_scope: bool` is already in the data. Style cross-scope edges with a dashed line or different color. Pass this through to the graph JS as an edge attribute.

---

## Phase 6 — Search-Tab: Scope-Aware Filter Chips

### Current State

File: `src/workbench/memory_panel.rs`, component `MemorySearchView` (around line 1830+).

Current filter chips are hardcoded to `"memory"` and `"learnings"` strings, which only cover workspace. The filter functions `memory_hit_count`, `learnings_hit_count`, `search_hit_category`, `filter_search_hits` all use string-comparison against these two categories.

`SearchHit` (in `src/tauri_bridge.rs`) already has:
```rust
pub struct SearchHit {
    pub scope: MemoryScope,
    pub path: String,
    pub line: u32,
    pub snippet: String,
    pub category: String,  // e.g. "decisions", "learnings", "memory"
}
```

### Changes Required

#### 1. Replace hardcoded filter chips with dynamic scope+category chips

Instead of a static "Memory" / "Learnings" split, build dynamic chips from the actual search results:

```
[All (N)]  [Workspace (N)]  [Global (N)]  [workspace:decisions (N)]  [global:memory (N)]  ...
```

The chip key format is:
- `None` → All results
- `Some("workspace")` → all workspace hits
- `Some("global")` → all global hits
- `Some("workspace:decisions")` → workspace hits in category "decisions"
- `Some("global:learnings")` → global learnings

Build chips dynamically from `SearchHit.scope` + `SearchHit.category`.

#### 2. Update `filter_search_hits`

```rust
fn filter_search_hits(hits: Vec<SearchHit>, filter: Option<String>) -> Vec<SearchHit> {
    match filter.as_deref() {
        None => hits,
        Some("workspace") => hits.into_iter().filter(|h| h.scope == MemoryScope::Workspace).collect(),
        Some("global") => hits.into_iter().filter(|h| h.scope == MemoryScope::Global).collect(),
        Some(key) => {
            // "workspace:decisions" or "global:learnings"
            if let Some((scope_str, cat)) = key.split_once(':') {
                let target_scope = match scope_str {
                    "workspace" => MemoryScope::Workspace,
                    "global" => MemoryScope::Global,
                    _ => return hits,
                };
                hits.into_iter()
                    .filter(|h| h.scope == target_scope && h.category == cat)
                    .collect()
            } else {
                hits
            }
        }
    }
}
```

#### 3. Update `load_note` call in search results

The search hit click currently calls:
```rust
load_note(s.clone(), ws, sc.clone(), p.clone());
```
This is already correct (Phase 4 set `h.scope` into `sc`). Verify it's still wired up correctly.

#### 4. Build dynamic filter chip list

Add a helper:
```rust
fn search_scope_categories(hits: &[SearchHit]) -> Vec<(String, usize)> {
    // Returns (chip_key, count) pairs for All + scope-level + scope:category level
    // Only include chips where count > 0
}
```

Render them in the `MemorySearchView` filter bar instead of the current hardcoded buttons.

#### 5. i18n

Add keys for "Workspace" and "Global" scope labels used in filter chips:
- `MemSearchFilterWorkspace`
- `MemSearchFilterGlobal`

Add to `src/i18n/keys.rs` and all locale files in `src/i18n/locales/`. See how `MemGlobalCreate` was added in Phase 4 for the pattern.

---

## Reference: EB Implementation

The BLXCode-eb reference lives at `c:\Users\quork\Entwicklung\BLXCode-eb\`.

For Phase 5:
- `src/views/app/layout/SidePanel/Memory/graph/MemoryGraphView.tsx` — hub node rendering, scope-aware click handlers
- `src/shared/memory.ts` — `overviewPathForCategory`, `preferredScopeForHub`, `CATEGORY_HUB_PREFIX = "hub:"`

For Phase 6:
- `src/views/app/layout/SidePanel/Memory/search/MemorySearchView.tsx` — scope+category filter chips
- `src/shared/memory.ts` — filter chip logic

---

## Compile Instructions

```powershell
# Check frontend only (fast)
cargo check -p blxcode-ui --target wasm32-unknown-unknown

# Check backend only
cargo check -p blxcode

# Run all tests
cargo test --workspace

# Full dev build
cargo tauri dev
```

There are currently no compile errors. There are some `dead_code` / `unused_import` warnings in `src-tauri/src/memory/store.rs` and `tauri_bridge.rs` (`parse_note_key`) — these are harmless leftovers.

---

## Files to Touch

| File | Why |
|------|-----|
| `src/workbench/memory_graph/mod.rs` | Phase 5: scope-aware node clicks, hub styling, `navigate_to_graph_node` signature |
| `src/workbench/memory_panel.rs` | Phase 5: update `navigate_to_graph_node` call sites; Phase 6: `MemorySearchView` filter chips |
| `src/i18n/keys.rs` | Phase 6: add `MemSearchFilterWorkspace`, `MemSearchFilterGlobal` |
| `src/i18n/locales/*.rs` | Phase 6: add translations for all 13 locales |
| `styles.css` | Phase 5: hub node CSS classes (if rendered in Leptos) |
| `public/` (JS graph) | Phase 5: hub node visual distinction in graph renderer |

Do **not** touch:
- `src-tauri/src/memory/` — backend is complete
- `src/tauri_bridge.rs` — bridge is complete (`parse_note_key` is already there)
- `src-tauri/src/lib.rs` — all commands registered

---

## Acceptance Criteria

**Phase 5 done when:**
- Clicking a global graph node opens it in the editor with `MemoryScope::Global`
- Clicking a hub node shows a summary (or a list of notes in that category) — not a 404
- Hub nodes are visually distinct from note nodes (size, shape, or color)
- The "Open in Files" button in the graph preview navigates to the correct scope section

**Phase 6 done when:**
- Search results from both workspace and global memory appear
- Filter chips allow narrowing by scope (`workspace` / `global`) and by scope+category
- Clicking a search result opens the note with the correct scope
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown` has no errors
