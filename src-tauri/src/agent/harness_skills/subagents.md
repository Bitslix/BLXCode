# Subagents

## `subagents.run { agents, mode?, maxConcurrency? }`

Run parallel subagents **only when the user explicitly asks** (e.g. "use subagents", "parallel review").

Roles: `scout`, `review`, `security_analyst`. Each agent must finish via `submit_result` (subagent-only tool).

Max 5 agents, default concurrency 3. Subagents cannot call `subagents.run` or `shell_write`.

### `allowedToolGroups` (optional, per-agent)

**Omit this field** unless you specifically need to narrow a role's default toolkit. Every role already ships with a reasonable default toolset.

If you set it, the value MUST be drawn from this exact list. Anything else is rejected and the role's defaults are used instead:

- `environment_read` — `environment_detect`
- `workspace_read` — `list_workspace_files`, `read_workspace_file`, `workspace_search`
- `diff_read` — `workspace_git_status`, `workspace_diff`
- `git_read` — `git_status`, `git_diff`, `git_log`, `git_show`, `git_branch_info`, `git_ls_files`
- `memory_read` — memory note read tools
- `plans_read` — plan read tools
- `tasks_read` — `task_list`, `task_get`
- `rules_skills_read` — `rules_list`, `rules_read`, `skills_list`, `skills_read`
- `web_read` — `web_search`, `web_fetch` (only if web is enabled)

Do NOT invent names like `"file_access"`, `"files"`, or `"shell_exec"` — they will all be rejected and the subagent will see no narrowing.
