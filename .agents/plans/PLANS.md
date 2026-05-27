# Plans

Persistent plans for multi-step work on **blxcode**. Individual plans live as Markdown files in this directory.

## Index

| Status | Plan | Description |
|--------|------|-------------|
| planned | [linux-browser-iframe-boot-fix.md](linux-browser-iframe-boot-fix.md) | Linux Boot-Crash Fix: sticky Lazy Mount für BrowserTabDock + iframe-src-Gating (WebKitGTK nested iframe) |
| planned | [performance-optimization.md](performance-optimization.md) | Performance-Audit: Agent-Streaming hot path, Auto-Save-Kaskade, Backend-Blocking, Boot/CDN, Terminal-Refit; Phasen P0–P3 |
| planned | [security-hardening.md](security-hardening.md) | Security-Audit: Subagent shell_write-Bypass, Shell-Allowlist, XSS/CSP, Runtime-Tool-Allowlist, Symlink/URL/PTY-Hardening; Phasen P0–P3 |
| planned | [v2-roadmap.md](v2-roadmap.md) | V2-Roadmap (Trust Repair): plan_context-Bug, Konversations-Persistenz, Kanban-MVP, web_fetch, Docs-Sync, CI; Phasen V2.0–V2.3+ |
| planned | [kanban-board-view.md](kanban-board-view.md) | Kanban-View im Plans-Panel: Status-Spalten, DnD fuer Karten/Spalten, Spalten ein-/ausblenden, Markdown-Writeback (Full Scope; MVP in v2-roadmap) |
| done | [agent-chat-maximize.md](agent-chat-maximize.md) | Agent-Tab: Chat-Maximize-Toggle vor Reset; Voice-Hero kompakt, mehr Platz fuer Chat-Verlauf |
| done | [agent-image-context.md](agent-image-context.md) | Bilder per Drag/Drop und Paste an Agent-Kontext; Drop-Zone, Preview-Dialog, einmaliges Senden, dann read |
| done | [agent-image-modus.md](agent-image-modus.md) | Agent Image-Modus: Chat-Toggle, Settings-Tab Image, Referenzbilder, Generierung via OpenAI/OpenRouter, Workspace-Speicherung und Download |
| done | [api-keys-centralized.md](api-keys-centralized.md) | API-Schluessel zentral unter Settings → API Keys; Backend-Katalog + resolve; Agent-Tab ohne Key-Felder |
| done | [auto-update-github-releases.md](auto-update-github-releases.md) | Tauri Updater + signed latest.json, themed Update-Dialog/Banner, Settings auto-check, Release-Manifest, i18n |
| done | [better-harness.md](better-harness.md) | System-Prompt schlanker via 6 embedded Core Skills; Skills-Tab mit Core/User-Untertabs |
| done | [coordinated-subagents.md](coordinated-subagents.md) | Coordinated Subagents: Rollen, i18n Subcards, Provider-Reuse, Environment/Shell/Git/Web-Tools, scoped Toolgruppen, Inline-Timeline |
| done | [file-browser-rich-preview.md](file-browser-rich-preview.md) | File-Browser-Preview: Bilder (SVG/Raster), Video, gerendertes Markdown, Mermaid; neue Topbar mit Datei-Metadaten und sandboxed Backend-Commands |
| done | [graph-3d-toolbar-preview.md](graph-3d-toolbar-preview.md) | Memory Graph: 3D-Orb, Icon-Toolbar, Popover-Preview, lazy Bundle, memory_panel split |
| done | [leptos-0.8-upgrade.md](leptos-0.8-upgrade.md) | Leptos 0.7→0.8 und leptos_icons/icondata 0.8; Icon-Prop auf `Signal<Icon>`, Code-Anpassungen abgeschlossen |
| done | [memory-nach-agents.md](memory-nach-agents.md) | Memory nach `.agents/memory`, Learnings-API, `.agents/`-Bootstrap, Legacy-Migration, Wikilinks fuer Graph |
| done | [per-turn-chat-metrics.md](per-turn-chat-metrics.md) | Per-Turn Metriken (in/out/ttft/tok/s/cost) und Session-Gesamtkosten im Chatlog-Titel; Persistenz via workbench.json |
| done | [plan-manager.md](plan-manager.md) | Plan Manager fuer `.agents/plans`: Manage-Tab, plan-linked Tasks, Agent-Toolcalls, shared Context fuer Agent und Terminal-Handoff |
| done | [settings-tabs-themes-refactor.md](settings-tabs-themes-refactor.md) | Dynamische Workbench-Tabs; Settings inline; 20 Themes; Sidebar bleibt sichtbar |
| done | [skills-rules-tabs.md](skills-rules-tabs.md) | RightPanel-Tabs Skills & Rules: Cards, Toggle, Install-Dialog, agent toolcalls, index.json, System-Prompt |
| done | [terminal-grid-drag-drop.md](terminal-grid-drag-drop.md) | Terminal-Slots per Drag-Handle im Grid umsortieren (Cross-Workspace-Transfer ausserhalb Scope) |
| done | [terminal-agent-context-handoff.md](terminal-agent-context-handoff.md) | BLXCode Agent uebergibt Kontext per Toolcall an Terminal-Agent-Sessions; Bilder als Metadaten + lokale Pfade |
| done | [workspace-color-terminal-badge.md](workspace-color-terminal-badge.md) | Workspace-Farbe persistent in Sidebar (Dot, Terminal-Slot-Badge, farbiges Unread-Badge), bearbeitbar im Kontextmenue |
