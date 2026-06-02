# Handoff — CLI "latest top model" + reasoning "effort" level

**Date:** 2026-06-02
**Context:** Continuation of `workspace-presets-and-session-roles.md`. Previous
agent hit the context limit. Everything from that plan + its provider/model
addendum is **implemented, compiles, and 271 backend tests pass**. This handoff
covers the NEXT requested change only.

## What the user asked for (this turn)

1. **Always use the latest top model** in the per-CLI model catalogs (replace
   the current placeholder model IDs with current latest/top models).
2. **Add an "effort" (reasoning effort) level** that is passed to the CLI agent
   alongside the model.
3. (Done here) Web research on how each agent CLI names/maps/passes model +
   effort. Findings below — use them; don't re-research from scratch.

## Current state of the relevant code (already implemented last turn)

- `src/workbench/terminal_agent_profiles.rs`
  - `TerminalAgentProfile` now has `model_flag: &'static str` + `models: &[&str]`.
  - `terminal_agent_launch_command(slug, resume_id, model)` appends
    `<model_flag> '<model>'` when a model is set.
  - `terminal_agent_models(slug)` returns the catalog.
  - **THESE catalogs are placeholders and must be updated to latest top models.**
- `src/workbench/state.rs`
  - `WorkspaceEntry.slot_agent_models: Vec<String>` (parallel to
    `slot_agent_labels`), `#[serde(default)]`.
  - `CreateWorkspaceDraft.agent_models: [String; 5]` (per agent row).
  - `fleet_slot_models_for_labels()` expands draft rows → per-slot models on
    commit. `agent_model_for_terminal_key()` resolves a slot's model at launch.
  - Helper `set_workspace_agent_model(id, idx, model)`.
- `src/workbench/terminal_cell.rs` — `AgentLaunchPending.model` sourced via
  `agent_model_for_terminal_key`, passed to `build_launch_command(slug, resume, model)`.
- `src/workbench/create_workspace_wizard.rs` — per-agent model `<select>` in the
  fleet step (step 2); role dropdown subline shows provider/models.
- Backend role frontmatter: `model:` → `provider:` + `models:` (advisory only).
  `src-tauri/src/agent/session_roles.rs` (`RoleMeta { provider, models, ... }`).
  All 7 `harness_skills/specialized/*.md` updated.
- Presets carry `agent_models[5]` (`src-tauri/src/workspace_presets.rs` +
  `tauri_bridge.rs WorkspacePresetView` + draft apply/save).
- **BLXCode Agent itself always uses the Settings provider/model** — role
  provider/models are advisory and never change its runtime model. Keep this.

## Web research findings — model + effort per CLI (USE THESE)

Key takeaway: **`--model` is uniform, but "effort" is NOT a uniform CLI flag.**
Each CLI passes reasoning effort differently — some via flag, some via `-c
key=val`, some via env var, some only via a config file. The `effort` feature
must therefore be a **per-CLI mapping**, not a single flag appended for all.

### claude (Claude Code)
- Model: `claude --model <id|alias>`. Aliases `opus` / `sonnet` / `haiku`
  always resolve to the **latest** version → using the alias satisfies "always
  latest top model". Full IDs e.g. `claude-opus-4-8`, `claude-sonnet-4-6`,
  `claude-haiku-4-5`.
- Effort: levels `low | medium | high | xhigh` (persist) + `max` (session).
  Passed via env var **`CLAUDE_CODE_EFFORT_LEVEL=high`** (or interactive `/model`).
  No confirmed plain `--effort` CLI flag → use the env-var prefix on the launch
  command (e.g. `CLAUDE_CODE_EFFORT_LEVEL=high claude --model opus`).
- Source: https://code.claude.com/docs/en/model-config

### codex (OpenAI Codex CLI)
- Model: `codex -m <id>` / `--model`. Latest/top: `gpt-5.2-codex`,
  `gpt-5.1-codex-max`.
- Effort: **`-c model_reasoning_effort="high"`** (repeatable `-c`). Values
  `minimal | low | medium | high | xhigh` (xhigh only on `gpt-5.1-codex-max` /
  `gpt-5.2-codex`). Example: `codex -m gpt-5.2-codex -c model_reasoning_effort="xhigh"`.
- Source: https://shipyard.build/blog/codex-cli-cheat-sheet/ ,
  https://codex.danielvaughan.com/2026/03/27/reasoning-effort-tuning/

### gemini (Gemini CLI)
- Model: `gemini --model <id>`. Latest/top: `gemini-3-pro`, then `gemini-3-flash`,
  `gemini-2.5-pro`.
- Effort: **config-file only** (no reliable CLI flag yet). Gemini 3 uses
  `thinkingLevel` (LOW/MEDIUM/HIGH) in `~/.gemini/settings.json`; 2.5 uses
  `thinkingBudget`. → effort cannot be passed on the launch command for gemini;
  treat as "model only" or write the config (out of scope for a simple launch).
- Source: https://geminicli.com/docs/cli/model/ ,
  https://github.com/google-gemini/gemini-cli/issues/25122

### opencode
- Model: `opencode --model <provider_id/model_id>` / `-m`. e.g.
  `anthropic/claude-opus-4-8`, `openai/gpt-5`. (Format is `provider/model`.)
- Effort: **config-file only** — `reasoningEffort` (`low|medium|high`) per model
  variant in `.opencode.json`. No CLI flag. Treat as model-only on launch.
- Source: https://opencode.ai/docs/models/ , https://opencode.ai/docs/cli/

### cursor (cursor-agent)
- Model: `cursor-agent -m <id>` / `--model`. Latest/top: `gpt-5.5` (defaults to
  medium effort), `gpt-5.4`, `gpt-5.3-codex`, `sonnet-4.5`, `sonnet-4-thinking`.
- Effort: values `none|minimal|low|medium|high|xhigh` (model-dependent). The
  CLI passing mechanism was a feature request and is **uncertain** — verify the
  installed `cursor-agent --help` before wiring a flag.
- Source: https://forum.cursor.com/t/cursor-cli-gpt-5-reasoning-effort/131653 ,
  https://cursor.com/docs/models/gpt-5-3-codex

## Recommended implementation plan for the next agent

1. **Update catalogs to latest top models** in `terminal_agent_profiles.rs`
   (put the top model first; prefer claude *aliases* so it auto-tracks latest):
   - claude: `["opus", "sonnet", "haiku"]` (aliases = latest — keep as is, OK).
   - codex: `["gpt-5.2-codex", "gpt-5.1-codex-max", "gpt-5"]`.
   - gemini: `["gemini-3-pro", "gemini-3-flash", "gemini-2.5-pro"]`.
   - opencode: `["anthropic/claude-opus-4-8", "openai/gpt-5"]`.
   - cursor: `["gpt-5.5", "gpt-5.3-codex", "sonnet-4.5"]`.
   ⚠️ Verify each against the *installed* CLI version (`<bin> --help` /
   `<bin> --model ?`) — these IDs drift fast. Keep "Default" (empty) as the
   safe first UI option.

2. **Add an `effort` mechanism per profile** (because it's not uniform). Extend
   `TerminalAgentProfile` with how effort is passed, e.g.:
   ```rust
   pub enum EffortPassing {
       None,                       // gemini, opencode (config-only) → skip on launch
       Flag(&'static str),         // e.g. cursor if confirmed: "--reasoning-effort"
       ConfigOverride(&'static str), // codex: "-c model_reasoning_effort=" → `-c model_reasoning_effort="high"`
       EnvVar(&'static str),       // claude: "CLAUDE_CODE_EFFORT_LEVEL"
   }
   pub effort_passing: EffortPassing,
   pub efforts: &'static [&'static str], // selectable levels for THIS cli
   ```
   - claude → `EnvVar("CLAUDE_CODE_EFFORT_LEVEL")`, efforts `["low","medium","high","xhigh","max"]`.
   - codex → `ConfigOverride("model_reasoning_effort")`, efforts `["minimal","low","medium","high","xhigh"]`.
   - cursor → confirm flag name first; else `None`.
   - gemini, opencode → `None` (document that effort is config-file only).
   - Extend `terminal_agent_launch_command(slug, resume_id, model, effort)` to
     render the right form: env-var **prefix** (`KEY=val claude …`), `-c key="val"`
     segment (codex), or a flag. Quote with the existing `shell_single_quoted_arg`.

3. **Plumb effort exactly like model** (mirror everything done for `model`):
   - `WorkspaceEntry.slot_agent_efforts: Vec<String>` (+ serde default, keep
     parallel on add/remove/swap), `CreateWorkspaceDraft.agent_efforts: [String;5]`,
     `fleet_slot_efforts_for_labels`, `agent_effort_for_terminal_key`,
     `set_workspace_agent_effort`, `apply_preset_to_draft` extra arg,
     `WorkspacePreset(View).agent_efforts`, `AgentLaunchPending.effort`.
   - Wizard fleet step: an **effort `<select>`** next to the model select, options
     from `profile.efforts` (+ "Default"). Only show when the CLI supports effort.

4. **i18n**: add `WzAgentEffortLabel` (+ maybe `WzAgentEffortDefault`, or reuse
   `WzAgentModelDefault`) to **all 15 locales** (`src/i18n/keys.rs` +
   `src/i18n/locales/*.rs`). Pattern: insert after the model keys (see how
   `WzAgentModelLabel`/`WzAgentModelDefault` were added via a Python loop last
   turn — search git history / those keys).

5. **Tests**: extend `terminal_agent_profiles` tests for each effort-passing
   form (env prefix, `-c` override, flag, none). Update the preset round-trip
   test to assert `agent_efforts`. Update any `WorkspaceEntry`/`WorkspacePreset`
   literal in tests (there are ~9 `WorkspaceEntry {` literals in `state.rs` —
   last time a Python insert-after-`slot_agent_labels` loop handled them; do the
   same for `slot_agent_efforts`). Two test literals use `vec![...]` forms and
   need manual edits (search `slot_agent_labels: vec!`).

6. **Docs**: update `docs/user/workspaces.md` (effort dropdown) and
   `docs/developer/agent-harness.md` ("CLI-agent model selection (fleet)"
   section) to add the effort mapping table above + note gemini/opencode are
   config-only.

## Gotchas learned last turn (save time)

- The Leptos `view!` macro treats a bare `>` in an attribute as a tag close.
  Wrap comparisons in parens: `when=move || (draft.get().agent_counts[idx] > 0)`.
- `Edit` requires a prior `Read` of the file in *this* session; files touched
  only via `Bash sed/cat` still need a `Read` first.
- Adding a non-`Option`/non-defaulted field to `WorkspaceEntry` breaks ~9
  struct literals in `state.rs` (incl. 2 test `vec!` ones) — script the inserts.
- Verify after each phase: `cargo check -p blxcode` and
  `cargo check -p blxcode-ui --target wasm32-unknown-unknown`, then
  `cargo test -p blxcode`.
- BLXCode Agent (the coordinator/role-wearer) must keep using **Settings**
  provider/model. Effort here is for the **terminal CLI agents only**, same as
  the model selection.

## Sources

- Claude Code: https://code.claude.com/docs/en/model-config
- Codex: https://shipyard.build/blog/codex-cli-cheat-sheet/ ·
  https://codex.danielvaughan.com/2026/03/27/reasoning-effort-tuning/
- Gemini CLI: https://geminicli.com/docs/cli/model/ ·
  https://github.com/google-gemini/gemini-cli/issues/25122
- OpenCode: https://opencode.ai/docs/models/ · https://opencode.ai/docs/cli/
- Cursor: https://forum.cursor.com/t/cursor-cli-gpt-5-reasoning-effort/131653 ·
  https://cursor.com/docs/models/gpt-5-3-codex
