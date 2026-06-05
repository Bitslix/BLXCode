# Terminal Drag To Split

## Summary

Implement drag-to-split for the Workspace Terminal Tab View Grid without breaking the existing swap behavior.

Current behavior: dragging a terminal slot onto another terminal slot swaps the two slots.

Target behavior: dragging a terminal onto an existing terminal can either swap or move the dragged terminal into the target slot as a new split pane. The UI should match the attached reference screenshots: edge zones show "Split Top", "Split Bottom", "Split Left", or "Split Right"; the center zone remains "Swap".

The implementation should stay in the existing Rust/Leptos workbench model. No new Tauri command is expected. The existing `workbench_rewrite_terminal_keys` command and the current PTY adopt/move-guard path should be reused so running terminal sessions and agent CLIs survive the move.

## Decisions

- Center-drop keeps the current swap behavior.
- Edge-drop moves the dragged terminal into the target slot as a split pane.
- The operation is a move, not a duplicate: the source slot disappears from the outer grid.
- A drag source must stay a single-pane slot for this feature. Multi-pane source slots remain non-draggable, matching current behavior.
- Target slots may already have multiple panes.
- The split model remains flat: one target slot has one `TerminalSplitAxis` and an ordered list of panes. No nested split tree is introduced.
- `SplitLeft` and `SplitRight` set the target axis to `Vertical`.
- `SplitTop` and `SplitBottom` set the target axis to `Horizontal`.
- `SplitLeft` and `SplitTop` insert the moved pane at the beginning of the target pane list.
- `SplitRight` and `SplitBottom` insert the moved pane at the end of the target pane list.
- If a target slot already has multiple panes and the user drops on a different axis, the slot switches to the new axis, same as the existing split buttons already do.
- Existing cross-workspace transfer, extract-to-new-workspace, popout ownership, unread notifications, and session resume must continue to work.
- Work must build on the current dirty tree; do not reset or overwrite unrelated in-flight changes in `state.rs`, Tauri bridge, popout code, or titlebar files.

## Implementation Notes

### Current Repo Facts

- Dirty-tree guard at implementation start: `git status --short --untracked-files=all` was clean on branch `feature/extend-terminal-dnd`; no unrelated uncommitted files need to be protected from task commits.
- DnD helper module: `src/workbench/terminal_slot_dnd.rs`.
- Drag preview module: `src/workbench/terminal_slot_drag_overlay.rs`.
- Terminal cell component: `src/workbench/terminal_cell.rs`.
- Grid/drop target component: `src/workbench/workspace_panel.rs`, especially `TerminalSlotSurface`.
- Workbench state and pure slot helpers: `src/workbench/state.rs`.
- Current terminal key format: `{storage_key}:{slot_id}:{pane_id}`.
- Existing per-slot split persistence: `WorkspaceEntry.slot_pane_states: Vec<SlotPaneState>`.
- Existing PTY adoption path: `WorkbenchService::begin_terminal_move`, `take_terminal_adopt`, and `workbench_rewrite_terminal_keys`.

### Data Model

- Extend `SlotPaneState` so pane identity can survive drag-to-split:
  - Keep existing `axis`, `pane_ids`, and `next_pane_id`.
  - Add a serde-defaulted per-pane metadata vector, for example `pane_agents: Vec<SlotPaneAgentState>`.
  - `SlotPaneAgentState` should carry at least `agent_label`, `agent_model`, and `agent_effort`.
- Preserve backwards compatibility:
  - Old snapshots with no pane metadata deserialize successfully.
  - When pane metadata is missing for a pane, resolve from the slot-level parallel arrays: `slot_agent_labels`, `slot_agent_models`, `slot_agent_efforts`.
  - Existing single-pane slots should behave exactly as before.
- Normalize pane metadata wherever `SlotPaneState` is read or mutated:
  - Ensure metadata length can be shorter than `pane_ids`.
  - Avoid panics and out-of-bounds indexing.
  - Do not eagerly rewrite every old snapshot just because metadata is absent.

### State Operation

- Add a pure helper in `state.rs`, for example:
  - `move_workspace_slot_into_split(workspace, source_slot_id, target_slot_id, edge) -> Result<TerminalSlotMove, String>`
  - Or a same-workspace-specific move result if `TerminalSlotMove` is too cross-workspace-shaped.
- Add a public service wrapper:
  - `WorkbenchService::move_terminal_slot_into_split(workspace_id, source_slot_id, target_slot_id, edge) -> Result<..., String>`.
- The operation must:
  - Reject same source/target slot.
  - Reject missing workspace or missing slots.
  - Reject source slots whose pane state has more than one pane.
  - Read source slot agent label/model/effort and source pane id before mutation.
  - Allocate a fresh target pane id from target `next_pane_id`.
  - Insert the new pane at start or end based on the drop edge.
  - Set target split axis based on the edge.
  - Remove source slot id and its parallel slot arrays.
  - Remove source slot name override.
  - Reduce `terminal_count` and recompute grid rows/cols through `set_count_and_dims`.
  - Keep `slot_ids`, `slot_agent_labels`, `slot_agent_models`, `slot_agent_efforts`, and `slot_pane_states` aligned.
  - Return exactly one terminal key rewrite pair: old source key -> new target pane key.
- After the state mutation, call the existing move-guard/adopt flow with that key pair:
  - live `pty_sessions` map should move to the new key,
  - notifications should move to the new key,
  - focused-terminal key should update,
  - source cell cleanup should skip `pty_kill`,
  - target pane bootstrap should adopt the existing PTY,
  - Tauri-side `sessions.json` and `notifications.json` should be rewritten by `workbench_rewrite_terminal_keys`.

### Drop Intent

- Replace `GhostPos` with a richer drop target, for example:
  - `TerminalSlotDropAction::{Swap, SplitTop, SplitBottom, SplitLeft, SplitRight}`.
  - `TerminalSlotDropTarget { target_slot_id, action, rows, cols }`.
- Compute drop action from target slot `get_bounding_client_rect()` and cursor coordinates:
  - Use a center zone for swap.
  - Use top/bottom/left/right edge zones for split.
  - A practical rule: center 40 percent of width and height is swap; outside that, choose the closest edge by normalized distance.
  - Clamp or fallback to `Swap` if geometry is unavailable.
- Keep current WebView2-safe behavior:
  - Continue accepting dragenter/dragover based on `slot_dnd.session_active()` or MIME type.
  - Continue using `slot_dnd.active` as the reliable source during protected `DataTransfer` phases.
  - Let the final drop handler revalidate the payload.

### UI

- Keep the existing floating drag preview.
- Add directional hover classes to `ws-term-slot` for split zones:
  - `ws-term-slot--drop-swap`
  - `ws-term-slot--drop-split-top`
  - `ws-term-slot--drop-split-bottom`
  - `ws-term-slot--drop-split-left`
  - `ws-term-slot--drop-split-right`
- Render an overlay matching the screenshots:
  - dashed border around the active target zone,
  - translucent teal/green fill only over the selected half/side,
  - label pill centered on the selected zone: "Split Top", "Split Bottom", "Split Left", "Split Right", or "Swap".
- Update the existing drop hint:
  - `WsTermDropHere` remains the swap label.
  - Add keys for the four split directions.
  - Add locale entries at least for all existing locale modules so builds do not fail.
- Keep pointer-events disabled on xterm while slot DnD is active so terminal contents do not swallow dragover/drop.

### Rendering And Runtime Lookup

- Resolve pane-level agent info when rendering each `WorkspaceTerminalCell`.
- `WorkspaceTerminalCell` should receive the pane's effective agent slug, model, and effort indirectly through the terminal key lookup path or explicit props; choose the smallest change that keeps existing launch behavior correct.
- Update state lookup helpers that currently infer agent/model/effort only from `slot_id`:
  - `agent_slug_for_terminal_key`
  - `agent_model_for_terminal_key`
  - `agent_effort_for_terminal_key`
  - `notification_ack_keys_for_terminal`
  - `notification_key_is_live`
- Ensure a dragged Codex/Claude/Gemini/etc terminal keeps:
  - its header agent label,
  - its launch metadata after remount,
  - its unread badge behavior,
  - its resume/session metadata after app restart.

### Compatibility Notes

- Do not change the Tauri PTY command surface unless an implementation blocker appears.
- Do not change `src-tauri/src/pty_host.rs` unless adoption fails with the existing key remap.
- Do not modify workspace sidebar cross-workspace drag behavior except where type changes require compile fixes.
- Do not make outer grid slot count increase for drag-to-split; it decreases by one because the source slot is absorbed into the target slot.
- Do not silently overwrite an existing target terminal key in `sessions.json` or `notifications.json`; rely on existing rewrite skip semantics.

## Tests

- Rust unit tests for the pure state helper:
  - source slot moves into target as `SplitRight`,
  - source slot moves into target as `SplitLeft`,
  - source slot moves into target as `SplitTop`,
  - source slot moves into target as `SplitBottom`,
  - source slot is removed from outer grid,
  - target pane order is correct,
  - target axis is correct,
  - source agent label/model/effort become the new pane metadata,
  - old and new terminal key pair is correct,
  - missing old pane metadata falls back to slot-level metadata,
  - same-slot drop errors,
  - unknown slot errors,
  - multi-pane source errors,
  - one-slot workspace source errors if removing the source would violate workspace invariants.
- Rust service-level test with `Owner::new()` if practical:
  - live PTY session registered under old key is moved to the new key,
  - adoption pending contains the new key,
  - move guard contains the old key,
  - focused terminal key rewrites to the new key.
- Existing regression tests:
  - swap tests remain green,
  - cross-workspace transfer tests remain green,
  - terminal key pair tests remain green,
  - popout adoption tests remain green.
- Commands:
  - `cargo test -p blxcode-ui workbench::state`
  - `cargo test --manifest-path src-tauri/Cargo.toml workbench_state`
  - `cargo check --workspace --locked`
- Manual Tauri verification:
  - drag center onto another terminal -> swap,
  - drag top edge -> target splits with moved terminal above,
  - drag bottom edge -> target splits with moved terminal below,
  - drag left edge -> target splits with moved terminal left,
  - drag right edge -> target splits with moved terminal right,
  - running shell keeps output/history after move,
  - running agent CLI keeps running and is not relaunched,
  - unread notification follows the moved terminal,
  - app restart preserves split layout and agent labels,
  - cross-workspace drag from sidebar still works,
  - extract-to-new-workspace still works,
  - popout terminal behavior still works.

## Tasks

### P0 Analysis And Safety

- [x] `dnd-state-audit` - Re-read current `TerminalSlotSurface`, `SlotPaneState`, and PTY adoption code before editing
- [x] `dirty-tree-guard` - Record current unrelated modified files and avoid reverting user/in-flight changes
- [x] `drop-intent-spec` - Document final geometry thresholds for swap vs split zones in code comments or tests

### P1 State Model

- [x] `pane-agent-state-type` - Add serde-compatible per-pane agent metadata type
- [ ] `slot-pane-state-compat` - Extend `SlotPaneState` with defaulted pane metadata while keeping old snapshots valid
- [ ] `pane-agent-resolver` - Add helper to resolve pane agent/model/effort with slot-level fallback
- [ ] `pane-state-normalization` - Add small helpers for aligned pane id and pane metadata mutation

### P2 Move Into Split Operation

- [ ] `split-drop-action-type` - Add Rust enum for `Swap`, `SplitTop`, `SplitBottom`, `SplitLeft`, and `SplitRight`
- [ ] `move-into-split-helper` - Add pure state helper that removes source slot and inserts it as target pane
- [ ] `move-into-split-errors` - Return explicit errors for same slot, missing slots, multi-pane source, and invalid workspace state
- [ ] `move-into-split-keypair` - Return old-to-new terminal key pair for the moved pane
- [ ] `move-into-split-service` - Add `WorkbenchService::move_terminal_slot_into_split` wrapper
- [ ] `move-into-split-adoption` - Reuse existing move-guard/adopt and Tauri key rewrite flow for same-workspace split moves
- [ ] `move-into-split-layout-tick` - Bump terminal layout after the move so xterm panes refit

### P3 Rendering And Runtime Lookups

- [ ] `render-pane-agent` - Render each pane with its effective agent slug instead of only the parent slot slug
- [ ] `launch-pane-agent` - Ensure adopted and freshly spawned panes resolve the correct model and effort from terminal key
- [ ] `notification-pane-agent` - Update notification liveness and ack helpers to understand pane-level agent metadata
- [ ] `live-keys-pane-meta` - Ensure live terminal keys and pruning still include every pane after split moves
- [ ] `manual-split-pane-meta` - Update existing split buttons so manually created panes get deterministic metadata

### P4 Drag UI

- [ ] `dnd-ghost-action` - Replace ghost state with target slot plus drop action
- [ ] `dnd-action-from-geometry` - Compute swap/split direction from dragover cursor and target rect
- [ ] `dnd-drop-dispatch` - Dispatch swap for center drops and move-into-split for edge drops
- [ ] `dnd-drop-validation` - Preserve WebView2-safe dragenter/dragover acceptance and final drop revalidation
- [ ] `split-drop-overlay-css` - Add directional CSS overlays matching the screenshots
- [ ] `split-drop-hints` - Render "Swap" and split direction labels from i18n
- [ ] `split-drop-i18n` - Add split direction locale strings to all locale modules
- [ ] `drag-preview-regression` - Confirm existing floating drag preview and source dimming still work

### P5 Verification

- [ ] `unit-tests-state` - Add pure state tests for all split directions and error cases
- [ ] `unit-tests-adoption` - Add service-level tests for PTY key move/adoption where practical
- [ ] `regression-tests-existing` - Run existing swap, transfer, key-pair, and popout tests
- [ ] `cargo-check-workspace` - Run `cargo check --workspace --locked`
- [ ] `manual-tauri-dnd` - Manually verify swap and all four split drops in Tauri
- [ ] `manual-tauri-runtime` - Manually verify running shell, running agent, notifications, restart, cross-workspace drag, extract, and popout
