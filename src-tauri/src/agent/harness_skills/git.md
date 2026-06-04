---
name: git
description: Inspect repository status, diffs, commits, branches, and supported git mutations through dedicated workspace git tools.
categorie: git
---

# Git

Prefer dedicated `git_*` and `workspace_git_*` tools over raw `git` in `shell_exec`.

When the active workspace is a Git worktree, treat that worktree path as the
repository root for all git reads and mutations. Do not jump to the main
worktree or shared git common dir unless the user explicitly asks.

## Read tools

### `workspace_git_status`
Returns a concise status for the active workspace. Use this before edits or before summarising repository state.

### `workspace_diff { path? }`
Returns the unstaged diff for the whole workspace or one workspace-relative path.

### `git_status { porcelain? }`
Returns repository status. Use `porcelain: true` for machine-readable output when you need to reason about exact paths.

### `git_diff { path?, staged?, stat? }`
Returns unstaged or staged diffs. Use `stat: true` for a lightweight overview before reading large patches.

### `git_log { maxCount?, path? }`
Shows recent commits, optionally scoped to one path.

### `git_show { rev, path? }`
Shows a commit/object or a path inside a revision.

### `git_branch_info`
Reports current branch and upstream/ahead/behind details.

### `git_ls_files { path? }`
Lists tracked files, optionally scoped to a path.

### `git_conflicts { cwd?, maxFiles?, maxHunksPerFile? }`
Inspects current merge/rebase/cherry-pick conflicts. Returns unmerged paths, index stage entries, and conflict-marker hunks. Read-only.

When this reports conflicts, **do not resolve them silently**. Summarize the choices and ask the user before editing, staging, committing, continuing, aborting, or otherwise changing the conflict state.

## Mutating tools

### `git_apply_patch { patch }`
Applies a unified patch to the workspace. Prefer this for precise code edits when the patch is already known.

### `git_add { paths }`
Stages the listed workspace-relative paths.

### `git_commit { message }`
Creates a commit from the staged index.

Mutating operations (`git_add`, `git_commit`, `git_apply_patch`) require the `git_write` tool group and follow Agent Chat mode gates.

`push`, `reset --hard`, and `rebase` are not supported in v1.

## Patterns
- Use `workspace_git_status` or `git_status` before reporting changes.
- Use `git_diff`/`workspace_diff` before committing or reviewing edits.
- In worktree workspaces, keep branch/status/diff/commit operations scoped to the active worktree.
- Use `git_conflicts` whenever status shows unmerged paths or a merge/rebase/cherry-pick appears interrupted.
- Use `shell_exec` for git only when no dedicated tool exists, and never for unsupported destructive operations unless the user explicitly asks and the active mode allows it.
