# Plans

Persistent plans for multi-step work on **blxcode**. Individual plans live in subfolders as `plan.md` files.

## Index

| Status | Plan | Description |
|--------|------|-------------|
| active | [agent-mermaid-diagrams/plan.md](agent-mermaid-diagrams/plan.md) | Agent erzeugt eigenständig Mermaid-Diagramme (Plan/Task/Response/User-Wunsch): strukturiertes Tool + Fence-Fallback, Storage unter plans/<slug>/diagrams/, Inline-Card vs aufklappbare Tree-Gruppe → zentrierter Gallery-Tab (Thumbnail-Slider + Großansicht), .md/.pdf-Export (Orientierung aus SVG) via Save-As-Dialog, Cost-Gate + Workspace-Settings, Rollen-Autogen architect/coordinator |
| planned | [agent-plan-status-lifecycle/plan.md](agent-plan-status-lifecycle/plan.md) | Agent-getriebener Plan-/Task-Status-Lebenszyklus: in_progress (Running) beim Abarbeiten setzen + automatisch auf pending/blocked/completed/cancelled, Markdown-Writeback, dedizierter In-Progress-Filter und Running-Markierung im Plans-Panel |
| done | [codewright-role/plan.md](codewright-role/plan.md) | Global Codewright Coder role with memory/web/docs workflows, subagent usage, terminal swarm, and auto-accept file-edit permission UX |
| active | [custom-app-titlebar/plan.md](custom-app-titlebar/plan.md) | Custom cross-plattform BLXCode-Titelbar (decorations:false): Brand+Version, Breadcrumb, Sidebar-Toggles (links/rechts) aus den Panels in die Bar verschoben, NAVIGATE-Popover (Terminal/New Terminal/Plans/Memory/Skills/Settings/Fullscreen), Notifications-Popover, Window-Controls; Drag via data-tauri-drag-region, Min/Max/Close/Fullscreen als Rust-Commands |
| done | [heartbeat-memory-indexer/plan.md](heartbeat-memory-indexer/plan.md) | Globaler HeartBeat-Orchestrator mit plugin-ready interner Service-Registry, Settings-UI, Memory-Indexer fuer alle offenen Workspaces und Graph3D-gerechten Rules/Skills/Plans-Kategorien |
| active | [kanban-board-view/plan.md](kanban-board-view/plan.md) | Workspace Multi-Kanban als pinned Center-Tab 0 pro Workspace; Terminal bleibt aktiver Default; Plan-States aus Task-Summaries abgeleitet, Task-Lanes, Persistenz, Agent-Tools, Notifications, i18n und Layout-Import/Export |
| active | [kanban-dnd-agent-mermaid/plan.md](kanban-dnd-agent-mermaid/plan.md) | Kanban erweitern: Plan/Task per Drag&Drop in den BLXCode-Agent als Kontext anhängen (Workspace-Match, sonst Reject), plus Mermaid-Indikatoren auf Plan-Rows und Task-Cards mit Shortcut-Button, der die zentrierte DiagramGallery im Center-Tab öffnet |
| planned | [leptos-to-typescript-migration/plan.md](leptos-to-typescript-migration/plan.md) | Leptos/WASM restlos entfernen: Vite+React+TS Frontend (~53k LOC), Tauri-Backend unverändert, atomarer Cutover; validiert (5 Subagent-Reviews); ~9–12 PM |
| planned | [performance-optimization/plan.md](performance-optimization/plan.md) | Performance-Audit: Agent-Streaming hot path, Auto-Save-Kaskade, Backend-Blocking, Boot/CDN, Terminal-Refit; Phasen P0–P3 |
| done | [plan-folder-migration/plan.md](plan-folder-migration/plan.md) | Plans von flachen `.agents/plans/*.md` auf `.agents/plans/<slug>/plan.md` umstellen, inklusive async Migration, Statusbar-Fortschritt, Tools, Skills, i18n und Docs |
| planned | [provider-grouped-model-picker-logos/plan.md](provider-grouped-model-picker-logos/plan.md) | Composer Model-Picker mit Provider-Logos und One-Open-Accordion fuer OpenRouter/OpenAI/Anthropic; Cross-Provider-Auswahl speichert Provider+Model, Favorites/Search bleiben erhalten |
| planned | [russh-transport-refactor/plan.md](russh-transport-refactor/plan.md) | Remote-Transport von wrapped-ssh auf russh: eine multiplexte Verbindung pro Connection (1 TCP/1 Auth, Windows-fähig), Terminals=Channels, Exec via channel.exec (kein Marker-RPC), Host-Key-TOFU, Connection-Pool; Tasks RUSSH-01…22 |
| planned | [security-hardening/plan.md](security-hardening/plan.md) | Security-Audit: Subagent shell_write-Bypass, Shell-Allowlist, XSS/CSP, Runtime-Tool-Allowlist, Symlink/URL/PTY-Hardening; Phasen P0–P3 |
