# Changelog

All notable changes to BLXCode are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Center multi-view tabs**: the workspace pane now hosts a VS Code-style tab strip above the terminal grid. The pinned **Terminals** tab is non-closeable and always renders the existing PTY layout; additional tabs are opened dynamically and closed via the strip. Per-workspace state (`center_tabs`, `center_active_tab_id`, `center_next_tab_id`) is persisted in the workspace snapshot with tolerant serde defaults so older snapshots load cleanly and self-heal to include the Terminals tab. Switching tabs hides (rather than unmounts) the terminal grid so xterm sessions and PTYs are never recreated. Active-tab tracking is wired into `is_workspace_active` so terminal focus/resize observers only fire when the Terminals tab is visible.
- **File preview tab**: clicking a file row in the sidebar Project Explorer opens (or reuses) a center tab that renders the file's text contents. New Tauri command `read_workspace_text_file` (`src-tauri/src/fs_entries.rs`) reads UTF-8 text under the workspace root with the same `canonical_root` / `resolve_under_root` sandbox the existing `list_path_entries` uses, hard-caps at 512 KiB (`MAX_TEXT_PREVIEW_BYTES`) and returns `{ content, truncated, byteLen }`. Non-text payloads surface `file is not valid UTF-8 text`, directories `not a file`, and out-of-root paths the existing `outside workspace` error. The new `tauri_bridge::read_workspace_text_file` wrapper plus `TextFilePreview` mirror the payload shape. 4 unit tests pin the happy path, traversal rejection, directory rejection, and missing-file rejection.
- **Docked settings tab**: the harness settings UI moved out of the modal `SettingsChrome` overlay and into a `SettingsDock` component rendered inside a dynamic center tab. Opening settings (command palette `Open Settings`, etc.) now calls `WorkbenchService::open_center_settings_tab` which reuses any existing settings tab for the active workspace instead of stacking duplicates. The legacy scrim + focus-trap helpers (`focus_first_settings_control`, `settings_focusables`, `trap_settings_tab`) and `HarnessUiService::open_settings` are gone; `HarnessSettingsCategory` is now `Serialize/Deserialize` so it can ride along with the snapshot. The chat header, right panel and other categories continue to interact with the existing `settings_category` signal.
- **GitHub Releases auto-updater**: BLXCode now wires Tauri v2's updater/process plugins with a signed GitHub Releases `latest.json` endpoint, desktop IPC for `app_version`, `updater_check`, `updater_install_start`, progress polling, and relaunch, plus a Leptos startup banner and themed update dialog with release notes, progress, speed, retry, and restart states. Settings -> App gains a persisted startup auto-check toggle and manual update check. Release automation now supports the hybrid flow: macOS/Windows artifacts are built in GitHub Actions, Linux artifacts can be built locally to save CI cost, `.deb`/`.rpm` stay manual download assets, and signed updater-capable payloads are merged into one canonical `latest.json`. Tauri updater signing is documented as separate from Apple/Microsoft installer certificates.
- **Boot loading screen**: new branded loading experience that paints before the WASM bundle is ready and stays on screen until the workbench is mounted. `index.html` ships a static `#blx-static-boot` section (logo, eyebrow, faux workbench preview, animated rail) so the first paint happens immediately on Trunk-served HTML; once Leptos mounts, the static node is removed and `BootLoadingScreen` (`src/boot_loading.rs`) takes over with phased copy (`Starting BLXCode` → `Restoring workspace` → `Opening workbench`). 306 lines of dedicated CSS in `styles.css` add the radial-gradient backdrop, frame-enter / sheen / rail keyframes, and the sidebar/main/panel preview skeleton. The `App` boot fallback now renders `<BootLoadingScreen phase=app_boot_phase.get()/>` instead of the prior empty `app-shell--boot` div.
- **Agent question card (`harness.ask_user`)**: new client-side tool that lets the coordinator agent ask a clarifying multiple-choice question and receive the user's answer as a structured tool result. Backend registers `harness.ask_user` (`src-tauri/src/agent/tools.rs`) with a JSON schema accepting `question`, optional `header` (≤12 chars), 2–4 `options` (label + optional description), `multiSelect`, and `allowOther`; the tool sits in the `CoordinatorHarness` group (subagents excluded) and the system prompt instructs the model to use it only when 2–4 distinct options would unblock progress. Frontend adds a new `TimelineItem::AskUser` variant and `ask_user_card/` component (Angular-style folder with co-located CSS) rendering a chat bubble with numbered option buttons, single- or multi-select mode, an optional free-text “Other” field, and a cancel button; the card submits via `agent_submit_tool_result` and transitions the row state to `Answered { selected, other }` or `Cancelled` so the bubble stays visible with disabled controls. Persistence drops `Open` ask-user rows from the saved timeline (the awaiting backend loop is dead after reload). Tool-result payload: `{ "selected": [...], "other": "...", "cancelled": false }`; on cancel: `ok=false` + `{ "cancelled": true }`. 8 new i18n keys (`AgAskUser*`) added in `en_us.rs` and `de_de.rs`; other locales receive English placeholders pending a `render_i18n_locales_from_en.py` pass.
- **Centralized API Keys settings**: new **Settings → API Keys** pane (`src/workbench/api_keys_pane/`) replaces per-provider key inputs that previously lived under Settings → Agent. Backend module `src-tauri/src/api_keys.rs` exposes `api_keys_status` (catalog of LLM + search providers with masked values, env-var hints, and `comingSoon` placeholders for Google / Mistral / **Grok xAI**) and `api_keys_apply` (batch `set` / `delete` actions). Active keys still use the existing OS keyring accounts (`agent:*`, `agent:web:*`) with optional `BLX_*` env fallback; resolve order is keyring-first so UI-saved secrets are not overridden by shell env. The pane uses draft state, a single **Save** / **Discard** footer, row-level remove/undo, and shared `settings-field-card` / `api-keys-row` styling with brand icons (`public/brand-icons/`: OpenRouter, Anthropic, OpenAI, Google/Gemini, Mistral, Groq, Tavily, Brave). 15 new `ApiKeys*` i18n keys in all 13 locales.
- **Settings revamp (docked center tab)**: harness settings moved from a modal overlay into a **center workbench tab** (`SettingsDock` in `harness_ui.rs`). Sidebar categories: **App**, **Appearance** (stub), **API Keys**, **Workspace**, **BLXCode Agent**. Legacy `HarnessSettingsCategory` values (`Image`, `Voice`, `Memory`) still deserialize and route to the correct pane for older snapshots. Command palette / tmux chords open or focus a singleton settings tab per workspace.
- **BLXCode Agent settings pane**: `src/workbench/agent_provider_pane/` — responsive grid. **Text** (top-left): provider + **thinking level** pickers (`harness-level-picker` icons), muted `text-xs` API-key status line → Settings → API Keys, shared **`AgentModelPicker`** (catalog rows, pricing-only detail sub-row, custom model id, refresh). **Image** (top-right): `AgentImageColumn` in `harness_image_pane` — provider dropdown, **quality level** picker, `AgentModelPicker`, auto-save; **fal.ai** provider + key via API Keys catalog. **Voice** (bottom, full width): `AgentVoiceColumn` in `harness_voice_pane` — unified STT/TTS provider (OpenAI / OpenRouter / **AWS Polly**), `AgentModelPicker` for speech + TTS models, recording quality, behavior (post-STT flow, gender filter, fixed 6-voice catalog per provider, TTS autoplay). Web Tools below the grid (Tavily / Brave / disabled); one **Save** / **Discard** footer for text provider + web backend. Replaced standalone `model_picker/` with `agent_model_picker/`.
- **Voice in App settings**: STT language mode + push-to-talk hotkey moved to **Settings → App** (`voice_app_controls`); full voice provider/model/voice UI lives only under BLXCode Agent.
- **Workspace category colors**: former **Settings → Memory → Color presets** moved to **Settings → Workspace** section **Category colors** (`workspace_settings_pane/category_colors.rs`, `WsSectionCategoryColors` + related i18n). Same preset list/edit/add/reset backed by `memory_color_presets` in `WorkbenchService`.
- **Per-turn chat metrics & session cost**: tokens, TTFT, decode speed and resolved USD cost are now reported per conversation row instead of as a single aggregated footer. Each provider round emits `TurnUsage { kind: ModelRound, .. }` and each tool dispatch emits `TurnUsage { kind: ToolExec, call_id, .. }`, in both the main agent (`openrouter.rs`, `anthropic.rs`) and `subagent_runner.rs`. OpenRouter requests opt into `usage: { include: true }` and parse the native `usage.cost` field; everything else (including Anthropic Direct) is priced locally via a new `agent/pricing.rs` module — `ProviderModelEntry` carries `Option<ModelPricing>` populated from OpenRouter `/v1/models`, and direct-provider model ids route through a static id-mapping table (`claude-opus-4-7` → `anthropic/claude-opus-4.7`, etc.). A monotonic `turn_generation` counter on `AgentEngineState`, bumped by `agent_clear_conversation`, lets the frontend drop late events from a cancelled turn instead of polluting the fresh chat. New `workbench/agent_panel/turn_metrics_bar/` component renders the per-row `in · out · tok/s · ttft · $cost` strip under Assistant, Tool, the new synthetic `ModelDecision` row (tool-only rounds), and per-tool inside subagent cards. Subagent cards show aggregated round metrics in the header. The chat header gains a `SessionCostChip` with the resolved USD total + turn count, replacing the retired `ChatUsageFooter`. `ToolGroup` bundling is gone — each tool is its own timeline row now. 14 new `I18nKey` variants (`AgMetricsIn/Out/Ttft/CostUnknown/TurnsOne/TurnsMany/Tooltip*`, `AgMetricsBarAria`, `AgMetricsModelRound`, `AgSessionCostAria`) added in all 12 locale files. Pricing module ships with 7 unit tests.

### Changed

- **`ChatUsageStats` schema migrated**: `total_cost_usd: f64` and `current_turn_generation: u64` added; legacy `ttft_sum_ms` / `ttft_sample_count` fields removed (TTFT now lives per-row). Older `workbench.json` snapshots deserialize cleanly thanks to `#[serde(default)]` and rewrite without the removed fields on next save. `record_chat_turn_usage` signature changed to accept the new `turn_generation` and `cost_usd` and to return `bool` so callers can drop stale events.
- **Workspace settings layout aligned with API Keys**: the Workspace pane (`src/workbench/workspace_settings_pane/`) uses `harness-subpane` sections (**Paths & sandbox**, **Embedded browser**, **Category colors**), `api-keys-list` / `api-keys-row` field cards, and one footer **Save** / **Discard** for project directory, agent sandbox root, and embedded-browser URL. Category color edits apply immediately (same as former Memory tab). Shared footer/field CSS in `workspace_settings_pane.css` and `api_keys_pane.css`.
- **BLXCode Agent API-key hints**: relocation lines under Text / Image / Voice use `0.75rem` + `--text-muted` (`.agent-provider-pane__key-row`) instead of body-sized copy.
- **Voice catalog UX**: six fixed voices per provider (OpenAI + AWS Polly active; OpenRouter shows OpenAI set disabled with hint); gender filter always visible; disabled cards keep layout (no hide-on-disable).
- **Settings panel restructured**: `AppSettingsPane` (`src/workbench/harness_ui.rs`) now reads top-to-bottom as Language → Keyboard shortcuts → Notifications → **Terminal hooks** → **App updates** (updates moved below hooks to put install actions closer to the per-agent hook list). Keyboard shortcuts and Notifications switched from single-column lists to 2-column grids (new `.app-prefs-shortcut-modes--grid` and `.app-prefs-toggle-grid`). Current version no longer renders inside a readonly `<input>` — it's now a styled `<dl>` row, and a sibling **Available version** row only appears when `UpdateService::available_version` is `Some`. Terminal hooks list (`.harness-hooks__list--grid`) lays out as 3 columns on wide screens, collapsing to 2 / 1 below 900 px / 600 px; per-agent **status is icon-only** (check / X) with full text in `title` + `aria-label`, aligned to the end of the title row. The EULA status row was removed from the App pane (acceptance is still gated at boot; the field added no actionable signal here). Workspace pane changed the agent sandbox `<textarea>` to a single-line `<input>` and gained a new **Default project directory** field above it: persisted to `blxcode_default_project_dir_v1` via new `WorkbenchService::default_project_dir` signal + setters/persist, seeded from the user's home directory through a new `harness_user_home_dir` Tauri command (`src-tauri/src/commands.rs`) on first run. Inline workspace configuration (`start_inline_configure` and snapshot hydration) now prefills new workspace `cwd` from `default_project_dir`, falling back to the agent sandbox root only when blank — sandbox path is reserved for BLXCode Agent sandbox actions. New i18n keys `AppUpdateAvailableVersion`, `WsRootHint`, `WsDefaultProjectDirLabel/Placeholder/Hint` added across all 13 locale files (English authored; other locales carry English placeholders until `scripts/tools/render_i18n_locales_from_en.py` runs).

### Fixed

- **`scripts/tools/render_i18n_locales_from_en.py`**: `GoogleTranslator.translate()` returning `None` (recent `deep-translator` behaviour for short tokens like `"in"`, `"out"`, `"ttft"` and the bare `"—"` glyph) was cached and later crashed `rust_escape` with `TypeError: 'NoneType' object is not iterable`. The script now treats empty / `None` responses as "no translation available" and falls back to the source string; `emit_locale_rs` hard-fails with a readable `SystemExit` if a `None` still slips through.

### Removed

- **Settings → Image / Voice / Memory sidebar entries**: image, voice, and memory color configuration moved into **BLXCode Agent** (image + voice) and **Workspace** (category colors). Dedicated **Image**, **Voice**, and **Memory** nav buttons removed; legacy categories still route to the correct pane.
- **`voice_provider_voices` Tauri command** and backend `voice/voices.rs` catalog (frontend uses fixed voice lists).
- **Global `ChatUsageFooter`**: superseded by the per-row `TurnMetricsBar` + chat-header `SessionCostChip`.
- **`compact_timeline` tool bundling**: `DisplayTimelineItem::ToolGroup`, `merge_consecutive_tools` and `aggregate_status` retired — each tool now renders as its own row carrying its own metrics bar.
- **Per-provider API key fields in Agent settings**: keys are managed only under Settings → API Keys; the old Agent Provider password rows and duplicate Web Tools description were dropped in favour of short relocation hints.


## [0.2.1] - 2026-05-21

### Added

### Changed

### Fixed

- **Plans panel new-plan composer**: fixed a flexbox collapse where pressing `+` could render only the dashed composer outline while clipping the title input, Markdown editor, and actions. Composer rows now keep their intrinsic height inside the scrollable Plans body, and the title input is focused after opening.

### Removed


## [0.2.0] - 2026-05-21

### Added

- **Interactive setup scripts**: platform-specific post-clone setup helpers under `scripts/setup/` for Linux, macOS and Windows. They install/check Rust, `wasm32-unknown-unknown`, Trunk, Tauri CLI, Node/npm frontend assets, OS-native Tauri dependencies, and optional verification/bundle builds.
- **Native release script counterparts**: PowerShell and Command Prompt release entrypoints (`scripts/release.ps1`, `scripts/release.cmd`, `scripts/release-windows.*`) plus a macOS launcher (`scripts/release-macos.sh`). The PowerShell modules under `scripts/release/*.ps1` mirror the Bash release pipeline for version bumps, CHANGELOG finalization, Tauri builds, artifact discovery, and GitHub uploads.
- **BetterHarness — slim system prompt + Core Skills**: extracted all per-tool documentation from the system prompt into 6 embedded Core Skill files (`file-access`, `memory`, `plans`, `tasks`, `rules-skills`, `harness`). System prompt shrunk from ~470 to ~240 lines; tool docs are served lazily via `skills_read`. New `SkillSourceKind::Core` variant; `list_skills` prepends always-present core skills, `read_skill` serves embedded content, `remove_skill` rejects core names. Skills tab gets Core / User sub-tabs (Install hidden in Core view).
- **Kanban board view for Plans**: per-workspace board across all plan tasks with status columns, drag-and-drop for cards and columns, show/hide columns, Markdown writeback. New `manage-plans` skill documentation.
- **Skills tab — expandable SkillCard**: SkillCard now renders as an expandable card with a shadcn-style switch, lazy-loads the skill's `SKILL.md` body on first expand, and shows source-kind icon badges. SkillsTab uses a modern pill switcher for Core / User sub-tabs with counts and improved empty / error states.
- **Rules tab — expandable RuleCard with inline editing**: RuleCard mirrors the SkillCard layout, can be expanded inline, supports editing rule body, and ships a new rule-creation flow with input validation and UI polish.
- **Voice recorder**: more reliable input-device name retrieval (resolves cases where the device label fell back to a generic identifier).
- **CI — PR cargo check workflow**: new GitHub Actions workflow runs `cargo check` for both crates (`blxcode` native Linux, `blxcode-ui` `wasm32-unknown-unknown`) on every pull request, using `dtolnay/rust-toolchain` and `actions/cache`.
- **Coordinated subagents**: Coordinator can spawn up to 5 parallel subagents (`scout`, `review`, `security_analyst`) via a new `subagents.run` tool that reuses the host agent's provider, API key, model and thinking level. Each subagent runs an isolated message loop with a filtered toolkit, a forced `submit_result` JSON-schema tool, per-run caps (8 iterations, ~20k output tokens) and findings/artifact truncation (max 20/10). Recursion is blocked and `shell_write` is Coordinator-only.
- **Unified tool dispatch**: new `agent/tool_dispatch.rs` routes Anthropic, OpenRouter and OpenAI-compatible providers through a single dispatcher so all toolgroups and `subagents.run` are wired in one place.
- **Toolgroup filtering**: `ToolGroup` enum + `registry_filtered` / `render_for_openai_filtered` / `render_for_anthropic_filtered` build per-role catalogs (`environment_read`, `workspace_read`, `diff_read`, `git_read`, `git_write`, `shell_read`, `shell_write`, `web_read`, memory/plans/tasks/rules-skills read/write).
- **Environment detection**: `environment_detect` tool with session-scoped cache; `shell_exec` and `git_*` reject calls before detection has run, cache is invalidated on workspace switch.
- **Shell tool**: non-interactive `shell_exec` with workspace CWD, output/time limits, a read-only command allowlist (incl. git subcommand parsing), child-process registry and SIGTERM→SIGKILL (2s) / Windows `TerminateProcess` cancellation. `shell_write` opt-in for Coordinator only.
- **Git tools**: `workspace_git_status`, `workspace_diff`, plus generic `git_status`, `git_diff`, `git_log`, `git_show`, `git_branch_info`, `git_ls_files` (read) and `git_apply_patch`, `git_add`, `git_commit` (gated behind `git_write`).
- **Workspace search**: `workspace_search` for ripgrep-style content lookup within the workspace root.
- **Web tools**: optional `web_search` / `web_fetch` with pluggable backend (Tavily or Brave). API keys live in the OS keyring (`BLXCode` service, accounts `agent:web:tavily` / `agent:web:brave`) with `BLX_TAVILY_API_KEY` / `BLX_BRAVE_API_KEY` env fallback; tools are dropped from the catalog and the `web` skill is flagged `disabled_no_key` when no key is configured. New Settings → Agent → Web Tools block with backend picker and masked key inputs.
- **Subagent protocol & UI**: new `AgentEvent::Subagent{Started,Step,ToolCall,Finished}` events, `TimelineItem::SubagentGroup` (tolerant serde for older `sessions.json`), inline subcards with live steps and compact tool-call icons, ~50 ms debounce for parallel event bursts.
- **Core skills**: embedded harness skills `subagents.md`, `environment.md`, `shell.md`, `git.md`, `web.md` plus updated `file-access.md` / `harness.md` (harness-vs-shell boundary). `system_prompt.rs` skill/tool index expanded with subagent activation triggers.
- **i18n**: subagent UI states, role display names (Scout / Review / Security Analyst with auto-suffix on conflict), all new tool/environment/shell/git/web labels and the web-disabled hint added to every locale.
- **Agent chat maximize**: new maximize/minimize icon button in the agent chat header (before Reset). Toggles a compact voice hero (`agent-hero--compact`) inside the Agent right-panel tab only — smaller orb and status line, tagline hidden — so the chat log gains vertical space while Tasks, Context and compose stay visible. Session-local `chat_maximized` state (not persisted per workspace). i18n: `AgChatMaximize`, `AgChatRestore` in all locales.
- **Subagent live streaming**: subagent HTTP calls switched from non-streaming to SSE on both the OpenAI-compatible (OpenRouter / OpenAI / Azure-routed) and Anthropic paths. New `AgentEvent::Subagent{AssistantDelta,ThinkingDelta,ThinkingDone}` events stream the model's text + reasoning per agent. `SubagentCard` gained `live_text` / `live_thinking` / `thinking_done` fields (tolerant serde for older snapshots), the card renders a collapsible Thinking block, a live assistant-text pre-block, and a pulsing three-dot indicator while `status == "running"`. Buffers are cleared on `SubagentFinished` so the final `summary` takes over.
- **Subagent tool inventory in prompt**: subagent system prompt now lists the actually-provisioned tools grouped by purpose (Workspace, Diffs, Git, Memory, Plans, Tasks, Rules & Skills, Web) — generated from `registry_filtered` so the prompt cannot drift from the schema. Adds an explicit anti-hallucination clause directing the subagent to attempt `list_workspace_files` / `read_workspace_file` before claiming a lack of tools. Stops weaker models from instantly returning `blocked`.
- **Timeline tool-call compaction**: consecutive identical tool calls in the chat timeline collapse into a single row with a live `×N` counter badge that grows as new calls stream in. Expand chevron reveals each invocation's args + detail in a sub-list. Single-call rows keep the prior layout; merged rows aggregate status (any pending → pending, any fail → fail, else ok).
- **Tool group horizontal wrap**: `.agent-tool-group` switched from vertical stack to `flex-direction: row` with `flex-wrap: wrap`, so consecutive tool-call chips sit side-by-side and only wrap when the chat column runs out of width.
- **Per-turn usage stats footer**: new `AgentEvent::TurnUsage { input_tokens, output_tokens, ttft_ms, elapsed_ms }` emitted at the end of each turn from both the OpenAI-compatible (`stream_options.include_usage`) and Anthropic (`message_start.usage` + `message_delta.usage`) paths. TTFT is measured from request send to first streamed content/reasoning delta. The frontend accumulates per-workspace totals in a new `ChatUsageStats` field on `WorkspaceEntry` (serde-default so old snapshots load) and renders a compact mono footer under the chat log: `N turns · in {tokens} · out {tokens} · {tok/s} · ttft {ms|s}`. Footer is hidden until the first turn produces data; resets on Reset Conversation.
- **Copy + Redo buttons on assistant answers**: each assistant chat bubble grew an action row with a Copy button (writes the markdown to the system clipboard via `navigator.clipboard`, switches to a check icon for 1.4 s) and a Redo button (re-submits the previous user-turn prompt). `DisplayTimelineItem::Assistant` carries the preceding user text so Redo knows what to replay; the Redo button is hidden for the welcome bubble. Actions fade up from `opacity: 0.55` to full on hover for visual restraint.
- **Subagent prompt hardening**: subagent system prompt restructured into `# Tools` / `# Required execution flow` / `# Forbidden behaviors` sections with a mandatory first `list_workspace_files {"path":"."}` call when `workspace_read` is provisioned. Explicit ban on `status:"blocked"` without a prior workspace probe and on paraphrasing tool errors as "tools unavailable".
- **Subagent server-side blocked-without-trying guard**: new `validate_submit` rejects a `submit_result` with `status:"blocked"` when the role had `workspace_read` but the agent never called `list_workspace_files`, `read_workspace_file`, or `workspace_search`. The rejection is fed back into the conversation as a tool response so the loop continues, forcing the model to actually probe access before re-submitting. `handle_tool_call` now returns a `ToolCallOutcome` enum (`SubmitAccepted` / `SubmitRejected(msg)` / `NotSubmit`) wired into both the OpenAI and Anthropic subagent loops. 4 unit tests pin the contract.
- **`allowedToolGroups` schema constrained**: `subagents.run` JSON schema now enums `allowedToolGroups.items` to the 9 valid group strings (`environment_read`, `workspace_read`, `diff_read`, `git_read`, `memory_read`, `plans_read`, `tasks_read`, `rules_skills_read`, `web_read`). Previously the array was `{type: string}` with no constraint, so the coordinator could invent names like `"file_access"` / `"files"` and silently end up with an empty subagent toolset.
- **Strict toolgroup parser + role-defaults fallback**: new `parse_allowed_groups_strict(names) → (valid, unknown)` separates parseable groups from typos. `subagents::run` surfaces the unknowns via a `SubagentStep` with `status:"warning"` listing the bad names and the valid alternatives, and falls back to the role's defaults instead of spawning a subagent with only `submit_result`. 2 new unit tests cover the path.
- **Subagent tool-roster diagnostic step**: every subagent emits a `SubagentStep` immediately after `SubagentStarted` listing the actual provisioned tools (`Provisioned N tool(s): list_workspace_files, read_workspace_file, …`). When a model claims "no tools" the operator can compare against this list in one glance and tell hallucination from real misprovisioning.
- **Subagent tools list `×N` compaction**: consecutive same-named tool calls inside a subagent card collapse into one row with a `×N` mint-green badge — same merge logic the main timeline uses. `Search workspace ×5` instead of five identical rows.

### Changed

- **Docs**: README, developer setup, build guide, and `.env.release.example` now document the setup and cross-platform release automation paths.
- **Plans panel — Rules-style card UI**: the right-panel Plans tab now mirrors the Rules card flow with inline create/view/edit, header edit toggle, grouped state filters, task-state chips, and compact per-card status lines instead of the old split list/editor layout.
- **Dependencies**: upgraded `leptos` 0.7 → 0.8, `leptos_icons` 0.5 → 0.7, `icondata` 0.5 → 0.7. Leptos 0.8 is backward-compatible for the signal / effect / callback / event-listener APIs in use; only Lucide icon renames required code changes (`LuFileEdit → LuFilePenLine`, `LuPlusCircle → LuCirclePlus`, `LuSendHorizonal → LuSendHorizontal`, `LuAlertTriangle → LuTriangleAlert`, `LuTerminalSquare → LuSquareTerminal`, `LuPlayCircle / LuAlertCircle / LuCheckCircle / LuMinusCircle → LuCircle*` variants).
- **Command palette shortcut**: tmux-style chord moved from `Ctrl+B :` to `Ctrl+B p`. To free `p`, `ToggleRightPanel` (formerly `Ctrl+B p`) was reassigned to `Ctrl+B r`. Classic-mode shortcuts (`Ctrl+Shift+P` palette, `Ctrl+P` side panel) unchanged. Hint strings in `en_us.rs` / `de_de.rs` updated.
- **Subagent card polish**: dedicated CSS for the subagent timeline group — name and status now separated by a baseline-aligned `gap` (no more `scanblocked` run-together), font sizes shrunk (`0.7rem` name / `0.62rem` status / `0.68rem` summary / `0.64rem` tools), default `<details>` marker hidden in favour of the existing chevron pattern.
- **Tool-loop round budgets raised**: `MAX_ROUNDS` for both the OpenAI-compatible and Anthropic coordinator loops bumped from `12 → 36`; `MAX_SUBAGENT_ROUNDS` from `8 → 24`. The previous limits aborted long multi-step turns mid-investigation, especially in subagents that need several file-read rounds before they can synthesize findings.
- **Subagent skill doc — `allowedToolGroups` documented**: `harness_skills/subagents.md` now lists every valid `allowedToolGroups` string with its tool-coverage, recommends omitting the field entirely (defaults are sensible), and explicitly warns against invented names like `"file_access"` / `"files"` / `"shell_exec"`.

### Fixed

- **Plans panel polish**: fixed new-plan composer scroll anchoring so the composer is visible immediately after pressing `+`, centered wrapped plan-state filter rows, and added a visible header edit/preview toggle for plan cards.
- **Azure tool-name validation**: tool names containing `.` (`subagents.run`, `harness.create_workspace`, `harness.open_terminal`, `harness.list_terminals`, `harness.send_terminal_keys`, `harness.send_agent_context`, `harness.read_terminal_output`) failed Azure's `^[a-zA-Z0-9_-]+$` regex when OpenRouter routed to an Azure-hosted model (`invalid_request_error` on `input[N].name`). The OpenAI-compatible tool catalog now sanitizes `.` → `_` at render time via `sanitize_openai_tool_name`, and inbound `tool_calls` are mapped back to the internal dotted form via `openai_tool_name_to_internal` before dispatch. Assistant-message replay keeps the sanitized form, matching provider expectations. Anthropic path's existing `to_anthropic_name` / `from_anthropic_name` unchanged.
- **Project Explorer flicker on chat updates**: the file-tree on the left was wiping its `children_cache` and reloading every time the agent panel pushed a `TimelineItem` (chat delta, tool result, …). Root cause was a Memo that returned the full `WorkspaceEntry` — any field change, including the timeline, invalidated it. Memo narrowed to project `Option<(id, cwd, configuring)>`, so unrelated workspace state no longer triggers a reload. Mirrors the same fix previously applied to the Git History sidebar.
- **Subagents reporting "no file-system tools were provisioned"**: subagents would fail with `blocked` claiming their tool schema was empty even when they had `workspace_read` in their role defaults. Root cause was `parse_allowed_groups` silently dropping any string the coordinator invented in `subagents.run`'s `allowedToolGroups` argument — `["file_access", "files", "workspace"]` would all parse to `[]`, leaving the subagent with only `submit_result` in its schema. The model was reporting honestly; the catalog really was empty. Fix combines schema-level `enum` constraint, a strict parser that surfaces unknown names, fallback to role defaults when nothing parses, a startup `Provisioned N tool(s)` diagnostic, and the existing prompt-level "you DO have file-system access" guard. Backed by a regression test (`no_subagent_reachable_group_contains_dotted_tool_names`) that asserts no subagent-reachable group exposes dotted tool names, so the Azure `.` sanitization can stay coordinator-only.

### Removed


## [0.1.14] - 2026-05-21

### Added

- **Agent Image mode**: new image-generation toggle in the agent chat header (next to Reset). When active, your prompt produces an image instead of a chat reply. Generated images render inline with a Download button and, when a workspace is set, are saved to `<workspace>/.blxcode/generated/<unix-ms>-<slug>.<ext>`. Reference images attached to the chat are forwarded as img2img inputs (max 4 × 8 MiB, PNG/JPEG/GIF/WebP). The toggle is per-workspace and persisted across reloads.
- **Image providers**: OpenAI (`/v1/images/generations` for text-only, `/v1/images/edits` multipart when references are attached) and OpenRouter (`/v1/chat/completions` with `modalities: ["image", "text"]`). API keys reuse the existing agent provider keyring entries.
- **Settings → Image tab**: provider buttons (OpenAI / OpenRouter) and a model picker with refresh, filtered to image-shaped models (`dall-e`, `gpt-image`, `flux`, `stable-diffusion`, `sdxl`, `imagen`, anything matching `image`) with curated fallbacks per provider.
- **Shared `ModelPicker` component** under `src/workbench/model_picker/` (datalist input + refresh button + entry count). Voice settings tab refactored to consume it; image settings tab uses the same component.
- **Tauri commands**: `image_settings_get`, `image_settings_save`, `image_curated_models`, `generated_image_preview` (re-reads a saved image from disk for the timeline after a workspace reload — the snapshot stores `saved_path` only, never base64).
- **Voice + image integration**: when an image-mode turn is submitted from voice input (PTT/hotkey) and TTS is enabled, BLXCode plays a short locale-aware confirmation phrase ("Bild erstellt." / "Image created.") after the image arrives. The image content itself is not narrated.
- **Protocol**: `UserTurn.image_generate: bool` (default false) and `AgentEvent::ImageGenerated { prompt, mime, savedPath?, filename?, previewSrc }`. Mirrored in `src/agent_wire.rs`.
- **i18n**: `HsCatImage`, `ImagePaneTitle`, `ImagePaneDescription`, `ImageProviderField`, `ImageModelField`, `ImageSettingsSaved`, `ImageModeToggleAria`, `ImageModeHint`, `ImageModeBadge`, `ImageGenerateUserPromptPrefix`, `ImageGenerateDownload`, `ImageGenerateOpenFolder`, `ImageGenerateNoWorkspaceWarn`, `ImageGenerateMissingKey`, `ImageGenerateFailed`, `ImageGenerateConfirmTts` — added to all 14 locales.
- **Docs**: `docs/user/image.md` covering setup, generation flow, voice/image, limits, persistence and troubleshooting.
- **Tests**: 8 new unit tests (`image::settings::tests`, `image::generate::tests`) covering the image-model heuristic, default settings, slug sanitisation, MIME→extension mapping and data-URL decoding.
- **Dynamic memory categories**: any subdirectory under `.agents/memory/` is now a real category in the sidebar and graph (built-in `memory` / `learnings` keep their special handling). New Tauri commands `memory_list_categories` and `memory_create_category` (creates the folder and drops `.gitkeep` so empty categories persist). `MemoryNoteGroup.key` and `groups_open` switched from `&'static str` to `String`; grouping derives the category from the first API-path segment.
- **Memory panel — Discord-style toolbar**: the top inline "note title…" input is gone. The toolbar holds a `+ Kategorie` button (`LuFolderPlus`) that opens `NewCategoryDialog`, plus the existing collapse button. Each category header gets a hover-revealed `+` button (`workbench-memory-files__group-add`) that opens `NewNoteDialog` prefilled with the clicked category.
- **Dialogs**: `NewCategoryDialog` (name input → `memory_create_category`) and `NewNoteDialog` (title input → `memory_create` with `<category>/<note>.md` API path). Both reuse the `workspace-rename-dialog` styling and post errors inline.
- **Graph clustering by category**: backend `GraphNode` carries a `category` string derived from the API path; the 2D force layout adds a per-iteration centroid attraction that pulls same-category nodes together, and the 3D bundle (`graph3d_entry.mjs`) installs a `categoryClusterForce` d3-force with matching behavior. Node fill comes from the category's color setting in both renderers.
- **Per-category colors for any folder**: `MemoryCategorySettings::for_category` returns a deterministic `#rrggbb` (FNV-1a → HSL → hex) and uses the folder name as the label for user-created categories; built-in `memory` / `learnings` keep their existing colors. The sidebar accent stripe and graph node fill always match.
- **Sidebar Project Files resize**: added `SidebarResizer` (drag handle with pointer capture) so the Project Files panel defaults to 50% of the sidebar height and is user-resizable; persisted as `blxcode_sidebar_explorer_height_pct_v1` (clamped 15–85%).
- **System prompt — Workspace memory**: rewrote the section to document dynamic categories, the new `memory_list_categories` / `memory_create_category` tools, that `memory_rename` may cross categories within `.agents/memory/`, that `memory_category_update` accepts any existing category key, and that `memory_graph` nodes are clustered by category.
- **i18n**: `MemNewCategory`, `MemNewCategoryTitle`, `MemNewCategoryLabel`, `MemNewCategoryPh`, `MemNewNoteTitle`, `MemNewNoteLabel`, `MemNewNoteInGroup`, `MemCreate` — added to all 14 locales (English fallbacks for non-`de_de`).

### Changed

- **Documentation**: expanded user guides (Plans, Rules & Skills, Keyboard Shortcuts), updated Memory/Workspaces/Agent docs, developer Architecture/IPC with Mermaid diagrams, refreshed README screenshots and doc hub links.
- **Settings envelope round-trip fix**: `agent_settings_save` now preserves sibling envelope keys (`voice`, `image`) instead of clobbering them. `voice/settings.rs` was deduplicated to share `read_envelope`/`write_envelope` helpers with `agent_settings.rs`; this is the single source of truth for `agent_provider_settings.json`.
- **Orchestrator TTS refactor**: `maybe_emit_tts` split into `emit_tts_for_text(app, state, text)` (general) and `maybe_emit_tts_for_last_assistant` (chat path). The image branch reuses `emit_tts_for_text` to play the confirmation phrase without touching the chat conversation history.
- **Timeline persistence**: when a `GeneratedImage` row has a `saved_path`, its base64 preview is dropped before persisting to `sessions.json` (the image is rehydrated lazily via `generated_image_preview` on next render). Keeps the snapshot small.
- `expand_files_group_for_path`, `MemoryFileGroupHead` / `MemoryFileGroupSection`, `MemoryContextTarget::Category::key`, `MemoryCategoryEditDialog`, `add_category_agent_context`, and `MemoryContextMenuView` all moved from `&'static str` category keys to owned `String` for dynamic categories.
- `memory_note_groups` now derives groups from the active workspace's notes plus `state.empty_categories` (loaded via `memory_list_categories`), sorts with `memory` first, `learnings` second, then alphabetic.

### Fixed

### Removed


## [0.1.13] - 2026-05-20

### Added

### Changed

### Fixed

- Right panel: native hover tooltips (`title`) on icon rail tabs and header tab strip (inactive tabs show icon-only).
- Sidebar footer brand label is **BLXCode** (was lowercase `blxcode`).
- Plans panel: `plans-panel.css` is now loaded via Trunk (was edited but never linked); task-summary gap/colors use `!important` so they override stale rules in `styles.css`.
- Plans panel: plan list column is **drag-resizable** (default 288px, persisted `blxcode_plans_list_width_px_v1`); list rows are more compact (less padding, tighter chips).

### Removed

## [0.1.12] - 2026-05-20

### Added

- **Plan Manager**: durable Markdown plans under `<workspace>/.agents/plans/`, with `PLANS.md` as a protected index seeded on workspace bootstrap. Each plan can declare a canonical `## Tasks` (or `## Todos`) section using the syntax `- [ ] \`task-id\` - title` where the marker is one of `[ ] [>] [!] [x] [-]` (pending/in-progress/blocked/completed/cancelled).
- **Plans tab** in the right panel (between Browser and Memory): list with per-plan task summary, Markdown editor with debounced auto-save, preview toggle, create/rename/delete, and a "Load into BLXCode Agent" action that syncs plan tasks into the task manager and attaches the plan to shared context. On workspace activation, the panel auto-opens the last-active plan via the persisted `activePlanPath`.
- **Plan-linked tasks**: `TaskRecord` gains optional `planPath` / `planTaskId`; `TaskSnapshot` gains `activePlanPath`. `plan_load` replaces only tasks where `planPath == path` and leaves free tasks untouched. `task_update` on plan-linked tasks writes status changes back into the plan Markdown automatically. The Agent panel's task list groups plan-linked tasks by plan first, then renders a separate **Free Tasks** group.
- **Agent tools**: `plan_list`, `plan_read`, `plan_create`, `plan_write`, `plan_delete`, `plan_rename`, `plan_load`, `plan_sync_from_tasks` (server-side), and `plan_context_list`, `plan_context_attach`, `plan_context_detach` (client-side).
- **Tauri commands**: `plan_list`, `plan_read`, `plan_create`, `plan_write`, `plan_delete`, `plan_rename`, `plan_load`, `plan_sync_from_tasks` under `src-tauri/src/plans.rs`.
- **Shared context kinds**: `AgentContextKind::{PlanIndex, PlanFile, PlanTaskGroup}` (mirrored in `src/agent_wire.rs`). `render_context_prompt` renders attached plans separately from memory, citing per-plan task counts.
- **Terminal handoff**: `harness.send_agent_context.includeKinds` accepts `"memory" | "plans" | "tasks" | "images"` (default: all four). Renderer adds a new `## Attached plans / tasks` section with per-plan status counts, the active/in-progress task title, and a compact task list. The handoff renderer is primed by a per-workspace task-snapshot cache populated by the Plans/Agent panels on workspace activation so the synchronous renderer surfaces restored plan state immediately after a reload.
- **System prompt — mandatory Turn checklist**: explicit ordered steps at the top of the prompt that the agent must run every turn — (1) `rules_list` + `rules_read` on relevant active rules as a binding first step, (2) `skills_list` + `skills_read` on matching skills when the task warrants, (3) **Resume check** that recognises continuation directives in English (*continue, keep going, resume, proceed*) and German (*weiter, fortsetzen, weitermachen, mach weiter*) and resumes from `task_list` / `activePlanPath`, (4) memory/project context as needed, (5) execute. Trivial conversational turns may skip steps 1–2.
- **Workspace plans system-prompt section**: explains plan tooling, `## Tasks` line syntax, the `task_*` vs. `plan_*` separation, automatic status write-back, and that plan files and the task store survive workspace reload / harness restart / OS exit — so `plan_list` + `task_list` reconstruct in-flight work authoritatively after a restart.
- **i18n**: `TabPlans` plus Plans-panel keys (`PlansEmptyTitle`, `PlansEmptyLead`, `PlansNewPlan`, `PlansNewPlanPh`, `PlansRename`, `PlansDelete`, `PlansSelectPlan`, `PlansEdit`, `PlansPreview`, `PlansLoadIntoAgent`, `PlansLoaded`, `PlansRefresh`, `PlansTaskSummary`, `PlansTaskStatTotal`, `PlansTaskStatPending`, `PlansTaskStatInProgress`, `PlansTaskStatBlocked`, `PlansTaskStatCompleted`, `PlansTaskStatCancelled`, `PlansProtectedIndex`) added to all 14 locales; `TabPlans` and `de_de` task-stat labels are translated, other locales fall back to English for the stat keys.
- **Tests**: 11 new plan tests covering CRUD round-trip, `PLANS.md` protection, path-traversal sandboxing, parser status markers + `## Todos` alias, `plan_load` replace-only-plan-tasks semantics, `task_update` status write-back to plan Markdown, `plan_sync_from_tasks` round-trip preserving non-Task sections. New handoff-renderer tests for the plans/tasks section. New system-prompt tests verifying the mandatory Turn checklist, all continuation keywords, and the persistence guarantee.
- **Sidebar layout — bottom panel**: Project Files and Git graph share a single `.workbench-sidebar__panels` block (default **50%** of sidebar height, not the full area below Workspaces). A horizontal drag handle between the workspace list and this block resizes the combined panel; the existing handle inside the block still splits Explorer vs. graph (`blxcode_sidebar_panels_height_pct_v1` / `blxcode_sidebar_explorer_height_pct_v1` in `localStorage`).
- **Sidebar width**: drag handle on the right edge of the sidebar (like the right panel splitter) resizes width in pixels; default **260px** (was `clamp(216px, 22vw, 280px)`). Persisted as `blxcode_sidebar_width_px_v1` and on `WorkbenchSnapshot.sidebar_width_px`.
- **Project Explorer — hidden files**: toolbar eye toggle (`LuEye` / `LuEyeOff`) shows or hides dot-prefixed entries client-side; preference stored as `blxcode_sidebar_explorer_show_hidden_v1` (default off).
- **Project Explorer — tree navigation**: clicking a folder row (name/icon area) expands or collapses the tree; the chevron still works and does not double-toggle.
- i18n: `SbExplorerShowHidden`, `SbExplorerHideHidden`, `SbExplorerResizeAria`, `SbPanelsResizeAria`, `SbWidthSplitterAria` — added to all 14 locales (`de_de`, `fr_fr`, `es_es` translated for the new strings).

### Changed

- Sidebar Explorer/graph: `SidebarResizer` supports top- vs. bottom-measured splits (`measure_from_bottom` for the workspace↔panels boundary); container basis for the inner split is `.workbench-sidebar__panels` instead of `.workbench-sidebar__views`.
- Workspaces nav in the sidebar uses remaining space above the bottom panel (`flex: 1 1 auto`) instead of a fixed `max-height: 32%`.

- `ensure_agents_layout` now also creates `.agents/plans/` and seeds `PLANS.md` if missing; `WorkspaceRoots` gains a `plans` field.
- `RightPanelTab` adds a `Plans` variant; the right-panel rail and tabstrip render it before Memory.
- `harness.send_agent_context` tool description updated: default `includeKinds` is `["memory", "plans", "tasks", "images"]`; the rendered Markdown block includes attached plans and plan-linked task state.

### Fixed

- Sidebar Project Explorer no longer occupies almost the full sidebar when the tree is empty: Explorer and graph are confined to the resizable bottom panel so the graph stays visible and the file tree does not “stick” at the top of an oversized slot.
- **Project Files** and **Graph** section expand/collapse state now restores correctly after app restart or workbench hydrate (per workspace via `sidebar_explorer_open` / `sidebar_graph_open` on `WorkspaceEntry`; default open on first load).
- Project Explorer: nested folder clicks no longer bubble to parent rows (fixing “second level closes the tree”); row hover uses a visible highlight (`rgba` instead of undefined `--bg-hover`).
- Sidebar: collapsed rail is **44px** wide (`min-width: 200px` no longer applies); Explorer/Git panels **snap to the bottom** above the footer and shrink to header height when both sections are minimized (no empty 50% gap).
- Plans panel: plan list rows show hover highlight; list/editor divider (`border-left` on the editor column) spans the full panel height; `.workbench-right-plans` dock fills the tab body so layout height propagates correctly.
- Plans panel: collapsible plan list sidebar (same control as Memory — `LuPanelLeftClose` / `LuPanelLeftOpen`, narrow `3.8rem` rail with plan initials badges).
- **Git commit graph (sidebar)**: lane layout assigns separate swim-lanes for forked branches (not everything on lane 0); `branch_from_lane` and `pass_through_lanes` wire fields; SVG uses cubic Bézier connectors for fork/merge and full-height pass-through lines; merge commits render as hollow nodes. Section title renamed to **Git Commits** (`SbGraphTitle`).
- Plans panel: plan list rows show **colorized task-summary chips** (Leptos icons + counts per status — pending, in-progress, blocked, completed, cancelled) instead of the ASCII summary string (`0 · 0p / 0>!0b / …`); zero counts are muted. New-plan toolbar uses `LuPlus` instead of a literal `+`.

### Removed

- Project Explorer toolbar action **Collapse all folders** (`SbExplorerCollapseAll`); replaced by the hidden-files eye toggle.


## [0.1.11] - 2026-05-20

### Added

- **Sidebar Explorer & Git graph** (VS Code–style view sections): collapsible panels at the bottom of the left sidebar (above the version footer), with the workspace list scrolling independently above.
- **Project Explorer** section: lazy file tree for the active workspace `cwd` (directories and files via Tauri `list_path_entries`, sandboxed under the workspace root); refresh and collapse-all toolbar actions; expanded paths and open/collapsed state persisted per workspace.
- **Git graph** section: commit history with swim-lane SVG layout, ref badges, and author/time metadata from `git_commit_graph` (up to 100 commits, `git log --topo-order`); shown only when `.git` is detected (`git_is_repository`); if Git is not on `PATH`, the section stays visible with an i18n hint (`SbGraphGitMissing`) instead of an empty graph.
- Reusable Leptos component `SidebarViewSection` (`src/workbench/sidebar_view_section/`) with optional toolbar row (hover-reveal) and persisted `sidebar_explorer_open` / `sidebar_graph_open` on `WorkspaceEntry`.
- Tauri: `list_path_entries`, `git_is_repository`, `git_commit_graph`; backend modules `fs_entries` and `git_graph` (lane layout unit-tested).
- i18n: `SbExplorerTitle`, `SbGraphTitle`, `SbSectionExpand`, `SbSectionCollapse`, `SbExplorerRefresh`, `SbExplorerCollapseAll`, `SbGraphRefresh`, `SbExplorerNoCwd`, `SbExplorerTauriOnly`, `SbGraphLoadError`, `SbGraphGitMissing` — added to all 14 locales (German fully translated for sidebar explorer/graph strings).
- **Tmux-style keyboard shortcuts** (default): prefix `Ctrl+b`, then a second key — `o` quick open, `p` toggle right panel, `a` / `b` / `m` agent / browser / memory tabs, `n` new terminal slot (active workspace only), `:` command palette. Prefix times out after 1.5 s; `Esc` cancels an armed prefix.
- **Classic (legacy) shortcut mode**: restores direct chords (`Ctrl+O`, `Ctrl+P`, `Ctrl+Shift+A/B/M/P`, `` Ctrl+` `` new terminal, `Ctrl+Shift+P` palette). Selectable in **BLXCode Settings → App → Keyboard shortcuts**; persisted as `blxcode_shortcut_mode_v1` (`tmux` | `legacy`, default `tmux`).
- Frontend module `harness_chords` (`handle_harness_keydown`, `dispatch_shortcut_action`, `open_new_terminal`, `ShortcutKeys` display helpers) wired from `HarnessHost`; welcome-screen shortcut list reflects the active mode (`Ctrl+b` **then** key vs. combined keys).
- i18n: `WsKwThen`, `AppShortcutHeading`, `AppShortcutModeTmux`, `AppShortcutModeLegacy`, `AppShortcutModeHint` — added to all 14 locales (German fully translated for shortcut settings).
- Success **toasts** for terminal/memory handoff actions: a lightweight toast stack (bottom-right) confirms when context is sent to a terminal or attached to the BLXCode Agent; errors always show an error toast regardless of the success-toast toggle.
- Optional **success sound** on handoff (short Web Audio beep, same timbre as terminal-hook notifications) — independent of the toast toggle.
- **BLXCode Settings → App → Notifications**: checkboxes to enable/disable success toasts and success sounds (`blxcode_success_toast_v1` / `blxcode_success_sound_v1` in `localStorage`, default on).
- Frontend modules `toast` (`ToastService`, `ToastHost`) and `app_prefs` (`AppPrefsService`) wired in `WorkbenchShell`.
- i18n: `HandoffOkAttached`, `HandoffNoActiveWorkspace`, `AppNotifHeading`, `AppNotifToasts`, `AppNotifToastsHint`, `AppNotifSound`, `AppNotifSoundHint` — added to all 14 locales; `HandoffToAgentContext` shortened to **Send to BLXCode Agent** (German: *An BLXCode-Agent senden*).

### Changed

- `` Ctrl+` `` (legacy) and **Ctrl+b** `n` (tmux) now open a **new terminal slot** via `append_terminal_slot` and focus it — no longer only reveal the Agent tab.
- In tmux mode, `Ctrl+b` is not intercepted while a workspace terminal has focus (PTY/shell keeps the prefix).
- Handoff feedback is **centralized in `HandoffMenu`**: Graph preview no longer shows an inline green/red status strip under the titlebar; the terminal titlebar handoff button no longer flips to check/alert icons — both rely on global toasts (+ optional sound) instead.
- `note_context_item` and workspace Memory-category attach now set `added_at` from `Date::now()` (consistent with the Memory panel context menu).

## [0.1.10] - 2026-05-20

### Added

- Terminal agent context handoff: new BLXCode Agent client tool `harness.send_agent_context { slotId? | agentSlug?, instruction?, includeKinds?, submit? }` that hands off the active workspace's attached Memory/Learnings/Images to a live `claude` / `codex` / `gemini` / `opencode` / `cursor` CLI session in the workbench terminal grid. Renders a terminal-safe Markdown block (workspace root, attached items, image paths, optional instruction), writes it through the existing PTY path, and submits by default. Image bytes are exported to disk and cited by path — base64 is never written into the prompt.
- Image export pipeline for context handoff: new Tauri command `agent_export_context_images` writes selected images to `<workspace>/.blxcode/agent-context/images/` with sanitized filenames and a JSON manifest sidecar; collision-safe (`-2`, `-3` suffix) and idempotent across retries.
- Terminal PTY env vars `BLX_AGENT_CONTEXT_DIR` and `BLX_AGENT_CONTEXT_MANIFEST` so hooks and the spawned CLI agent can discover the workspace handoff directory and manifest path. Hooks remain advisory — the explicit toolcall is the only transport.
- Terminal titlebar **handoff dropdown** on every workspace terminal cell: opens a menu listing every live terminal in the workspace (so context can be forwarded to any peer slot), a separator, and a "Send to BLXCode agent context" entry that attaches the workspace's Memory category to the BLXCode Agent context (idempotent upsert). Button icon briefly flips to a check or alert and the border tints green / red as visual feedback (~2.8 s).
- Memory **Graph preview** handoff button: the note preview popover gains a terminal-share icon (next to "Open in Files" / "Close") that opens the same dropdown. Picking a terminal sends ONLY this note as a one-shot `memory_note` / `learning_note` context item; the "Send to BLXCode agent context" entry attaches the previewed note to the BLXCode Agent context.
- Shared frontend module `agent_context_handoff` containing the Markdown renderer (`render_agent_context_block`), the `WorkspaceTerminalTarget` listing, the async `perform_handoff` helper, and the reusable Leptos components `HandoffMenu` + `TerminalSlotHandoffButton`. The renderer is the single source of truth for the prompt shape and is fully unit-tested (empty / memory-only / image-only / mixed-with-instruction / long-path collapsing).
- `pty_sessions_signal()` accessor on `WorkbenchService` so UI components can react to live PTY session registration (button enables the instant the cell registers; no stale-disabled states).
- i18n keys for the new UI: `MemGraphSendToTerminal`, `HandoffSendContext`, `HandoffPickTerminal`, `HandoffNoTerminals`, `HandoffOkSent`, `HandoffFailed`, `HandoffToAgentContext` — German strings translated, all 12 other locales stubbed with the English fallback.
- Agent Panel image context: attach PNG, JPEG, WebP, or GIF images to the BLXCode Agent via OS/browser drag-and-drop or paste.
- Agent Panel drop-zone feedback: dragging images over the Agent chat highlights the pane with a dashed border and helper text; unsupported drops show a rejection hint.
- Agent Context image rows: attached images show Pending/Read status, remove controls, manual “use again” reactivation, and a preview dialog with close/remove actions.
- BLXCode Agent multimodal provider integration: pending images are sent once through OpenAI/OpenRouter and Anthropic vision payloads, then marked read via an `ImageContextConsumed` event.
- BLXCode Agent image context client tools: `image_context_list` and `image_context_detach`.
- Native image file validation command for dropped files, including MIME detection and per-image size limits.
- Right panel **Rules** tab: cards for every `.agents/rules/rule-*.md` with title, summary, enabled/disabled pill, toggle, read, and remove controls; refresh button auto-loads on tab activation and workspace switch.
- Right panel **Skills** tab: cards for every `.agents/skills/<name>/SKILL.md` with source badge (`git` / `npm` / `local` / `agent`), `SKILL.md missing` warn marker, toggle/remove controls, and an **Install skill** dialog.
- Skill install dialog: segmented `Git` / `npm` / `Local` source picker with name + per-source fields; submits through the new `skills_install` Tauri command, shows progress and per-attempt error inline.
- Tauri command surface for skills & rules: `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove`, `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`, `skills_remove`, `skills_install`, and `skills_rules_bootstrap`.
- On-disk manifests `.agents/rules/index.json` and `.agents/skills/index.json` tracking enabled state and (for skills) source provenance; atomic writes via tmp + rename, self-heal removes orphan entries at read time.
- First-touch bootstrap: when a workspace is opened, the harness auto-creates `.agents/{rules,skills}/` and seeds each `index.json` from the on-disk content (every existing rule and skill folder enters as `enabled: true`, skills with `source.kind = "local"`); manually disabled entries survive subsequent bootstraps.
- Skill install pipeline: `git clone --depth=1 --single-branch` for `git`, `npm pack` + `tar -xzf --strip-components=1` for `npm`, recursive copy for `local`; every install stages into `.install.<name>.tmp/`, validates that `SKILL.md` is present at the top level, and rolls the staging dir back on any failure.
- BLXCode Agent server tools mirroring the Tauri commands: `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove`, `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`, `skills_remove`, and `skills_install { name, source: { kind, url?, ref?, package?, version?, path? } }`.
- System-prompt section for workspace skills & rules: active rules are declared **binding and non-negotiable** and outrank skill guidance; disabled entries must be treated as if they did not exist; install/remove require explicit user requests; `index.json` is harness-managed, not hand-edited.
- i18n: new keys `TabRules`, `TabSkills`, `SrRulesEmpty`, `SrSkillsEmpty`, `SrEnable`, `SrDisable`, `SrInstallSkill`, install-dialog labels and placeholders, status pills (`enabled`/`disabled`), `SrMissingSkillMd`, and `SrNoWorkspace` — added to all 14 shipped locales.

### Changed

- BLXCode Agent system prompt now documents `harness.send_agent_context` next to `harness.send_terminal_keys`, instructs the agent to prefer the new tool for context-aware delegation, and warns against broadcasting context to multiple terminals without explicit user intent.
- `WorkspaceTerminalCell` accepts `slot_id` and `pane_id` props so the new handoff dropdown can be embedded directly in the titlebar (caller in `workspace_panel.rs` updated).
- Agent conversation history now sanitizes image content after a turn so base64 image bytes are not persisted or resent on later text/voice turns.
- Agent Chat reset moved from the compose action row into the Chat log header as an icon-only control with tooltip.
- Right panel rail and open-tabstrip now carry five tabs (Agent / Browser / Memory / Rules / Skills) with `LuShield` and `LuPuzzle` icons for the two new entries.
- `RightPanelTab` enum extended with `Rules` and `Skills` variants.

## [0.1.8] - 2026-05-20

### Added

- One-time dialog after the EULA asking whether to append `.blxcode/` to the workspace `.gitignore` (answer stored in `blxcode_gitignore_prompt_v1`; skipped in non-Tauri builds).
- Tauri command `workspace_ensure_agents` to create `.agents/memory/`, `.agents/learnings/`, and `_templates/` on workspace open.
- Tauri command `gitignore_append_blxcode` to add a `.blxcode/` entry when the user accepts the post-EULA prompt.
- Release CI workflow: `authorize` job — only the repository owner may run release builds (`workflow_dispatch` or `v*` tag push); org repos may set Actions variable `RELEASE_OWNER`.
- Release CI **manual run**: choose build targets — **Alle**, **Linux (deb, rpm, AppImage)**, **Mac Universal**, or **Windows** (`plan` job sets the matrix). Tag pushes still build all platforms.
- Memory **Files** sidebar: collapsible **Memory** / **Learnings** groups (chevron toggle, collapsed by default). Group headers open the category index note (`README.md` / `MEMORY.md` or `learnings/LEARNINGS.md`) and list other notes when expanded.
- Memory **Search** view: category filter badges (pill style) between the query field and results — **All**, **Memory**, and **Learnings** with hit counts; filters appear whenever there are hits (not only when both roots match); filters reset on a new query.
- Memory **Search** and note editor: **Show in graph** control jumps to the 3D graph, selects the note node, and flies the camera to it.
- Memory **Graph** view: lazy-loaded offline 3D graph powered by a local `3d-force-graph` / Three.js bundle, with the existing SVG graph retained as the 2D fallback.
- Memory **Graph** toolbar: icon-only controls for reset, zoom in/out, and 2D/3D mode switching; the selected graph mode is persisted locally.
- Memory **Graph** preview flow: clicking a node stays in the Graph tab, flies/focuses to the node, and opens a markdown preview popover with close, “Open in Files”, and in-preview wikilink navigation.
- Memory category context menu: right-click category headers to edit display name, category color, sidebar visibility, graph visibility, or send the category to the BLXCode Agent.
- Memory note context menu: right-click individual Memory/Learnings entries to open them or send that single note to the BLXCode Agent.
- BLXCode Settings → Memory: app-wide color preset management for Memory category colors, with add/edit/delete/reset controls.
- Agent Panel **Context** section: list of attached Memory/Learnings categories and notes, with per-item remove controls.
- BLXCode Agent **`list_tools`** server tool: returns JSON catalog of every registered tool (name, `server`/`client` site, description, parameters schema).
- BLXCode Agent **memory management** server tools: `memory_delete`, `memory_rename` (move/rename within one root, optional wikilink rewrite), `memory_graph`, and `memory_backlinks`.
- BLXCode Agent **memory UI** client tools: `memory_category_list`, `memory_category_update` (label, `#rrggbb` color, sidebar/graph visibility for `memory` / `learnings`), `memory_context_list`, `memory_context_attach`, and `memory_context_detach`.

### Changed

- Memory file list no longer repeats the `learnings/` folder label on every row; notes are grouped under their category instead.
- Memory graph labels are cleaned up for display (`tanstack-start-api-routes` → `Tanstack Start API Routes`) in 3D, 2D fallback, and preview titles.
- Memory graph node colors now respect workspace category settings and graph visibility toggles.
- Memory graph spacing and interaction feel more physical: 3D nodes spread farther apart, links curve subtly, and connected lines wobble when nodes or links are dragged/released.
- BLXCode Agent turns now include attached Memory context as compact path metadata before the user prompt, leaving file contents to existing workspace read tools.
- Workspace memory and learnings now live under `.agents/memory/` and `.agents/learnings/` (unified Memory API with `learnings/…` paths). Legacy `.blxcode/memory/` is migrated automatically when the new memory folder is empty. Existing learnings Markdown index links are upgraded to `[[wikilinks]]` for the memory graph.
- `.agents/` layout is bootstrapped when a workspace path is set (wizard commit, workspace switch, or workbench restore), not only when opening the Memory tab.
- Agent system prompt and memory tool descriptions reference `.agents/memory/` and `.agents/learnings/`, document full memory CRUD/graph/category/context tools, and recommend `list_tools` when schemas are unclear.
- Agent Panel **Tasks** and **Context** sections stay collapsed when empty and expand automatically when at least one task or context item is present (manual toggle still works until the count changes).
- Agent memory pointer blocks list both memory and learnings roots.
- Memory export/import uses `memory/` and `learnings/` subdirectories.
- Developer docs (`architecture`, `tauri-ipc`, `getting-started`, `memory-and-tasks`) updated for the `.agents/` memory layout, `workspace_ensure_agents`, and memory-flow diagrams.
- `scripts/render_i18n_locales_from_en.py` default mode translates only **missing** keys (not a full locale rewrite); use `--full` for the previous behavior, `--patch-english-matches` or `--keys` for English placeholder rows.

### Fixed

- Release CI `authorize` job reads optional `RELEASE_OWNER` via the GitHub API (`actions/github-script`) instead of `${{ vars.RELEASE_OWNER }}` in workflow expressions (avoids invalid context warnings when the variable is unset).
- i18n render script no longer rewrites locale files when nothing changed; prints guidance when zero rows are translated.

## [0.1.5] - 2026-05-19

### Added

- Session resume for hooked agent terminals (`sessions.json`) with captured titles on the terminal grid.
- Workspace and per-terminal completion notification badges with optional sound when background agents finish.
- Voice input and replies: microphone STT, OpenAI/OpenRouter transcription, and OpenAI TTS playback with a dedicated settings pane.
- Embedded browser navigation history and `browser_run_js` for running scripts in browser tabs.
- Workspace terminal improvements: multi-slot agent assignment, active/visibility controls, `pty_drain_wait`, terminal-wait-ready events, and agent launch retry.
- Workspace session UUIDs and notification pruning for agent tasks.
- User documentation: building guide, UI language guide, extra screenshots, and expanded README highlights.

### Changed

- First-run terms updated for MIT open-source distribution (MIT license acknowledgment, third-party API disclaimer); acceptance storage key bumped to `blxcode_eula_v2` so existing installs see the new text.
- Terminal layout and workspace grid observation for more responsive resizing.
- Workspace ID allocation and reset logic for stable session handling.

### Fixed

- Localization keys for unread workspace notifications.
- Potential deadlock when resetting workspace ID in terminal update closures.

## [0.1.0] - 2026-05-18

### Added

- Initial public release under the [MIT License](LICENSE).
- Native desktop shell (Tauri 2 + Leptos CSR) with multi-terminal workspaces, split panes, and persisted layout.
- Agent panel with OpenRouter, Anthropic, and OpenAI-compatible providers; sandboxed workspace file tools.
- **14-language UI** with compile-time translations and localized first-run terms.
- Workspace memory (`.blxcode/memory`), tasks (`.blxcode/tasks`), and memory graph view.
- Embedded browser with native child webviews on supported platforms and iframe fallback on Linux.
- Agent hooks for Claude, Codex, Gemini, OpenCode, and Cursor session/title capture.
- Command palette, Quick Open, drag-and-drop workspace reordering, and harness settings.
- EULA acceptance gate and platform app-config persistence.
