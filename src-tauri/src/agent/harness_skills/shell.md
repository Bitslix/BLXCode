# Shell

## `shell_exec { command, writes? }`

Run a **non-interactive** command in the workspace directory.

- Use **harness** terminal tools for interactive CLIs (`harness.open_terminal`, `harness.send_terminal_keys`, `harness.send_agent_context`).
- For terminal CLI agents (`claude`, `codex`, `gemini`, `opencode`, `cursor`), prefer the PTY workflow: open/list terminal, send prompt/context, wait with `harness.wait_terminal_output`, inspect with `harness.read_terminal_output`, interrupt with `harness.terminal_interrupt` if needed.
- Use `shell_exec` for one-shot commands whose output can finish and return normally. Do not use it to drive TUIs or long-lived agent conversations.
- Read-only mode uses an allowlist unless `shell_write` group is granted (coordinator only in v1).
- In `Ask Edits`, the exact command is shown to the user before execution.
- In `Allow all`, commands run without asking.
- In `Plan`, read-only commands may run, but `writes:true` commands are blocked.
