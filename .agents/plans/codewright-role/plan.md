# Codewright Specialized Coder Role

## Summary

Add a global Coder role named **Codewright** (`slug: codewright`) for advanced, experienced, stack-agnostic programming across arbitrary codebases.

The role is enabled, uses `terminalAgentSwarm: true`, and may use both terminal-agent swarm and `subagents.run`. Implementation should use the `tauri-v2` skill for Tauri client-tool / IPC behavior, `rust-best-practices` for Rust backend state, enums, error handling, and tests, and `manage-plans` for plan maintenance.

## Decisions

- Role name defaults to **Codewright**.
- Codewright may proactively use `subagents.run` for bounded `scout`, `review`, or `security_analyst` tasks; subagents remain advisory/read-oriented and Codewright owns final edits and verification.
- "Auto-accept" means the current workspace Agent Chat mode becomes `AllowAll` immediately and remains so until the user changes it.
- Web/docs lookup is required when stack, runtime, library, or framework behavior may be stale or undocumented locally; trivial repo-local edits do not require web search.

## Implementation Notes

- Add `src-tauri/src/agent/harness_skills/specialized/codewright.md` with frontmatter:
  - `name: codewright`
  - `description: Global advanced experienced programmer...`
  - `tools: [Read, Write, Edit, Bash, Grep, Glob, Web, Memory, Git, Plans, Tasks, AskUser, Subagents]`
  - `provider: claude`
  - `models: [opus, sonnet, gpt-5, gemini-2.5-pro]`
  - `color: cyan`
  - `terminalAgentSwarm: true`
  - `enabled: true`
  - `categorie: coding`
- Register `codewright` in `session_roles.rs`; update role tests so enabled roles include `architect`, `branch-steward`, `codewright`, and `coordinator`.
- Update the system prompt subagent rule so `subagents.run` is allowed when the active role explicitly permits subagent orchestration; Codewright will permit it.
- Codewright prompt behavior:
  - Orient first with rules/skills, project docs, Memory, architecture notes, manifests, file tree, and Git status.
  - Use BLXCode Memory when present: `memory_search`, `memory_read`, architecture map, and learning writes for durable non-obvious findings.
  - Use repo docs first, then official web docs/release notes via `web_search`/`web_fetch` when needed.
  - Prefer existing codebase patterns and implement in small reviewable batches.
  - Verify with targeted tests/build/checks and inspect diffs before final reply.
  - Use terminal-agent swarm for separable implementation or verification work.
  - Use `subagents.run` proactively for bounded analysis/review/security support.
- Extend `AskUserOption` with optional `setChatModeOnSelect?: "allow_all"`.
- Update file-mutating `ToolPermissionRequest` rendering so options are:
  - `Approve once` - run only the current tool call.
  - `Auto-accept` - approve the current tool call and switch the current workspace Agent Chat mode to `AllowAll`.
- Thread the existing `chat_mode` signal into `AskUserCard`; selecting Auto-accept updates the composer pill and persists workspace mode.
- Add backend runtime override in `AgentEngineState`:
  - Reset override at `start_turn`.
  - Set override from `agent_submit_tool_result` when data includes `chatModeChangedTo: "allow_all"`.
  - `dispatch_tool` uses override before the original turn mode so the current turn stops asking immediately.
- Update user/developer docs that list built-in session roles.

## Tests

- Run `cargo test --manifest-path src-tauri/Cargo.toml agent::session_roles::tests`.
- Run `cargo test --manifest-path src-tauri/Cargo.toml agent::system_prompt::tests`.
- Add/update tests for:
  - Codewright is embedded, enabled, has provider/models/color, and `terminalAgentSwarm: true`.
  - Role-authorized subagent orchestration appears in the active role prompt behavior.
  - Permission reducer adds Auto-accept only for file-mutating tool permission prompts.
  - AskUser option parser preserves `setChatModeOnSelect`.
  - Runtime chat-mode override resets per turn and overrides `AskEdits` during the active turn.
- Manual scenario:
  - Start in `AskEdits`, activate Codewright, request a file edit.
  - Permission card shows `Approve once` and `Auto-accept`.
  - `Approve once` keeps supervised mode.
  - `Auto-accept` switches to Full Access and prevents repeated prompts in the same turn.

## Tasks

- [x] `save-plan` - Create `.agents/plans/codewright-role/plan.md` and update `.agents/plans/PLANS.md`.
- [x] `add-role` - Add and register Codewright specialized role markdown.
- [x] `allow-subagents` - Update system prompt and tests so Codewright can use `subagents.run`.
- [ ] `auto-accept-ui` - Extend permission-card AskUser options and frontend mode switching.
- [ ] `auto-accept-backend` - Add runtime chat-mode override in backend dispatch.
- [ ] `docs-tests` - Update docs and run targeted Rust/frontend tests.
