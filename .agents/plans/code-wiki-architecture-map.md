# Code-Wiki / Architecture-Map Integration

## Context

blxcode agents currently navigate a codebase by blind-scanning the filesystem (`list_workspace_files`, `workspace_search`, ad-hoc `rg`) on every turn. There is no durable, structured "map" of the repo's architecture, so each session re-derives layout from scratch and the team has no committed structural overview.

This feature adds an **architecture map** to the existing workspace memory system — *not* a separate wiki. A new reserved category `architecture/` holds a harness-generated structural skeleton (`architecture/modules/*.md`) plus a curated index `ARCHITECTURE.md` (Karpathy-style `index.md`). A new backend command `memory_rebuild_architecture` regenerates the static skeleton from the Cargo workspace structure (no LLM, no `syn`). Agents are nudged to consult the map first (system-prompt checklist + first-turn preload + scout ordering), and write-guards keep agents from clobbering harness-generated content.

The decision is deliberate: reuse the one memory API (graph, search, wikilinks, pointers all come for free) rather than build parallel infrastructure. Reviews scored this GO/4-5 with the integration points below as the known gaps to close.

**Scope: full MVP + all Phase 2.** Phase 2 = optional LLM-prose-ingest workspace setting (default off), `memory_lint_architecture`, UI rebuild button, and `architecture/flows/`.

---

## Key facts established from the codebase

- **Categories** are derived purely from the API path. `graph_category_for(api_path)` (`src-tauri/src/memory/paths.rs:79`) returns the first path segment as the category; root-level files (incl. `ARCHITECTURE.md`) fall into `"memory"`. Mirrored on the frontend in `graph_category_for_path` (`src/workbench/memory_graph/mod.rs:1089`) and `category_for_path` (`src/workbench/memory_panel.rs:33`).
- **Reserved categories** are gated by `is_reserved_category` (`paths.rs:134`) → used by `validate_category_name` (`paths.rs:116`) → used by `memory_create_category_impl` (`store.rs:567`). Today: `_templates`, `memory`, `learnings`.
- **Note writes go through** `note_abs` → `safe_join` (`store.rs:134`/`:144`): components must be `Normal`, must stay under root, must end `.md`. `memory_write_impl`/`memory_create_impl`/`memory_delete_impl` (`store.rs:422`/`:444`/`:463`) have **no content-level guards** today.
- **Frontmatter** (`src-tauri/src/memory/frontmatter.rs`) parses **only** `title`, `enabled`, `tags` — all other YAML keys are **silently dropped** on parse and lost on any serialize. `NoteMeta` (`src-tauri/src/memory/types.rs:10`) mirrors those three plus derived flags (`is_template`, `is_learning`, `is_overview`, `category`).
- **Workspace scaffold** is `ensure_agents_layout` (`src-tauri/src/agents_layout.rs:68`), exposed via `workspace_ensure_agents` (`memory/mod.rs:33`). Memory README seeding lives separately in `seed_memory` (`store.rs:284`) reached via `memory_bootstrap_impl` (`store.rs:324`).
- **Git** is read only via `current_branch` reading `.git/HEAD` directly (`src-tauri/src/git_info.rs`). No `git rev-parse HEAD` helper exists yet; `is_git_repository`/`find_git_dir`/`git_cli_available` do.
- **Splice-block markers** already exist: `splice_block`/`strip_block`/`block_installed` in `src-tauri/src/pointers/mod.rs:57-103` — reuse these for the `<!-- architecture:static:* -->` markers.
- **First-turn preload**: `project_docs::render_first_turn_block` (`src-tauri/src/agent/project_docs.rs:28`) reads `CLAUDE.md/AGENTS.md/GEMINI.md`, caps each at `MAX_FILE_BYTES = 16*1024` (`:22`), returns a `<project-docs>` block. Called from `dispatch_user_turn` only when `is_first_turn` (`src-tauri/src/agent/session_orchestrator.rs:60-65`) and prepended in `render_context_prompt`.
- **System prompt** (`src-tauri/src/agent/system_prompt.rs`) is shared by Anthropic + OpenRouter + OpenAI. Step 4 of the mandatory checklist is at `:56-61`; the "Memory vs Learnings" section at `:176-197`.
- **Scout subagent** (`src-tauri/src/agent/subagent_prompts.rs:169-188`): forced first call is `list_workspace_files {"path":"."}`; success criteria are passed in by the caller.
- **Built-in skills** are compiled in via `include_str!` from `src-tauri/src/agent/harness_skills/*.md`, registered in `CORE_SKILLS` (`src-tauri/src/skills_rules/store.rs:28`).
- **Tauri commands** register in `src-tauri/src/lib.rs` invoke_handler (memory block `:213-231`). Frontend bridge wrappers live in `src/tauri_bridge.rs` (memory block ~`:1447-1680`).
- **Context attach**: `handle_memory_context_attach` (`src/workbench/agent_panel/client_tools.rs:225`) attaches a whole category (all paths) or a single note.
- The new-note dialog and per-category "+" button live in `src/workbench/memory_panel.rs` (`NewNoteDialog` ~`:1369`, `MemoryFileGroupHead` "+" button ~`:1583-1594`, `new_note_category` signal ~`:804`).

---

## Implementation

Follow the order below (matches dependency flow).

### 1. Reserve `architecture/` + `.meta/`, ignore state file

`src-tauri/src/memory/paths.rs`
- Add `pub const ARCHITECTURE_CATEGORY: &str = "architecture";` and `pub const META_DIRNAME: &str = ".meta";`.
- Extend `is_reserved_category` (`:134`) to also match `"architecture"` so `memory_create_category("architecture")` is blocked. (`.meta` is already excluded from category listing by the `starts_with('.')` check at `:107`, and from graph/note walks because `walk_md` recurses everything — see step 3 for the `.meta` exclusion.)
- `graph_category_for` (`:79`): special-case so `ARCHITECTURE.md` (root index) maps to category `"architecture"` instead of `"memory"`. Keep `architecture/...` paths mapping to `"architecture"` (already do, since first segment is `architecture`).

Root `.gitignore`
- Add `.agents/memory/.meta/` (state file is local/CI only, never committed).

### 2. New `git rev-parse HEAD` helper

`src-tauri/src/git_info.rs`
- Add `pub fn head_commit(start: &Path) -> Option<String>` — prefer `git rev-parse HEAD` via the CLI when `git_cli_available()`, else read `.git/HEAD`/packed-refs, else `None`. Used by the rebuild state to detect staleness without a full re-walk.

### 3. Static architecture indexer + state

New module `src-tauri/src/memory/architecture/` (`mod.rs`, `static_index.rs`, `state.rs`), wired into `src-tauri/src/memory/mod.rs` (`mod architecture;`).

- **Walk** (`static_index.rs`): enumerate Rust sources via `git ls-files` when `is_git_repository`, else recursive walk skipping `target`, `node_modules`, `dist`, `.git`. **No `rg`/`syn` dependency.**
- **Crate detection**: parse root `Cargo.toml` for the root package (`blxcode-ui`, `src/`) **and** `[workspace].members` (`src-tauri`). Do not rely on members alone — the root package is not listed there.
- **Module tree**: per crate, collect `mod`/`pub mod` to **max depth 2**; aggregate deeper UI paths (`workbench/agent_panel/…`) under their top-level module (`workbench`). Emit one `architecture/modules/<crate>.md` per crate.
- **Markers**: wrap generated content in `<!-- architecture:static:begin -->` … `<!-- architecture:static:end -->` using `pointers::splice_block` so manual notes above/below survive. `ARCHITECTURE.md` gets a `## Generated` section (spliced) + a `## Manual` section (left untouched).
- **State** (`state.rs`): `.agents/memory/.meta/architecture-state.json` storing `{ git_rev, generated_at, crate_count, module_count }`. Rebuild only rewrites files whose content changed (idempotent).
- **Scope**: workspace `.agents/memory/` only — never `~/.blxcode/memory/`.
- **Frontmatter**: each generated `modules/*.md` and `ARCHITECTURE.md` carries `managed: static`, `git_rev: <sha>`, `source_paths: [...]`, and `stale: false` (see step 5).
- Public entrypoint: `pub fn rebuild_architecture_impl(workspace_cwd: &str) -> Result<RebuildReport, String>`.

### 4. Tauri command `memory_rebuild_architecture`

- `src-tauri/src/memory/mod.rs`: add `#[tauri::command] pub fn memory_rebuild_architecture(workspace_cwd: String) -> Result<RebuildReport, String>` delegating to `architecture::rebuild_architecture_impl`. Add `RebuildReport` to `types.rs`.
- `src-tauri/src/lib.rs`: register `memory::memory_rebuild_architecture` in the invoke_handler memory block (`~:213`).
- **Trigger hook**: in `ensure_agents_layout` (or a thin wrapper called from `workspace_ensure_agents`), seed `ARCHITECTURE.md` + run rebuild **only when** the architecture files are missing **or** `state.git_rev != head_commit()` — guard for performance, since `workspace_ensure_agents` runs on workspace open.

### 5. Frontmatter extension + write guards

`src-tauri/src/memory/frontmatter.rs`
- Add fields to `MemoryFrontmatter`: `managed: Option<String>`, `stale: Option<bool>`, `git_rev: Option<String>`, `source_paths: Option<Vec<String>>`. Extend the parse `match` and `serialize_frontmatter` so they round-trip (today they silently drop unknown keys, which would erase `managed:` on any agent write — must fix).

`src-tauri/src/memory/store.rs` — add guard checks at the top of `memory_write_impl`, `memory_create_impl`, `memory_delete_impl`:
- Block `memory_create`/`memory_write`/`memory_delete` for any path under `architecture/` (harness-only territory).
- For `ARCHITECTURE.md`: allow writes that touch only the `## Manual` section; reject writes that would alter the spliced `## Generated` block (compare the region between markers). Simplest robust rule for MVP: reject any write to `ARCHITECTURE.md` whose `## Generated` marker region differs from the on-disk one.
- Surface clear errors ("architecture/ is harness-managed; run memory_rebuild_architecture").

`src-tauri/src/memory/types.rs`: add `managed`, `stale` to `NoteMeta` (populated in `meta_from_file`, `store.rs:182`) so the UI can detect managed notes.

### 6. Shared first-turn preload budget (12 KiB total)

`src-tauri/src/agent/project_docs.rs` — refactor from per-file 16 KiB to a **shared ~12 KiB total budget** across project-docs + architecture:
- Replace `MAX_FILE_BYTES` usage with a running total budget (`const PRELOAD_BUDGET_BYTES: usize = 12 * 1024;`).
- Priority order: `CLAUDE.md` → `AGENTS.md` → `GEMINI.md` → then the `## Generated` section of `ARCHITECTURE.md` (read via the architecture module) with the **remaining** budget; truncate at a char boundary with the existing marker style.
- Emit ARCHITECTURE content inside a `<memory-architecture>` block appended after `</project-docs>` (or as an extra section). Keep the function returning a single `Option<String>` so `session_orchestrator.rs:61-65` needs no shape change (it already only calls when `is_first_turn`).
- Update the existing unit tests in `project_docs.rs` (the 16 KiB/truncation test) to the new budget semantics.

### 7. System prompt + skill + scout

`src-tauri/src/agent/system_prompt.rs`
- Step 4 (`:56-61`): prepend a **navigation sub-checklist** for navigation / "where is" / refactor / repo-explore intents: (1) `memory_read ARCHITECTURE.md` or `memory_search` scoped to `architecture/`; (2) `memory_read` 1–3 `architecture/modules/*`; (3) *then* `workspace_search`/`list_workspace_files`.
- "Memory vs Learnings" section (`:176-197`): add a short paragraph describing `architecture/` as the harness-maintained structural map (read-first, never hand-edit; regenerate with `memory_rebuild_architecture`).

`src-tauri/src/agent/harness_skills/memory-architecture.md` (new) + register in `CORE_SKILLS` (`skills_rules/store.rs:28`). Content: what the map is, when to read it, that it's harness-generated, how to rebuild, the read-first navigation flow.

`src-tauri/src/agent/subagent_prompts.rs` — Scout flow (`:169-188`): when workspace-read is available, instruct the scout to read `ARCHITECTURE.md` (and 1–3 module notes) **before** falling back to broad `list_workspace_files` enumeration, and add a success criterion: result must cite the index + 1–3 module paths. Keep `list_workspace_files` as the mandatory first call (don't regress the existing non-negotiable), but add the architecture read as step 2.

### 8. UI — graph pin, block new-note, context attach scope, rebuild button

`src/workbench/memory_graph/mod.rs` — `graph_category_for_path` (`:1089`): special-case `ARCHITECTURE.md` → `"architecture"` (mirror the backend change in step 1).

`src/workbench/memory_panel.rs`
- `category_for_path` (`:33`): mirror the `ARCHITECTURE.md` → `"architecture"` mapping.
- `MemoryFileGroupHead` (`~:1583-1594`): hide/disable the "+" (new note) button when the category is `"architecture"`. Also guard `NewNoteDialog` submit path so an `architecture/` target is rejected client-side (defense in depth; backend already blocks).
- Use the new `NoteMeta.managed`/`stale` flags to render managed/stale badges on architecture notes (read-only affordance).

`src/workbench/agent_panel/client_tools.rs` — `handle_memory_context_attach` (`:225`): no behavior change required, but ensure attaching the `architecture` category resolves correctly through the existing prefix filter (architecture paths are non-learnings, so they already group under the `else` branch). Add `architecture` as an explicitly attachable scope label.

`src/tauri_bridge.rs` — add wrapper `pub async fn memory_rebuild_architecture(ws: &str) -> Result<RebuildReport, String>` following the existing memory-command pattern (~`:1615`). Add a mirrored `RebuildReport` type alongside the other memory wire types (`~:1307`).

**Rebuild button (Phase 2)**: in `memory_panel.rs`, add a "Rebuild architecture map" action in the workspace memory toolbar that calls the bridge wrapper, shows a spinner, then re-fetches `memory_list`/`memory_graph`. Surface the `RebuildReport` (crate/module counts) as a toast.

### 9. Phase 2 — prose ingest setting, lint, flows/

- **`architectureLlmProse` workspace setting** (default **off**): add to the workspace settings struct read in `session_orchestrator.rs` (where settings + API key load). When on, after a rebuild/stale detection, optionally enqueue an internal LLM job to synthesize prose into the `## Manual`/module prose regions, with a cost note in the UI. Default-off means **no** automatic background LLM in normal operation — prose otherwise only happens via an explicit agent turn.
- **`memory_lint_architecture`** command: compare `state.git_rev` vs `head_commit()` and flag `architecture/` notes as `stale: true` (frontmatter) when HEAD moved without a rebuild; report a list of stale paths. Register in `lib.rs` + bridge. UI shows stale badges (step 8).
- **`architecture/flows/`**: reserve the subdirectory and have the indexer accept hand-authored flow notes there (harness leaves them untouched; they participate in graph/search like normal notes). MVP indexer only emits `modules/`; `flows/` is curated.

### 10. Tests

- `architecture/static_index.rs` unit tests against a **fixture mini-repo** (temp dir with a root `Cargo.toml` + workspace member, a few `mod` declarations): asserts crate detection (root pkg + member), depth-2 aggregation, and idempotent rebuild (second run = no content change).
- Marker-merge test: manual text above/below `<!-- architecture:static:* -->` survives a rebuild (reuse `pointers` test style).
- Guard tests in `store.rs`: `memory_create`/`memory_write`/`memory_delete` under `architecture/` are rejected; `ARCHITECTURE.md` Manual-only edit allowed, Generated-region edit rejected; `memory_create_category("architecture")` rejected.
- `frontmatter.rs` round-trip test: `managed`/`stale`/`git_rev`/`source_paths` survive parse→serialize.
- Update `project_docs.rs` preload tests for the shared-budget semantics.

### 11. Docs / Git policy

- Developer doc (e.g. `docs/` or a section in repo `CLAUDE.md`): explain the architecture map, the rebuild command, and the **Git policy**: commit `ARCHITECTURE.md` and `architecture/modules/*.md` (structure travels with the repo; PR diffs show harness updates); gitignore `.meta/architecture-state.json`. After large refactors: run `memory_rebuild_architecture` and commit the regenerated files.

---

## Files touched (summary)

| Area | Files |
|---|---|
| Reserve + paths | `src-tauri/src/memory/paths.rs`, root `.gitignore` |
| Git rev | `src-tauri/src/git_info.rs` |
| Indexer | new `src-tauri/src/memory/architecture/{mod,static_index,state}.rs`, `memory/mod.rs`, `memory/types.rs` |
| Command | `memory/mod.rs`, `src-tauri/src/lib.rs` |
| Frontmatter + guards | `memory/frontmatter.rs`, `memory/store.rs`, `memory/types.rs` |
| Preload | `src-tauri/src/agent/project_docs.rs` (+ `session_orchestrator.rs` if shape changes) |
| Prompts/skill | `agent/system_prompt.rs`, `agent/subagent_prompts.rs`, new `agent/harness_skills/memory-architecture.md`, `skills_rules/store.rs` |
| UI | `src/workbench/memory_graph/mod.rs`, `src/workbench/memory_panel.rs`, `src/workbench/agent_panel/client_tools.rs`, `src/tauri_bridge.rs` |
| Phase 2 | workspace settings struct (orchestrator), `memory_lint_architecture` (`memory/mod.rs`+`lib.rs`+bridge), `architecture/flows/` reservation |
| Tests/docs | per-module `#[cfg(test)]`, developer doc |

---

## Verification

1. **Backend builds + tests**: `cargo check -p blxcode` and `cargo test --workspace` (covers indexer fixture, marker merge, guards, frontmatter round-trip, preload).
2. **Frontend builds**: `cargo check -p blxcode-ui --target wasm32-unknown-unknown`.
3. **End-to-end** (`cargo tauri dev`): open a workspace → confirm `.agents/memory/ARCHITECTURE.md` + `architecture/modules/{blxcode-ui,blxcode}.md` are generated and `.meta/architecture-state.json` exists (and is gitignored). Confirm the memory panel shows an `architecture` group with the "+" new-note button hidden, and the graph pins `ARCHITECTURE.md` under the `architecture` category.
4. **Rebuild idempotency**: run `memory_rebuild_architecture` (via the UI button) twice with no code change → second run reports no file changes; manually edit text outside the static markers → rebuild preserves it.
5. **Guards**: via the agent, attempt `memory_create` under `architecture/` and a write to the `## Generated` block of `ARCHITECTURE.md` → both rejected with clear errors; a Manual-section edit succeeds.
6. **Preload**: start a fresh conversation in a repo with a large `CLAUDE.md` → confirm the first user message carries both `<project-docs>` and `<memory-architecture>` within the shared ~12 KiB budget (no overrun).
7. **Phase 2**: toggle `architectureLlmProse` off (default) → no background LLM job fires after rebuild; run `memory_lint_architecture` after moving HEAD without a rebuild → stale paths reported and badged in the UI.
