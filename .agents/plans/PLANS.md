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
| planned | [agent-plan-status-lifecycle.md](agent-plan-status-lifecycle.md) | Agent-getriebener Plan-/Task-Status-Lebenszyklus: in_progress (Running) beim Abarbeiten setzen + automatisch auf pending/blocked/completed/cancelled, Markdown-Writeback, dedizierter In-Progress-Filter und Running-Markierung im Plans-Panel |
| planned | [voice-push-to-talk-stt.md](voice-push-to-talk-stt.md) | Push-to-Talk STT: lokales whisper.cpp (warm, off-thread, Feature-`local-whisper`) + Cloud-Reuse; erweitert bestehendes voice-Subsystem (recorder/stt/settings/voice_orb/harness_voice_pane) statt Parallelbau; Ziel-Routing Composer/Terminal/Active-Input/Clipboard mit Remember-Target, Kollisions-State-Machine, optionale Partials; AWS-Polly bleibt STT-gesperrt; Phasen P0–P5 |
| planned | [ai-plan-task-generation.md](ai-plan-task-generation.md) | AI-gestützte Plan-/Task-Generierung: zwei Header-Buttons (AI Plan / AI Tasks) öffnen Dialog mit Prompt-Textbox, Loading-Animation, scrollbarer Markdown-Preview und Tasks-Toggle; reuse oneshot::complete_text (wie git_commit_ai) + Plan-Skill-Format, Speichern via plan_create/plan_load |
| done | [terminal-naming-and-claude-theme.md](terminal-naming-and-claude-theme.md) | Terminal-Titel wahlweise Slot-Nummer oder Agent-Name (editierbarer Pool, Per-Slot-Override, Doppelklick-/Kontextmenü-Rename); Agent kennt Namen via list_terminals; github-dark → „Claude Code"-Theme |
