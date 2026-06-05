# Window Popout

## Summary

Implement real Tauri child windows for workbench popouts on `feature/window-popout`, based on `stage`. Popouts use separate `WebviewWindow`s with the BLXCode custom titlebar, existing theme tokens, and i18n.

## Decisions

- Terminal popout moves the live renderer to the child window instead of mirroring it.
- Memory, Mermaid, and File Diff use local toolbar popout buttons.
- Mermaid support covers both file previews and diagram gallery/group views.
- Child windows use custom BLXCode chrome adapted for popouts.

## Implementation Notes

- Add typed popout commands in the Tauri backend and matching frontend bridge wrappers.
- Add a minimal popout shell route in the Leptos app.
- Keep user-facing strings in `I18nKey` locale tables.
- Use existing CSS theme tokens and lucide icons.

## Tests

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo check --workspace`
- Manual Tauri verification for terminal, memory, Mermaid, and File Diff popouts.

## Tasks

- [x] `branch` - Create `feature/window-popout` from `stage`
- [x] `backend` - Add popout window commands
- [ ] `frontend-route` - Add popout bridge and route
- [ ] `child-titlebar` - Add child custom titlebar
- [ ] `terminal-ownership` - Implement move-to-popout terminal ownership
- [ ] `view-buttons` - Add local popout buttons
- [ ] `i18n` - Add localized popout labels
- [ ] `verify` - Run checks and polish
