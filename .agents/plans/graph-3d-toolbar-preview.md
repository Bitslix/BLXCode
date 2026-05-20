# Graph 3D Toolbar And Preview Plan

**Status:** implemented

## Summary

Add a lazy-loaded offline 3D memory graph as the default Graph tab experience, keep the existing SVG graph as the 2D fallback, replace graph text controls with icon toolbar controls, and change graph node clicks to stay in the Graph tab while opening a markdown preview popover. Split graph-specific code out of `memory_panel.rs` while preserving existing Files/Search behavior.

## Key Changes

- Create `frontend-js/` with `package.json`, committed `package-lock.json`, `graph3d_entry.mjs`, and `build.mjs`; build via `npm --prefix frontend-js ci` in CI and `npm --prefix frontend-js run build:graph3d` locally/Tauri.
- Generate `public/graph3d.bundle.mjs` during dev/build/CI; do not rely on CDN and do not eagerly load it in `index.html`.
- Update Tauri build hooks to run `npm --prefix frontend-js run build:graph3d && trunk ...`; update release workflow to install Node, run `npm ci --prefix frontend-js`, then build the graph bundle before `tauri-action`.
- Split graph code into `src/workbench/memory_graph/` with modules for mode/state, 2D layout/view, 3D view, toolbar, preview popover, JS glue, and module exports. Keep graph CSS in `styles.css` unless a second Trunk CSS link is explicitly added.
- Keep `memory_panel.rs` responsible for shared memory state, Files, Search, note loading/editing, and tab routing. Target `< 800` lines only if Files/Search/editor helpers are also split; otherwise treat graph extraction as the required milestone.

## Interfaces And Behavior

- Add `Serialize` to `GraphData`, `GraphNode`, and `GraphEdge` so Rust can pass graph data to JS through `serde_wasm_bindgen`.
- Add `GraphMode::{ThreeD, TwoD}` with default `ThreeD`; persist the selected mode in localStorage using a new `GRAPH_MODE_STORAGE_KEY`.
- Add `window.__blxcodeGraph3d` API: `create(container)`, `dispose(id)`, `setData(id, graphData)`, `zoom(id, factor)`, `resetView(id)`, `flyToNode(id, nodeId, ms)`, and `resize(id)`.
- Emit `blxcode-graph3d-api-ready` when the bundle is ready, and emit `blxcode-graph3d-node-click` with `{ graphId, nodeId }` for 3D node clicks.
- Implement `ensure_graph3d_script()` with loaded/loading/error states, script de-duplication, and timeout/error fallback to 2D.
- In JS, implement layout gating with `layoutReady = false` after `setData()`, `layoutReady = true` on `onEngineStop`, and a `pendingFlyTo` queue replayed after layout stabilizes.
- Toolbar buttons: Reset, Zoom In, Zoom Out, and 2D/3D toggle using lucide icons and localized `title`/`aria-label` text.
- Node click in both 2D and 3D: fly to node when possible, call `memory_read`, remain in Graph tab, and show preview popover.
- Preview popover renders markdown with `render_markdown_to_html`, has Close and Open in Files actions, and intercepts `blxmemory:` links locally so wikilink navigation updates the popover and graph focus instead of switching tabs through the global memory-link handler.
- Open in Files calls the existing note-loading flow, sets the active path/editor content/backlinks, and switches to `MemoryView::Files`.

## i18n

Add keys in `I18nKey` and all locale tables:
`MemGraphReset`, `MemGraphZoomIn`, `MemGraphZoomOut`, `MemGraphMode3d`, `MemGraphMode2d`, `MemGraphPreviewClose`, `MemGraphOpenInFiles`, `MemGraph3dLoadFailed`.

## Test Plan

- `npm ci --prefix frontend-js`
- `npm --prefix frontend-js run build:graph3d`
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
- `trunk build`
- Verify app start without opening Graph does not request `graph3d.bundle.mjs`.
- Verify opening Graph lazy-loads the bundle and shows the 3D graph.
- Verify graph bundle load failure falls back to 2D with a localized error.
- Verify 2D/3D toggle disposes/recreates graph instances without duplicate listeners.
- Verify Reset, Zoom In, and Zoom Out work in both modes.
- Verify clicking a node before 3D layout completion flies to the node after `onEngineStop`.
- Verify node click opens the popover, Close hides it, Open in Files switches tabs, and wikilinks update the popover while staying in Graph.
- Verify offline Tauri build includes the generated graph bundle.

## Assumptions

- Generated `public/graph3d.bundle.mjs` is a build artifact, not a committed source file.
- `three-spritetext` is omitted unless labels are needed in 3D after the basic orb view works.
- Existing 2D graph remains the functional fallback and keeps its current force layout behavior.
- The global `blxmemory:` document click handler remains unchanged; the popover prevents propagation for its own wikilinks.
