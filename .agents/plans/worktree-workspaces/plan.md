# Worktree Workspaces

## Summary

Make Git worktrees first-class BLXCode workspaces. A worktree workspace has its
own `cwd`, terminal grid, agent timeline, session resume identity, and
workspace-local `.agents` rules, skills, memory, plans, and tasks. The feature
must work for local workspaces and SSH remote workspaces, and the built-in
BLXCode Agent must be able to inspect, create, open, and switch worktree
workspaces safely.

The main user-visible entry point is a left-aligned Worktree Management menu in
the app titlebar, bound to the active workspace. The Create Workspace wizard and
the BLXCode Agent get the same worktree creation/opening capability.

## Decisions

- Worktrees are full workspaces, not virtual child views. Each worktree uses its
  own checked-out `.agents` content instead of sharing rules, skills, memory, or
  plans from the base workspace.
- Local and SSH remote worktrees are both in v1. Remote worktrees require `git`
  on the remote host and write permission to the requested worktree path.
- The BLXCode Agent may create a worktree only after the user asks for it, after
  checking existing worktrees, and after asking the user to confirm the exact
  base, branch, start point, path, and local/remote target.
- If a matching branch or path is already present in `git worktree list`, the
  app and Agent open/switch to the existing worktree instead of creating a new
  one.
- Worktree removal is v1-safe: only clean worktrees may be removed, and dirty
  worktrees are blocked with a clear explanation.

## Implementation Notes

- Add `WorkspaceEntry.worktree: Option<WorkspaceWorktreeMeta>` with
  `base_cwd`, `worktree_cwd`, `branch`, `head`, `git_common_dir`,
  `main_worktree_cwd`, and `created_by_blxcode`. Keep Serde defaults so old
  snapshots load as non-worktree workspaces.
- Add worktree draft fields to `CreateWorkspaceDraft`: `workspace_kind`,
  `worktree_base_cwd`, `worktree_branch`, `worktree_start_point`, and
  `worktree_path`.
- Centralize local Git root resolution in `git_info::resolve_work_tree(cwd)`,
  primarily via `git -C <cwd> rev-parse --show-toplevel`. Use it from
  `git_status`, `git_graph`, `git_sync`, and `git_commit_ai` so `.git` file
  worktrees resolve correctly.
- Add backend commands for `git_worktree_list`, `git_worktree_create`,
  `git_worktree_open_info`, and `git_worktree_remove`. For remote workspaces,
  run the equivalent commands over `RemoteExecManager`.
- Extend remote terminal spawning so `pty_spawn_remote` accepts the active
  workspace `cwd` as `remote_dir`. `WorkspaceTerminalCell` must pass the
  workspace path through, so remote terminal agents start in the worktree and
  not only in the connection preset's default directory.
- Add `src/workbench/app_titlebar/worktree_menu.rs` and mount it in the left
  titlebar cluster after `TitleBarBrand`. The menu shows active workspace
  worktree state, create/open/switch/remove actions, and disabled guidance when
  the active workspace is not a Git repository.
- Extend the Create Workspace wizard with a Worktree mode that shares the same
  backend creation/opening path as the titlebar menu.
- Extend Agent protocol and bridge types with `workspace_scope` metadata
  carrying root, optional `connectionId`, and optional worktree metadata. Update
  `system_prompt` to show active worktree branch/base/git common dir and to
  instruct the Agent to stay inside the active worktree.
- Add client tools:
  - `harness.worktree_list { baseCwd? }`
  - `harness.create_worktree_workspace { baseCwd?, connectionId?, branch, startPoint?, path?, terminalCount?, agentSlugs?, confirmed? }`
- `confirmed:false` performs validation and preview only, returning
  `requiresConfirmation:true`. `confirmed:true` creates or opens the worktree
  workspace and selects it.
- Update core skills `harness`, `git`, `environment`, and `rules-skills` with
  worktree guidance, including the Agent's required preflight and confirmation
  flow.

## Tests

- Unit-test Git root resolution for a normal repository and a Git worktree whose
  `.git` entry is a file.
- Unit-test `git worktree list --porcelain` parsing, branch/path validation, and
  existing-worktree detection.
- Unit-test old/new Serde roundtrips for `WorkspaceEntry`, `CreateWorkspaceDraft`,
  and `UserTurn`/`agent_wire` protocol mirrors.
- Unit-test remote SSH command construction for worktree commands and
  `pty_spawn_remote` `remote_dir` propagation.
- Integration-test a temp repo: create commit, create worktree, open it as a
  workspace, verify Git status/diff/graph, and verify session keys remain
  isolated by `storage_key`.
- Integration-test Agent flow: preview returns `requiresConfirmation`, existing
  worktree is detected, confirmed creation opens/selects the new workspace.
- Manual-test titlebar Worktree Management menu for local and SSH workspaces.
- Manual-test terminal CLI agents (`claude`, `codex`) launching inside the
  worktree path.
- Manual-test dirty worktree removal block and clean worktree removal.

## Tasks

- [x] `worktree-model` - Add persisted worktree metadata to workspace entries and create-workspace drafts.
- [x] `worktree-git-root` - Replace local Git root resolution with `git rev-parse --show-toplevel` based helper.
- [x] `worktree-backend-local` - Implement local worktree list/create/open-info/remove commands with validation and existing-worktree detection.
- [x] `worktree-backend-remote` - Implement remote SSH worktree list/create/open-info/remove using RemoteExecManager.
- [x] `worktree-remote-pty-cwd` - Pass workspace cwd as remote_dir into remote PTY spawning so remote terminals start in the active worktree.
- [x] `worktree-titlebar-menu` - Add left-aligned active-workspace Worktree Management menu to the BLXCode app titlebar.
- [x] `worktree-wizard` - Add Worktree mode to the Create Workspace wizard and reuse the backend worktree creation/open path.
- [x] `worktree-agent-protocol` - Extend UserTurn/agent_wire/system_prompt with workspace scope and worktree metadata.
- [x] `worktree-agent-tools` - Add harness.worktree_list and harness.create_worktree_workspace client tools with preview/confirmed behavior.
- [x] `worktree-agent-guidance` - Update core harness/git/environment/rules-skills skill docs with worktree preflight and confirmation rules.
- [ ] `worktree-existing-open` - Open or switch to existing worktree workspaces when branch/path already exists instead of creating duplicates.
- [ ] `worktree-remove-safe` - Implement clean-only worktree removal with clear dirty-state blocking.
- [ ] `worktree-tests` - Add unit, integration, and manual coverage for local/remote worktree workspace flows.
