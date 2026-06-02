# Workspace Presets & Agent Session Roles

## Summary

Two connected features for the **Create Workspace** flow and the Agent panel:

1. **Create-Workspace redesign + persistent presets.** Rework
   `create_workspace_wizard.rs` so its structure matches the reference layout
   (single, top-down step-1 screen: Connection → Name → Working folder +
   Recent → Terminal/Layout count → **Presets** row for one-click launch).
   Presets are reimplemented from scratch and **persisted in the
   application-data folder** (not the workspace). A preset captures terminal
   *count*, the *agents* assigned per slot, per-slot *names*, and the chosen
   *session role*, so the user can preselect a full fleet and launch it in one
   click.

2. **BLXCode Agent Harness Session Mode (role).** The user picks one
   *specialized role* from `src-tauri/src/agent/harness_skills/specialized/`
   via a dropdown `select` (Title + dimmed sub-line for description & tools).
   The selected role is loaded and handed to the BLXCode Agent: its content is
   appended to the **system prompt** (see Decisions for why system over
   assistant). The active role is shown in the Agent-tab name badge as a
   **colored sub-line** (color sourced from the role `.md` frontmatter). The
   role is **persisted in the workspace session** so it is restored on reload.

## Decisions

- **System prompt over assistant prompt for the role.** Inject the role as a
  dedicated trailing section in the single shared `system_prompt()` rather than
  as a synthetic assistant message. Rationale: (a) the system prompt is the
  highest authority tier in the prompt-authority ladder already documented in
  `system_prompt.rs`, so a role there cannot be overridden by untrusted
  content; (b) an assistant-role message is treated as *prior model output* —
  lower authority, and models follow it weakly and inconsistently; (c) it
  survives multi-turn cleanly without polluting `conversation_snapshot()`.
  The role body is embedded/trusted (shipped in the binary), but we still inject
  only the role's *operational* text under a clearly delimited
  `# Active session role` heading that explicitly states it ranks **below**
  Security and Agent-Chat-mode rules.
- **Specialized roles are embedded, read-only built-ins** (like `CORE_SKILLS`),
  not user `.agents/skills`. They live in `harness_skills/specialized/` and are
  surfaced through a new registry + Tauri commands, parallel to the existing
  skills store but separate from it.
- **Presets are global**, stored once per installation under
  `{app_data_dir}/workspace_presets.json` (atomic write, same tmp+rename
  pattern as `skills_rules/store.rs`). They are *not* committed with a
  workspace.
- A preset stores: `id`, `name`, `terminal_count`, `agent_counts[5]` (or
  resolved `slot_agent_labels`), optional `slot_names`, and optional
  `session_role` slug. `cwd`/connection stay user-chosen at launch time.
- The active role is a single optional slug on the workspace
  (`WorkspaceEntry.agent_session_role: Option<String>`), `#[serde(default)]`,
  so existing session snapshots deserialize unchanged and it auto-persists
  through the existing workbench snapshot mechanism.
- The role is passed to the backend per turn via a new optional
  `UserTurn.session_role: Option<String>` field (mirrors how `workspace_root`
  and `chat_mode` already travel), resolved to embedded role text inside the
  provider turn — no new long-lived backend session state.
- Every specialized `.md` must carry a `color:` frontmatter key. `coordinator`
  (violet) and `harness-optimizer` (teal) already have one; the others get a
  color added.

## Implementation Notes

### Backend — specialized role registry

- New module `src-tauri/src/agent/session_roles.rs`:
  - `const SPECIALIZED_ROLES: &[(&str, &str)]` embedding each
    `specialized/*.md` via `include_str!` (architect, coordinator, doc-updater,
    harness-optimizer, pr-test-analyzer, refactor-cleaner, security-reviewer).
  - Frontmatter parser reusing the existing pattern from
    `skills_rules/store.rs` (`name`, `description`, `tools`, `model`, `color`).
  - `RoleMeta { slug, title, description, tools: Vec<String>, color, model }`
    and `list_roles() -> Vec<RoleMeta>`, `role_body(slug) -> Option<&str>`,
    `role_meta(slug) -> Option<RoleMeta>` (strip the Prompt-Defense baseline
    duplication when composing the prompt block — Security already covers it).
- Tauri commands in `commands.rs` (registered in `lib.rs`):
  - `agent_session_roles_list() -> Vec<RoleMetaView>` (slug, title,
    description, tools, color).
  - (Optional) `agent_session_role_read(slug) -> String` if the UI ever needs
    the full body; not required for the dropdown.
- Frontend bridge wrappers in `tauri_bridge.rs` + a mirror type.

### Backend — inject role into system prompt

- Extend `system_prompt(workspace_root, agent_name, session_role: Option<&str>)`
  to append a `# Active session role` block built from the resolved role body
  when a slug is set and valid. Update both call sites (`openrouter.rs:211`,
  `anthropic.rs:116`) to pass `turn.session_role`.
- Add `session_role: Option<String>` to `UserTurn` (`protocol.rs`) with
  `#[serde(default)]`; thread it through `session_orchestrator.rs` into the
  provider turn. Mirror the field in `src/agent_wire.rs`.

### Backend — workspace presets store

- New module `src-tauri/src/workspace_presets.rs`:
  - `WorkspacePreset { id, name, terminal_count, agent_counts:[u8;5],
    slot_names: Vec<String>, session_role: Option<String> }`.
  - `path()` → `{app_data_dir}/workspace_presets.json`; `list()`, `save(upsert)`,
    `delete(id)` with atomic tmp+rename; validate counts (1..=16) and slug.
- Tauri commands: `workspace_presets_list`, `workspace_presets_save`,
  `workspace_presets_delete`, registered in `lib.rs`; bridge wrappers +
  mirror type in `tauri_bridge.rs`.

### Frontend — Create-Workspace redesign

- Rework `create_workspace_wizard.rs` step 0 to the reference top-down order
  and add two new sections at the bottom of the configurator:
  - **Session role** dropdown (`select`): one `<option>` per role with the
    title; below the select a dimmed `ws-config__hint` line rendering the
    selected role's description + `tools` (live from
    `agent_session_roles_list`). Bind to the draft
    (`CreateWorkspaceDraft.session_role`).
  - **Presets** row: chips for each saved preset ("one-click launch" →
    applies preset to the draft and commits/launches), plus a `+ New` chip that
    saves the *current* draft (count + agents + names + role) as a preset via
    `workspace_presets_save`. A small delete affordance per preset chip.
- Extend `CreateWorkspaceDraft` (`state.rs`) with `session_role: Option<String>`
  and (if not already derivable) per-slot name seeds; add
  `set_workspace_session_role`, `apply_preset`, `save_current_as_preset`
  helpers on `WorkbenchService`.
- `commit_inline_configure` writes the chosen `session_role` into the created
  `WorkspaceEntry`.

### Frontend — persist + propagate role

- Add `WorkspaceEntry.agent_session_role: Option<String>` (`#[serde(default)]`)
  — auto-persists via the existing workbench snapshot.
- When submitting a turn, populate `UserTurn.session_role` from the active
  workspace's `agent_session_role` (composer submit path).

### Frontend — Agent badge colored sub-line

- In `agent_panel/voice_orb/mod.rs`, below the existing `agent-name-badge`
  text, render a `agent-name-badge__role` sub-line when the active workspace
  has a role: title text, colored via the role's frontmatter `color` (resolved
  through `agent_session_roles_list`, cached in a signal keyed by slug). Add
  CSS in the agent panel stylesheet using theme tokens for layout and the
  role color for the accent.
- Resolve the active workspace's `agent_session_role` from `WorkbenchService`
  so the badge updates live when the role changes / workspace switches.

### Content — role frontmatter colors

- Add `color:` to the specialized `.md` files lacking it: architect,
  doc-updater, pr-test-analyzer, refactor-cleaner, security-reviewer (pick
  distinct theme-friendly hues; coordinator=violet, harness-optimizer=teal
  stay).

### i18n

- New `I18nKey`s for: session-role label/placeholder/none, presets heading,
  "New preset", save/delete preset, role badge aria. Add the string to **every**
  `src/i18n/locales/*.rs` (compile-time exhaustiveness — 15 locales). Seed
  non-English via `scripts/render_i18n_locales_from_en.py` then review.

### Docs

- Update `docs/user/workspaces.md` (presets + session role), and the
  agent-harness docs to describe the role injection and the new commands.

## Tests

- `session_roles`: frontmatter parse (name/description/tools/color/model),
  `list_roles` count == files, `role_body` strips frontmatter, unknown slug →
  `None`.
- `system_prompt`: with `Some(role)` contains `# Active session role` and the
  role title and explicitly states it ranks below Security/mode; with `None`
  unchanged (existing tests still pass after signature change).
- `workspace_presets`: save→list round-trip, upsert by id, delete, count
  clamp, atomic write leaves no tmp file, malformed JSON → default.
- Serde back-compat: an old `WorkspaceEntry`/`UserTurn` JSON without the new
  fields deserializes (defaults applied).
- Frontend `cargo check -p blxcode-ui --target wasm32-unknown-unknown` and
  backend `cargo test --workspace` green.
- Manual Tauri smoke: create workspace, pick a role, save a preset, relaunch
  via preset, reload app → role restored, badge shows colored role sub-line,
  agent system prompt reflects the role.

## Tasks

- [ ] `roles-registry` - Embed `specialized/*.md` as a role registry with frontmatter parsing
- [ ] `roles-colors` - Add missing `color:` frontmatter to specialized role files
- [ ] `roles-commands` - `agent_session_roles_list` command + bridge + mirror type
- [ ] `prompt-injection` - Extend `system_prompt` + `UserTurn.session_role`, wire both providers
- [ ] `presets-store` - `workspace_presets.rs` app-data store + CRUD commands + bridge
- [ ] `draft-state` - Add `session_role`/preset helpers to `CreateWorkspaceDraft` + `WorkbenchService`
- [ ] `entry-persist` - `WorkspaceEntry.agent_session_role` + submit-time propagation
- [ ] `wizard-ui` - Redesign step 0 layout + session-role dropdown + presets row
- [ ] `badge-subline` - Colored role sub-line in the agent name badge + CSS
- [ ] `i18n` - New keys across all 15 locales
- [ ] `docs` - Update user/developer docs
- [ ] `tests` - Backend unit tests + wasm/check + manual smoke
