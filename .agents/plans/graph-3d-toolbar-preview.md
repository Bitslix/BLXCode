# Graph 3D Toolbar, Memory Styling, And Agent Context Plan

**Status:** implemented

## Summary

Ship the 3D Memory graph and toolbar, then extend Memory with clean labels,
responsive category coloring, BLXCode-owned context menus, Memory color presets
in Settings, and Agent context attachment for categories and individual notes.

## Key Changes

- Added the offline `frontend-js` graph bundle build and lazy-loaded 3D graph
  experience, with 2D fallback, icon toolbar controls, preview popovers, clean
  labels, responsive graph fitting, and calmer line wobble interactions.
- Added global Workbench right-click suppression and custom Memory context menus
  for category headers and individual Memory/Learnings entries.
- Added workspace-persisted Memory category settings: display label, color,
  show in sidebar, and show in graph.
- Added global Memory color presets in BLXCode Settings, with add/edit/delete
  and reset support; category edit dialogs consume these presets as swatches.
- Added Agent context attachments for Memory categories and individual notes,
  shown in a collapsed-by-default Agent Panel Context section with remove
  controls.
- Extended Agent turn wire format to send attached context metadata and paths;
  provider prompts receive a compact path-only context block before the user
  prompt.

## Interfaces And Persistence

- `UserTurn` now includes `contextItems`.
- `AgentContextItem` records kind, label, source, paths, and timestamp.
- `WorkspaceEntry` persists `agent_context_items` and
  `memory_category_settings`.
- `MEMORY_COLOR_PRESETS_STORAGE_KEY` stores global app-level color presets in
  localStorage.

## Test Plan

- `npm --prefix frontend-js run build:graph3d`
- `cargo check`
- Manual checks:
  - Right-click opens BLXCode Memory menus, not the browser menu.
  - Category dialog saves label/color/sidebar/graph settings.
  - Settings presets appear in the dialog and can be managed globally.
  - Graph nodes use configured category colors and respect graph visibility.
  - Category and note context items appear in Agent Context and are sent with
    the next Agent turn as paths only.
