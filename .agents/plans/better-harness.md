# BetterHarness: Core Harness Skills + Slim System Prompt

## Context

The current system prompt (`system_prompt.rs`) is ~470 lines and embeds full documentation for 50+ tools inline. This bloats every API request, consuming tokens and making prompt maintenance fragile. The goal is to extract tool documentation into **Core Skills** — built-in Markdown skill files shipped with the app — and reduce the system prompt to a minimal checklist + compact tool-name index. The Skills panel UI is extended to separate Core (built-in) from User (installed) skills.

---

## Overview of Changes

### 1. Core Harness Skill Files (new embedded assets)
Create `src-tauri/src/agent/harness_skills/` with 6 Markdown skill files (embedded via `include_str!`):

| File | Covers |
|---|---|
| `memory.md` | `memory_*`, `memory_context_*`, `image_context_*` |
| `plans.md` | `plan_*`, `plan_context_*` |
| `tasks.md` | `task_*` |
| `rules-skills.md` | `rules_*`, `skills_*` |
| `file-access.md` | `list_workspace_files`, `read_workspace_file`, `list_tools` |
| `harness.md` | `harness.*` client tools |

Each file contains usage guidance, parameter notes, and patterns — exactly as a user-authored `SKILL.md` would.

### 2. Backend: Add `Core` skill source + load harness skills

**`src-tauri/src/skills_rules/types.rs`**
- Add `Core` variant to `SkillSourceKind` enum

**`src-tauri/src/skills_rules/store.rs`**
- Add `CORE_SKILLS: &[(&str, &str)]` array — pairs of `(name, content)` using `include_str!`
- `list_skills()` prepends core skills into the result (enabled state from workspace index, default `true`)
- `read_skill()` serves core skill content from the embedded array
- Core skills cannot be removed (error on `remove_skill` for core names)
- `set_skill_enabled` works for both core and user skills (persisted in workspace index)

**`src-tauri/src/agent/system_prompt.rs`** — major reduction (~470 → ~240 lines):
- Remove all per-tool documentation sections
- Keep: workspace scope, security policy, turn checklist
- Replace tool catalog with compact grouped name list (~25 lines)
- Add: *"Call `skills_read` with a core skill name to get full usage guidance"*

**`src/skills_rules_wire.rs`**
- Add `Core` variant to `SkillSourceKind` (mirrors backend type)

### 3. Frontend: Skills Tab — Core / User tabs

**`src/workbench/skills_rules_panel/skills_tab.rs`**
- Add `SkillsView` signal: `Core | User` (default: `Core`)
- Render a two-button sub-tab strip below the header
- Filter displayed skills by view; "Install skill" button only visible in User view

**`src/workbench/skills_rules_panel/skill_card.rs`**
- Add `"core"` badge with `blx-sr-card__badge--core` CSS class
- Remove button suppressed for core skills (`is_core` flag)

**I18n strings added** (all 13 locale files):
- `SrSkillsTabCore` — "Core" / locale equivalents
- `SrSkillsTabUser` — "User" / locale equivalents
- `SrSourceCore` — "core"

---

## Critical Files

| File | Change |
|---|---|
| `src-tauri/src/skills_rules/types.rs` | Add `Core` to `SkillSourceKind` |
| `src-tauri/src/skills_rules/store.rs` | Embed + serve core skills, merge with index |
| `src-tauri/src/skills_rules/install.rs` | Add `Core` arm to install match |
| `src-tauri/src/agent/system_prompt.rs` | Reduce to ~240 lines; add core skill reference |
| `src-tauri/src/agent/harness_skills/*.md` | 6 new embedded skill files |
| `src/skills_rules_wire.rs` | Add `Core` to `SkillSourceKind` |
| `src/workbench/skills_rules_panel/skills_tab.rs` | Core/User sub-tab strip + filtering |
| `src/workbench/skills_rules_panel/skill_card.rs` | Core badge + no-remove guard |
| `src/workbench/skills_rules_panel/install_dialog.rs` | `Core` arm in source kind matches |
| `src/i18n/locales/*.rs` (all 13 locales) | 3 new I18n keys |

## Tasks

- [x] `create-skill-files` - Create 6 harness skill .md files in src-tauri/src/agent/harness_skills/
- [x] `extend-source-kind` - Add Core variant to SkillSourceKind (backend + frontend wire)
- [x] `update-store` - Embed + serve core skills, guard remove, enable/disable
- [x] `slim-system-prompt` - Replace inline tool docs with compact list + core skill reference
- [x] `i18n-keys` - Add SrSkillsTabCore, SrSkillsTabUser, SrSourceCore to all locales
- [x] `skill-card-ui` - Core badge + remove button suppression
- [x] `skills-tab-ui` - Core/User sub-tab strip with filtering
- [x] `fix-match-exhaustive` - Add Core arms to install.rs and install_dialog.rs
- [x] `update-tests` - Fix store tests to account for core skills always being present
