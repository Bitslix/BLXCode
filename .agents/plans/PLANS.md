# Plans

Persistent plans for multi-step work on **blxcode**. Individual plans live in subfolders as `plan.md` files.

## Index

| Status | Plan | Description |
|--------|------|-------------|
| planned | [agent-plan-status-lifecycle/plan.md](agent-plan-status-lifecycle/plan.md) | Agent-getriebener Plan-/Task-Status-Lebenszyklus: in_progress (Running) beim Abarbeiten setzen + automatisch auf pending/blocked/completed/cancelled, Markdown-Writeback, dedizierter In-Progress-Filter und Running-Markierung im Plans-Panel |
| planned | [leptos-to-typescript-migration/plan.md](leptos-to-typescript-migration/plan.md) | Leptos/WASM restlos entfernen: Vite+React+TS Frontend (~53k LOC), Tauri-Backend unverändert, atomarer Cutover; validiert (5 Subagent-Reviews); ~9–12 PM |
| planned | [performance-optimization/plan.md](performance-optimization/plan.md) | Performance-Audit: Agent-Streaming hot path, Auto-Save-Kaskade, Backend-Blocking, Boot/CDN, Terminal-Refit; Phasen P0–P3 |
| planned | [russh-transport-refactor/plan.md](russh-transport-refactor/plan.md) | Remote-Transport von wrapped-ssh auf russh: eine multiplexte Verbindung pro Connection (1 TCP/1 Auth, Windows-fähig), Terminals=Channels, Exec via channel.exec (kein Marker-RPC), Host-Key-TOFU, Connection-Pool; Tasks RUSSH-01…22 |
| planned | [security-hardening/plan.md](security-hardening/plan.md) | Security-Audit: Subagent shell_write-Bypass, Shell-Allowlist, XSS/CSP, Runtime-Tool-Allowlist, Symlink/URL/PTY-Hardening; Phasen P0–P3 |
| active | [window-popout/plan.md](window-popout/plan.md) | Real Tauri child-window popouts for terminal, memory, Mermaid, and file diff views with custom BLXCode titlebars |
