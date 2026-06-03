# Agent Onboarding Default Role

## Summary

Add a one-time startup dialog for choosing the BLXCode Agent name and default session role. The same default role remains editable in Settings -> Agent and Settings -> Workspace, and it seeds new Create Workspace drafts.

## Decisions

- Blank agent name resolves to `BLXCody`.
- The onboarding dialog appears once for all users after the next start; existing names are prefilled.
- `Settings -> Agent` and `Settings -> Workspace` edit the same stored `defaultSessionRole` value.
- The Workspace settings copy explicitly says it changes the same Agent default role.
- Cargo checks run at the end of implementation phases only.

## Implementation Notes

- Extend persisted agent provider settings with `onboardingSeen` and `defaultSessionRole`, keeping serde defaults for older files.
- Add an onboarding completion IPC command so the dialog can mark itself done without requiring the full settings pane save flow.
- Extract the Create Workspace role picker into a reusable Leptos component shared by onboarding, Create Workspace, and both settings panes.
- Keep new workspace drafts seeded from the stored default role; existing workspaces and already-open drafts are left unchanged.

## Tests

- Backend tests for nickname fallback and agent settings roundtrip defaults.
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown` at phase ends touching frontend.
- `cargo test -p blxcode` after backend changes and at final verification.
- Manual smoke: first launch dialog, defaults path, settings sync between Agent and Workspace, and Create Workspace default role.

## Tasks

- [x] `plan` - Save implementation plan and index it.
- [>] `backend-settings` - Persist onboarding and default-role settings through backend IPC.
- [ ] `shared-role-picker` - Extract reusable session role picker and keep Create Workspace behavior.
- [ ] `onboarding-dialog` - Add one-time startup dialog for agent name and default role.
- [ ] `settings-role` - Add default-role picker to Agent and Workspace settings.
- [ ] `workspace-default` - Seed new Create Workspace drafts from the saved default role.
- [ ] `final-checks` - Run final checks and finish plan/index status.
