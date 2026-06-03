# Interactive Kanban Drag And Drop

## Summary

Make the Workspace Kanban board interactive: whole plans can be dragged between Kanban status sections, and subtasks can be dragged within their parent plan across status lanes or within a lane. Plan Markdown remains the source of truth, the visual tree stays `section -> plan -> subtasks`, and the right Plans panel stays synchronized through the shared plans epoch.

## Decisions

- Plan drops write real task status changes instead of only visual overrides.
- Plan status changes use a minimal-change policy so existing task history is preserved where possible.
- Subtasks can move only within their current plan; cross-plan task drops are rejected.
- Subtask drag changes both status and Markdown order.
- Kanban drag visuals reuse the existing terminal/context DnD token style: dashed drop zones, source dimming, accent glow, and cursor-following overlay.

## Implementation Notes

- Add a dedicated Kanban DnD service and overlay rather than mixing Kanban payloads into terminal-slot DnD.
- Add backend commands for plan moves and task moves; both rewrite canonical plan task lines through the existing plan parser/rewriter.
- Keep `.agents/kanban/index.json` as layout metadata only. Persist plan-card ordering in `layout.plan_order`; persist task ordering in the Markdown `## Tasks` section.
- After every successful drop, bump `WorkbenchService::plans_epoch()` so Workspace Kanban and the right Plans panel reload from the same source.

## Tests

- Backend tests for minimal plan-state transitions, task reorder within a lane, task move across lanes, and preservation of non-task Markdown sections.
- Frontend compile check for Leptos/WASM.
- Manual checks for plan drag, task drag, rejected cross-plan task drops, visual drop zones, overlay positioning, and Plans panel sync.

## Tasks

- [x] `save-plan` - Save and index the implementation plan
- [x] `backend-move-commands` - Add Kanban plan and task move backend commands
- [x] `frontend-dnd-state` - Add Kanban drag service and overlay
- [x] `kanban-ui-drops` - Wire plan and task drag/drop into the Kanban tree UI
- [x] `visual-polish` - Add dashed drop zones and overlay styling with existing tokens
- [x] `verify` - Run focused tests and compile checks
