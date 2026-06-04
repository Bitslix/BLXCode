---
name: environment
description: Detect workspace runtime facts such as OS, architecture, shell, git availability, and workspace root before shell or git work.
categorie: workspace
---

# Environment

## `environment_detect {}`

Detect OS, architecture, default shell, git availability, and workspace root.

**Required** before `shell_exec` or any `git_*` tool in the same session.

If the system prompt says the active workspace is a Git worktree, the detected
workspace root is the worktree root. Rules, skills, memory, plans, tasks, shell
commands, and git tools should stay there unless the user explicitly asks to
inspect another checkout.
