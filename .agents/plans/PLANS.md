# Plans

Persistent plans for multi-step work on **blxcode**. Individual plans live as Markdown files in this directory.

## Index

| Status | Plan | Description |
|--------|------|-------------|
| active | [custom-app-titlebar.md](custom-app-titlebar.md) | Custom cross-plattform BLXCode-Titelbar (decorations:false): Brand+Version, Breadcrumb, Sidebar-Toggles (links/rechts) aus den Panels in die Bar verschoben, NAVIGATE-Popover (Terminal/New Terminal/Plans/Memory/Skills/Settings/Fullscreen), Notifications-Popover, Window-Controls; Drag via data-tauri-drag-region, Min/Max/Close/Fullscreen als Rust-Commands |
| planned | [provider-grouped-model-picker-logos.md](provider-grouped-model-picker-logos.md) | Composer Model-Picker mit Provider-Logos und One-Open-Accordion fuer OpenRouter/OpenAI/Anthropic; Cross-Provider-Auswahl speichert Provider+Model, Favorites/Search bleiben erhalten |
| active | [kanban-board-view.md](kanban-board-view.md) | Workspace Multi-Kanban als pinned Center-Tab 0 pro Workspace; Terminal bleibt aktiver Default; Plan-States aus Task-Summaries abgeleitet, Task-Lanes, Persistenz, Agent-Tools, Notifications, i18n und Layout-Import/Export |
| planned | [leptos-to-typescript-migration.md](leptos-to-typescript-migration.md) | Leptos/WASM restlos entfernen: Vite+React+TS Frontend (~53k LOC), Tauri-Backend unverändert, atomarer Cutover; validiert (5 Subagent-Reviews); ~9–12 PM |
| planned | [performance-optimization.md](performance-optimization.md) | Performance-Audit: Agent-Streaming hot path, Auto-Save-Kaskade, Backend-Blocking, Boot/CDN, Terminal-Refit; Phasen P0–P3 |
| planned | [russh-transport-refactor.md](russh-transport-refactor.md) | Remote-Transport von wrapped-ssh auf russh: eine multiplexte Verbindung pro Connection (1 TCP/1 Auth, Windows-fähig), Terminals=Channels, Exec via channel.exec (kein Marker-RPC), Host-Key-TOFU, Connection-Pool; Tasks RUSSH-01…22 |
| planned | [security-hardening.md](security-hardening.md) | Security-Audit: Subagent shell_write-Bypass, Shell-Allowlist, XSS/CSP, Runtime-Tool-Allowlist, Symlink/URL/PTY-Hardening; Phasen P0–P3 |
| planned | [agent-plan-status-lifecycle.md](agent-plan-status-lifecycle.md) | Agent-getriebener Plan-/Task-Status-Lebenszyklus: in_progress (Running) beim Abarbeiten setzen + automatisch auf pending/blocked/completed/cancelled, Markdown-Writeback, dedizierter In-Progress-Filter und Running-Markierung im Plans-Panel |
