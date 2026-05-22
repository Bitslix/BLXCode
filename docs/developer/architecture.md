# Architecture

BLXCode is a local-first Tauri desktop app with a Leptos/WASM frontend and a Rust backend. The frontend owns UI state and rendering. The backend owns native capabilities such as PTYs, filesystem access, app config storage, keyring access, browser host integration, provider HTTP calls, memory, and tasks.

## High-Level Flow

```text
Leptos UI
  -> tauri_bridge.rs invoke wrappers
  -> Tauri commands registered in src-tauri/src/lib.rs
  -> Backend modules and managed state
  -> Serialized responses/events back to the UI
```

## Frontend Entry Points

- `src/main.rs`: mounts the Leptos app.
- `src/app.rs`: sets up i18n, **ThemeService**, EULA gating, and renders `WorkbenchShell`.
- `src/workbench/mod.rs`: workbench context, state hydration, auto-save, embedded browser event handling.
- `src/workbench/state.rs`: workspace state, snapshots, workspace creation draft, layout and browser state.
- `src/tauri_bridge.rs`: typed wrappers around Tauri `invoke()` calls.
- `src/agent_wire.rs`: frontend mirror of backend agent protocol types.

## Backend Entry Points

- `src-tauri/src/main.rs`: thin binary entry point.
- `src-tauri/src/lib.rs`: Tauri builder, managed state, plugin setup, command registration.
- `src-tauri/src/commands.rs`: general app commands, agent command shims, browser commands, directory picker helpers, PTY command wrappers, and git helpers.
- `src-tauri/src/workbench_state.rs`: persisted workbench snapshot/session storage.
- `src-tauri/src/pty_host.rs`: terminal session lifecycle and PTY IO.
- `src-tauri/src/browser_host.rs`: native or iframe browser embedding support.
- `src-tauri/src/voice/`: microphone recording, voice settings, STT, TTS, and voice catalog.

## Agent Subsystem

The agent subsystem lives under `src-tauri/src/agent/`. See [Agent Harness](agent-harness.md) for Better Harness; [Subagents](subagents.md) for coordinated parallel runs.

### Core runtime

- `state.rs`: shared event queue, busy/cancel flags, provider environment status, and conversation state.
- `protocol.rs`: `UserTurn` and `AgentEvent` types (including subagent events).
- `session_orchestrator.rs`: loads provider settings/key, `note_workspace_change`, dispatches the turn.
- `openrouter.rs` / `anthropic.rs`: streaming tool-call loops via `tool_dispatch.rs`.
- `tools.rs`: full tool registry and sandboxed `execute_server_tool`.
- `system_prompt.rs`: slim shared prompt (checklist + tool name index; docs in core skills).

### Harness extensions

- `harness_skills/*.md`: eleven embedded core skill documents.
- `tool_groups.rs` / `tool_dispatch.rs`: filtered catalogs for coordinator vs subagents.
- `environment.rs`, `shell_exec.rs`, `git_agent.rs`, `workspace_agent.rs`: server tools.
- `web_settings.rs`, `web_tools.rs`, `web_commands.rs`: Tavily/Brave keys and search.
- `subagents.rs`, `subagent_runner.rs`, `subagent_prompts.rs`: parallel subagent runs — [Subagents](subagents.md).

The frontend submits turns through `agent_submit_turn` and polls `agent_poll_events`. Subagent timeline updates are debounced 50 ms ([Subagents](subagents.md)). Tool results that need client execution are returned through `agent_submit_tool_result`.

Voice-originated turns set `voice_input=true`. After the provider turn finishes, the session orchestrator can synthesize the final assistant text and emit `AgentEvent::VoiceReady` for frontend playback.

When `UserTurn.image_generate` is true, the orchestrator takes an early exit: calls `src-tauri/src/image/generate.rs`, saves to `<workspace>/.blxcode/generated/`, emits `AgentEvent::ImageGenerated`, and skips the tool loop. Image settings live in the `image` envelope of `agent_provider_settings.json` (`src-tauri/src/image/settings.rs`).

```mermaid
sequenceDiagram
  participant UI as AgentPanel
  participant IPC as Tauri
  participant Orch as session_orchestrator
  participant API as Provider

  UI->>IPC: agent_submit_turn
  IPC->>Orch: dispatch_user_turn
  alt image_generate
    Orch->>API: image API
    Orch-->>UI: ImageGenerated + Done
  else chat turn
    Orch->>API: stream + tools
    loop poll
      UI->>IPC: agent_poll_events
      IPC-->>UI: AgentEvent stream
    end
  end
```

Client-only tools (context attach, plan context, image context list) execute in the frontend; results return via `agent_submit_tool_result`.

## Voice Subsystem

The voice subsystem lives under `src-tauri/src/voice/` with frontend support in `src/workbench/agent_panel/voice_orb/` and `src/workbench/harness_voice_pane/`.

It captures microphone audio with `cpal`, writes temporary mono WAV files with `hound`, sends STT requests to OpenAI or OpenRouter, and sends TTS requests to OpenAI. Voice settings are persisted as a `voice` sub-object inside `agent_provider_settings.json` and reuse the existing provider keyring entries.

See [Voice Architecture](voice.md) for the detailed flow.

## Workbench State

Workbench snapshots are serialized from frontend state and saved through backend commands. The snapshot version is defined by `WORKBENCH_SNAPSHOT_VERSION` in `src/workbench/state.rs`.

The state model includes workspaces, active workspace ID, recent workspaces, sidebar/right-panel layout, browser tabs, agent timeline, and terminal pane layout.

## Memory And Tasks

Memory lives in `src-tauri/src/memory.rs` and `src-tauri/src/agents_layout.rs`. Notes are stored under `<workspace>/.agents/memory/` and learnings under `<workspace>/.agents/learnings/` (API paths `learnings/…`). Legacy `.blxcode/memory/` is migrated on workspace bootstrap via `workspace_ensure_agents`.

```mermaid
flowchart LR
  subgraph frontend [Frontend]
    Panel[MemoryPanel]
    PlansPanel[PlansPanel]
    Bridge[tauri_bridge.rs]
  end
  subgraph backend [Backend]
    Ensure[workspace_ensure_agents]
    MemCmd[memory_* commands]
    PlanCmd[plan_* commands]
    Layout[agents_layout.rs]
    MemMod[memory.rs]
    PlansMod[plans.rs]
  end
  subgraph storage [Workspace]
    MemDir[".agents/memory"]
    LearnDir[".agents/learnings"]
    PlansDir[".agents/plans"]
    Legacy[".blxcode/memory"]
    TasksJson[".blxcode/tasks"]
  end
  Panel --> Bridge
  PlansPanel --> Bridge
  Bridge --> Ensure
  Bridge --> MemCmd
  Bridge --> PlanCmd
  Ensure --> Layout
  MemCmd --> MemMod
  PlanCmd --> PlansMod
  Layout --> MemDir
  Layout --> LearnDir
  Layout --> PlansDir
  Layout -.->|migrate if empty| Legacy
  MemMod --> MemDir
  MemMod --> LearnDir
  PlansMod --> PlansDir
  PlansMod --> TasksJson
```

Tasks live in `src-tauri/src/tasks.rs` and store JSON under `<workspace>/.blxcode/tasks/index.json`. `plan_load` replaces tasks matching a plan path; `tasks_update` can write status markers back into plan Markdown.

```mermaid
flowchart LR
  PlanMd["plan.md ## Tasks"]
  PlanLoad[plan_load]
  TasksMod[tasks.rs]
  TaskUpdate[tasks_update]
  PlanMd --> PlanLoad
  PlanLoad --> TasksMod
  TasksMod --> TaskUpdate
  TaskUpdate --> PlanMd
```

## Plans

`src-tauri/src/plans.rs` parses and writes the canonical `## Tasks` / `## Todos` section. `PLANS.md` is protected. Path traversal is rejected relative to the plans root.

## Skills And Rules

`src-tauri/src/skills_rules/` implements list/read/write, enable flags in `index.json`, and install staging (`git`, `npm`, local). **Core skills** are embedded via `CORE_SKILLS` in `store.rs` (`SkillSourceKind::Core`) and merged into `skills_list` on every workspace. `skills_rules_bootstrap` runs on workspace open via `workspace_ensure_agents` / layout helpers.

## Sidebar Explorer And Git Graph

- `src-tauri/src/fs_entries.rs` — `list_path_entries` (sandboxed directory listing), `read_workspace_text_file` (UTF-8 text preview, 512 KiB cap), and the file-preview trio:
  - `stat_workspace_file` → `FileMeta { name, relPath, byteLen, modifiedMs, kind, mime, policyKind? }` with `FileKind` (`Image` / `Video` / `Markdown` / `Mermaid` / `Code` / `Text` / `Binary`). `Code` covers source languages (Rust/TS/JS/Py/Go/HTML/CSS/JSON/YAML/shell/SQL/…); `Text` is reserved for plain text (txt/log/ini/conf/env/csv/…) that should still get gutter+selection but no syntax highlighting.
  - **Repository policy classification**: `classify_policy(stem)` runs **after** `classify_kind(ext)` and inspects the lowercased filename stem. When it matches a well-known stem (`license`/`licence`/`copying`/`copyright`/`unlicense`, `contributing`/`contribution(s)`, `contributors`/`contributer(s)`, `code_of_conduct` / `code-of-conduct` / `codeofconduct`, `security` / `security-policy` / `security_policy`, `authors` / `maintainers` / `owners` / `codeowners`, `changelog` / `changes` / `history` / `release_notes`, `readme`) it returns `Some(PolicyKind)` and `stat_workspace_file` **forces `kind = FileKind::Markdown`** (regardless of extension) and falls back to `text/markdown` for the MIME guess. Effect: a bare `LICENSE` (no extension) renders identically to `LICENSE.md`. The optional `policy_kind: Option<PolicyKind>` field on `FileMeta` is `#[serde(skip_serializing_if = "Option::is_none")]` so older snapshots and non-policy files stay unchanged on the wire.
  - `read_workspace_image_file` → base64 + MIME, **16 MiB** cap (`MAX_IMAGE_PREVIEW_BYTES`).
  - `read_workspace_video_file` → base64 + MIME, **64 MiB** cap (`MAX_VIDEO_PREVIEW_BYTES`).
  All four commands reuse the same `canonical_root` / `resolve_under_root` sandbox so traversal-out-of-root, missing files, and non-files behave identically.
- `src-tauri/src/git_graph.rs` — `git_is_repository`, `git_commit_graph` (lane layout, unit-tested).

Frontend:

- `src/workbench/sidebar_view_section/` — explorer and graph panels in the workbench sidebar.
- `src/workbench/file_preview/` — center-tab preview dispatcher:
  - `mod.rs` loads `FileMeta` once and routes to `ImageView` / `VideoView` / `MarkdownView` / `MermaidView` / `CodeView` (used for both `FileKind::Code` and `FileKind::Text`) / `UnsupportedView`.
  - `header.rs` renders the topbar (icon, name, path, size, mtime, Copy path, Refresh).
  - `code_view.rs` two-column layout: `<div class="code-view__row" data-line="N">` rows with a line-number gutter on the left and `inner_html` highlighted code on the right. Selection is stored as `RwSignal<Option<(usize, usize)>>` (1-based, inclusive range, always ordered low..=high). `mousedown` captures a `drag_anchor` and seeds `Some((N, N))`; `mousemove` extends the range while the anchor is set; a window-level `mouseup` listener (installed once, cleaned up via `on_cleanup`) ends drags even when the cursor leaves the gutter. A pure click without drag still toggles the single-line case. `on:contextmenu` opens the right-click menu (see `code_context_menu.rs`) at the click position; if the click lands outside the current range the selection is first replaced with that single line. Window-level `mousedown` and `keydown=Escape` listeners close the menu. The component also caches a `plain_lines: Arc<Vec<String>>` (raw `\n`-split source) parallel to the HTML lines so snippet builders never have to re-read the file. Plain-text files share the same layout but skip the highlight call. Lines are pre-rendered into a `Vec<View>` (no per-line `<For>` clone cost on large files). `.code-view` gets `user-select: none` so the drag never fights native text selection.
  - `code_context_menu.rs` renders the four-section right-click menu (`Snippet → Insert into terminal`, `Full context block → Insert into terminal`, `Snippet → Attach to agent`, `Clipboard`). Terminal sections list every workspace with at least one live PTY session — grouped by workspace, with the preview's own workspace pinned to the top and tagged with a localized **current** badge. The menu is purely view-layer: the parent owns `RwSignal<Option<CodeContextMenuState>>` and a `Callback<CodeMenuAction>` that runs the actual side effects (`pty_write`, `upsert_workspace_agent_context`, `navigator.clipboard.writeText`).
  - `hljs_glue.rs` lazy-loads the vendored highlight.js 11 common bundle `public/vendor/highlight/highlight.min.js`, polls `globalThis.hljs` for up to 5 s, and exposes `highlight(code, language)` over `hljs.highlight(code, { language, ignoreIllegals: true })`. Same lazy-script pattern as `mermaid_glue.rs`.
  - `markdown_view.rs` runs `pulldown-cmark` (tables, strikethrough, task lists, footnotes, smart-punctuation), detects ```` ```mermaid ```` fences and replaces them with `<pre class="mermaid">` sentinels that the post-mount effect hands to `mermaid.run({ nodes })`. Accepts an optional `policy_kind: Option<PolicyKind>` prop; when set, a `policy_hero(kind)` lookup table chooses the icon (`LuScale` / `LuGitPullRequest` / `LuUsers` / `LuShieldCheck` / `LuLock` / `LuUserRound` / `LuHistory` / `LuBookOpen`), the `FilePreviewPolicy{Kind}{Title,Subtitle}` i18n keys, and a CSS modifier (`license` / `contributing` / `security` / …). The component renders a `<header class="file-preview__policy-hero file-preview__policy-hero--<modifier>">` above the markdown body; per-modifier `--policy-accent` overrides in `styles.css` retint the left bar and icon (e.g. `Security` → `var(--danger)`, `License` → `var(--success)`) while staying fully theme-aware.
  - `mermaid_glue.rs` lazy-loads the vendored bundle `public/vendor/mermaid/mermaid.min.js`, calls `mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: 'dark' })`, and exposes `run_mermaid_on(&[HtmlElement])`.
  - `util.rs` ships `format_bytes`, `format_mtime` (`js_sys::Date.to_locale_string`), `icon_for_kind`, `hljs_lang_for_ext` (extension → highlight.js alias map), `html_escape`, `split_highlighted_into_lines` (UTF-8-safe HTML splitter that balances open `<span>`s across `\n`), `build_file_snippet_block(rel_path, language, plain_lines, range, source_workspace_for_header)` (fenced markdown emitter — clamps out-of-range indices, prefixes the header with the source workspace when crossing workspaces), allowlist-based `sanitize_svg` + `sanitize_markdown_html` (strips `<script>` / `<style>` / `<iframe>` / `<object>` / `<embed>` / `<foreignObject>` blocks, `on*=` event handlers, and `javascript:` / `vbscript:` URIs while preserving multi-byte UTF-8), plus a shared `FilePreviewError` enum (`NoTauri` / `WorkspaceNotFound` / `TooLarge(u64)` / `Failed(String)`) and `render_load_error(i18n, failed_label, error)` helper used by every renderer for consistent localized banners.

```mermaid
flowchart LR
  Click[Sidebar file click]
  Tab[CenterTabKind::FilePreview]
  Dock[FilePreviewDock]
  Stat[stat_workspace_file]
  Disp{FileKind}
  Img[read_workspace_image_file]
  Vid[read_workspace_video_file]
  Txt[read_workspace_text_file]
  Mer[Mermaid bundle]
  Hjs[highlight.js bundle]
  Click --> Tab --> Dock --> Stat
  Dock --> Disp
  Disp -->|Image| Img
  Disp -->|Video| Vid
  Disp -->|Markdown / Mermaid / Code / Text| Txt
  Disp -->|Markdown / Mermaid| Mer
  Disp -->|Code| Hjs
```

## Terminal Context Handoff

- Frontend: `src/workbench/agent_context_handoff.rs` — full-block path: `render_agent_context_block`, `HandoffMenu`, `perform_handoff` (single renderer for tool and UI). Lightweight path for file-preview snippets: `render_file_snippet_envelope` emits the same `⟪ BLXCode attached context ⟫` delimiters with just a Session header + File snippet section.
- Cross-workspace terminal enumeration: `list_terminal_targets_all_workspaces(&wb, Some(preferred_workspace_id))` iterates every workspace (filtering shell workspaces via `state::is_shell_workspace`), groups live PTY sessions per workspace, and moves the preferred (preview-owning) workspace to the front. `WorkspaceTerminalGroup` carries the workspace id + label + the per-workspace `WorkspaceTerminalTarget` list.
- `AgentContextItem` (mirrored in `src/agent_wire.rs` and `src-tauri/src/agent/protocol.rs`) has an optional `content: Option<String>` field and a `FileSnippet` kind. `file_snippet_context_item(rel_path, start, end, language, label, snippet, source_workspace)` is the canonical constructor used by the file preview's "Attach to agent" action.
- Backend prompt renderer `render_context_prompt` in `src-tauri/src/agent/session_orchestrator.rs` partitions `FileSnippet` items into a dedicated `Attached file snippets (verbatim, line-numbered headers):` section and embeds each item's inline `content` directly. `render_agent_context_block` mirrors this with a `## Attached file snippets` section (memory/plans filters skip snippet items).
- Backend: `agent_export_context_images` writes `<workspace>/.blxcode/agent-context/images/` plus manifest JSON (full-block path only — file-preview snippets ride entirely inline).
- PTY env: `BLX_AGENT_CONTEXT_DIR`, `BLX_AGENT_CONTEXT_MANIFEST`.

```mermaid
flowchart LR
  UI[HandoffMenu]
  CodeMenu[CodeContextMenu]
  Bridge[tauri_bridge]
  Export[agent_export_context_images]
  RenderFull[render_agent_context_block]
  RenderSnip[render_file_snippet_envelope]
  BuildSnip[build_file_snippet_block]
  AgentCtx[upsert_workspace_agent_context FileSnippet]
  Pty[pty_write]
  UI --> Bridge
  Bridge --> Export
  Bridge --> RenderFull
  Export --> RenderFull
  RenderFull --> Pty
  CodeMenu --> BuildSnip
  CodeMenu --> AgentCtx
  BuildSnip --> RenderSnip
  BuildSnip --> Pty
  RenderSnip --> Pty
```

Both memory and plan modules validate workspace paths and sandbox file operations to workspace-local directories.

## Browser Embedding

The browser host supports native child webviews on platforms where Tauri's unstable child-webview API works well. Linux currently uses iframe fallback. The frontend stores the detected embedding kind in `BrowserEmbedSurface`.

## Internationalization

The i18n service lives under `src/i18n/` and `src/service/`. Locale tables are Rust source files, while EULA source content is Markdown under `content/eula/`.

## Theming

Themes are frontend-only. `ThemeService` (`src/workbench/theme_service.rs`) sets `html[data-theme]` from `themes/tokens.css` and persists to `localStorage`. The Appearance settings pane reads the catalog from `src/theme/catalog.rs`. JavaScript subsystems (xterm, 3D memory graph) listen for `blxcode-theme-changed` and read computed CSS variables.

See [Themes](themes.md) and [Theme exceptions](../THEME_EXCEPTIONS.md).

## Boundaries To Preserve

- UI code should not perform native filesystem or keyring operations directly.
- Backend modules should not depend on Leptos signals or DOM concepts.
- `src-tauri/src/lib.rs` should register and wire modules, not accumulate feature implementation.
- Shared protocol types should be mirrored intentionally, as with `agent_wire.rs` and `agent/protocol.rs`.
