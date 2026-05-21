# Shell

## `shell_exec { command, writes? }`

Run a **non-interactive** command in the workspace directory.

- Use **harness** terminal tools for interactive CLIs (`harness.open_terminal`, `harness.send_terminal_keys`, `harness.send_agent_context`).
- Read-only mode uses an allowlist unless `shell_write` group is granted (coordinator only in v1).
