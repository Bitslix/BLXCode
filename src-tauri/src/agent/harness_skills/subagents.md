# Subagents

## `subagents.run { agents, mode?, maxConcurrency? }`

Run parallel subagents **only when the user explicitly asks** (e.g. "use subagents", "parallel review").

Roles: `scout`, `review`, `security_analyst`. Each agent must finish via `submit_result` (subagent-only tool).

Max 5 agents, default concurrency 3. Subagents cannot call `subagents.run` or `shell_write`.
