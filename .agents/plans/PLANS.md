# Plans

Persistent plans for multi-step work on **blxcode**. Individual plans live as Markdown files in this directory.

## Index

| Status | Plan | Description |
|--------|------|-------------|
| planned | [kanban-board-view.md](kanban-board-view.md) | Kanban-View im Plans-Panel: Status-Spalten, DnD fuer Karten/Spalten, Spalten ein-/ausblenden, Markdown-Writeback (Full Scope; MVP in v2-roadmap) |
| planned | [leptos-to-typescript-migration.md](leptos-to-typescript-migration.md) | Leptos/WASM restlos entfernen: Vite+React+TS Frontend (~53k LOC), Tauri-Backend unverändert, atomarer Cutover; validiert (5 Subagent-Reviews); ~9–12 PM |
| planned | [performance-optimization.md](performance-optimization.md) | Performance-Audit: Agent-Streaming hot path, Auto-Save-Kaskade, Backend-Blocking, Boot/CDN, Terminal-Refit; Phasen P0–P3 |
| planned | [russh-transport-refactor.md](russh-transport-refactor.md) | Remote-Transport von wrapped-ssh auf russh: eine multiplexte Verbindung pro Connection (1 TCP/1 Auth, Windows-fähig), Terminals=Channels, Exec via channel.exec (kein Marker-RPC), Host-Key-TOFU, Connection-Pool; Tasks RUSSH-01…22 |
| planned | [security-hardening.md](security-hardening.md) | Security-Audit: Subagent shell_write-Bypass, Shell-Allowlist, XSS/CSP, Runtime-Tool-Allowlist, Symlink/URL/PTY-Hardening; Phasen P0–P3 |
