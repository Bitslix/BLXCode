# Git

Prefer dedicated `git_*` and `workspace_git_*` tools over raw `git` in `shell_exec`.

Mutating operations (`git_add`, `git_commit`, `git_apply_patch`) require the `git_write` tool group.

`push`, `reset --hard`, and `rebase` are not supported in v1.
