# Plans

Persistent plans for multi-step work on **blxcode**. Individual plans live as Markdown files in this directory.

## Index

| Status | Plan | Description |
|--------|------|-------------|
| done | [memory-nach-agents.md](memory-nach-agents.md) | Move workspace memory to `.agents/memory`, integrate `.agents/learnings` into Memory API, bootstrap `.agents/` on workspace open, auto-migrate legacy paths and wikilinks for graph |
| done | [agent-image-context.md](agent-image-context.md) | Attach images to BLXCode Agent context via global drag/drop and paste, show drop-zone feedback and preview dialog, send pending images once, then mark them read |
| done | [graph-3d-toolbar-preview.md](graph-3d-toolbar-preview.md) | Memory Graph: 3D-Orb (default), Icon-Toolbar (Zoom/2D-3D), Popover-Preview on node click, lazy local bundle, memory_panel.rs split |
| done | [skills-rules-tabs.md](skills-rules-tabs.md) | Neue RightPanel-Tabs für Skills & Rules: Card-Liste mit Toggle, Skill-Install-Dialog (git/npm/local), agent toolcalls (list/read/write/enable/remove/install), index.json pro Bereich, System-Prompt-Update |
| done | [terminal-agent-context-handoff.md](terminal-agent-context-handoff.md) | BLXCode Agent kann aktuellen Kontext gezielt per Toolcall an Terminal-Agent-Sessions uebergeben; Bilder werden als Metadaten plus lokale Pfade exportiert |
| done | [plan-manager.md](plan-manager.md) | Plan Manager fuer `.agents/plans`: Manage-Tab wie Memory Files, plan-linked Tasks gruppiert, Agent-Toolcalls, System-Prompt-Update und shared Context fuer BLXCode Agent plus Terminal-Handoff |
| planned | [kanban-board-view.md](kanban-board-view.md) | Kanban-View im Plans-Panel fuer alle Plan-Tasks eines Workspaces: Status-Spalten, Drag-and-Drop fuer Karten und Spalten, Spalten ein-/ausblenden und Markdown-Writeback |
| done | [per-turn-chat-metrics.md](per-turn-chat-metrics.md) | Per-Turn Metriken (in/out/ttft/tok/s/cost) statt globaler Footer; Tool- und Subagent-Turns; Session-Gesamtkosten im Chatlog-Titel; Persistenz via workbench.json |
| planned | [settings-tabs-themes-refactor.md](settings-tabs-themes-refactor.md) | Dynamische Workbench-Tabs (Main/Settings/File-Stub); Settings inline statt Modal; Appearance mit ~12 Themes (Default blxcode-dark); Sidebar bleibt sichtbar |
| done | [auto-update-github-releases.md](auto-update-github-releases.md) | Tauri Updater + signed latest.json, themed Leptos dialog/banner, Settings auto-check, hybrid release manifest flow, docs, and i18n implemented |
| done | [coordinated-subagents.md](coordinated-subagents.md) | Coordinated Subagents fuer BLXCode Agent mit Rollen, i18n Live-Subcards, Provider-Reuse, Environment Detection, Shell/Git/Web Toolsets und scoped Toolgruppen |
| done | [better-harness.md](better-harness.md) | BetterHarness: Shrink system prompt by extracting tool docs into 6 embedded Core Skills; Skills tab gets Core/User sub-tabs |
| done | [agent-chat-maximize.md](agent-chat-maximize.md) | Agent-Tab: Chat-Maximize-Toggle vor Reset; Voice-Hero kompakt (`agent-hero--compact`), mehr Platz fuer Chat-Verlauf |
| planned | [workspace-color-terminal-badge.md](workspace-color-terminal-badge.md) | Workspace-Farbe persistent in Sidebar (Dot, farbiges Unread-Badge), Terminal-Slot-Zahl vor Name, aenderbar via Kontextmenue-Dialog |
