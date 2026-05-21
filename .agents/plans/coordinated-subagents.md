# Coordinated Subagents fuer BLXCode Agent

## Summary

Erweitere den BLXCode Agent um koordinierte Subagents, die ueber denselben Provider, API-Key, Modell und Thinking-Level laufen wie der Hauptagent. Der Coordinator kann mehrere Subagents parallel starten, ihnen Rollen und explizite Toolgruppen geben und deren Live-Fortschritt als Inline-Subcards in der Chat Timeline anzeigen.

Subagents bekommen i18n-faehige Rollen-Namen, rollenbezogene System-Prompts, strukturierte JSON-Antworten und eigene Live-Events fuer Steps, Tasks und Toolcalls. Zusaetzlich entstehen globale Core Skills und Toolcalls fuer Environment Detection, Shell/Bash/PowerShell, Git, Web-Recherche, Diff und Workspace-Suche, damit Coordinator und Subagents plattformbewusst arbeiten koennen.

## Decisions

- Subagents verwenden immer die aktive BLXCode Agent Provider-Konfiguration; es gibt keine separaten Subagent-Provider-Settings in v1.
- Ausfuehrung ist parallel, aber begrenzt: Default `maxConcurrency = 3`, maximal 5 Subagents pro `subagents.run`.
- Subagents duerfen nicht rekursiv weitere Subagents starten; `subagents.run` wird aus deren Toolkatalog entfernt.
- Subagent-Rechte werden pro Toolcall ueber `allowedToolGroups` explizit gesetzt.
- Runtime vergibt stabile, rollenbasierte i18n-Displaynamen, damit der Coordinator die Agenten namentlich referenzieren kann.
- Mindestrollen sind `scout`, `review` und `security_analyst`.
- Vor Shell- oder Git-Nutzung muss der Agent im selben Turn `environment_detect` nutzen.
- Shell/Git sind globale BLXCode Agent Tools, werden Subagents aber nur ueber explizite Toolgruppen gegeben.
- Standard fuer Subagents bleibt read-only; mutierende Shell/Git-Rechte sind opt-in.
- Der Coordinator synthetisiert die sichtbare Endantwort; Subagents liefern nur strukturierte Ergebnisse.

## Implementation Notes

### Backend Agent Orchestration

- Neues Modul `src-tauri/src/agent/subagents.rs` anlegen.
- `session_orchestrator.rs` so erweitern, dass Tool-Dispatch Zugriff auf Provider, API-Key, Modell, Thinking-Level und Workspace Root hat.
- OpenAI/OpenRouter und Anthropic sollen einen gemeinsamen internen Tool-Dispatch-Pfad nutzen, damit `subagents.run` und neue Toolgruppen nicht doppelt gepflegt werden.
- Subagent-Lauf:
  - eigener transienter Message-Verlauf pro Subagent
  - gleicher Provider und gleiche Settings wie der Hauptagent
  - gefilterter Toolkatalog nach erlaubten Toolgruppen
  - keine Persistenz in der Haupt-Conversation
  - Cancellation ueber `AgentEngineState.cancelled()`
  - finales Toolresult enthaelt alle Subagent-JSON-Ergebnisse plus Runtime-Metadaten

### Tool API

- Neues Server-Tool `subagents.run`:

```json
{
  "agents": [
    {
      "id": "chat-ui-review",
      "role": "review",
      "title": "Chat UI Review",
      "task": "Inspect the chat timeline integration risks.",
      "successCriteria": ["Find UI touchpoints", "Return concrete risks"],
      "allowedToolGroups": ["environment_read", "workspace_read", "diff_read"]
    }
  ],
  "mode": "parallel",
  "maxConcurrency": 3
}
```

- Neue globale Tools:
  - `environment_detect`
  - `shell_exec`
  - `workspace_search`
  - `workspace_git_status`
  - `workspace_diff`
  - `git_status`
  - `git_diff`
  - `git_log`
  - `git_show`
  - `git_branch_info`
  - `git_ls_files`
  - `git_apply_patch`
  - `git_add`
  - `git_commit`
  - `web_search`
  - `web_fetch`

- `environment_detect` Rueckgabe:

```json
{
  "os": "linux|macos|windows",
  "arch": "x86_64|aarch64|...",
  "defaultShell": "bash|sh|pwsh|powershell",
  "availableShells": ["bash", "sh"],
  "pathSeparator": "/",
  "lineEnding": "\n",
  "gitAvailable": true,
  "workspaceRoot": "/abs/workspace"
}
```

- `shell_exec` ist nicht-interaktiv, arbeitet im Workspace-CWD, hat Timeout und Output-Limit und blockiert mutierende Absicht ohne `shell_write` Toolgruppe.
- Git v1 erlaubt kein `push`, `reset --hard`, `clean`, `checkout --` oder `rebase`.
- Mutierende Git-Tools sind nur ueber `git_write` verfuegbar.

### Toolgruppen

- `environment_read`: `environment_detect`
- `workspace_read`: `list_workspace_files`, `read_workspace_file`, `workspace_search`
- `diff_read`: `workspace_git_status`, `workspace_diff`, `git_status`, `git_diff`, `git_show`
- `git_read`: `git_status`, `git_diff`, `git_log`, `git_show`, `git_branch_info`, `git_ls_files`
- `git_write`: `git_apply_patch`, `git_add`, `git_commit`
- `shell_read`: `shell_exec` mit `writes:false`
- `shell_write`: `shell_exec` mit `writes:true`
- `web_read`: `web_search`, `web_fetch`
- `memory_read`, `plans_read`, `tasks_read`, `rules_skills_read`
- `memory_write`, `plans_write`, `tasks_write`, `rules_skills_write`

### Rollenprofile und Prompts

- `scout`: erkundet Codebase, Dateien, Architektur, Environment, Rules und Skills. Default Toolgruppen: `environment_read`, `workspace_read`, `git_read`, `memory_read`, `plans_read`, `rules_skills_read`.
- `review`: sucht Bugs, Regressionen, UX-Brueche, fehlende Tests und Diff-Risiken. Default Toolgruppen: `environment_read`, `workspace_read`, `diff_read`, `tasks_read`.
- `security_analyst`: prueft Secrets, Prompt-Injection, ungewollte Shell/Git/Netzwerk-Rechte, Tool-Scope und riskante Datenfluesse. Default Toolgruppen: `environment_read`, `workspace_read`, `diff_read`, `git_read`, optional `web_read`.

- Neuer `subagent_system_prompt(workspace_root, profile, display_name, agent_spec, allowed_tools)`.
- Prompt injiziert Rolle und Displayname, zum Beispiel: `You are Security Analyst, a BLXCode subagent specialized in security review...`.
- Finale Subagent-Antwort muss strikt JSON sein:

```json
{
  "status": "completed|blocked|failed",
  "role": "security_analyst",
  "displayName": "Security Analyst",
  "summary": "short result",
  "steps": [
    { "id": "inspect-scope", "title": "Inspect tool scope", "status": "completed", "note": "..." }
  ],
  "findings": [
    { "severity": "info|warning|error", "title": "...", "evidence": "...", "paths": ["src/..."] }
  ],
  "artifacts": [
    { "kind": "plan|patch_hint|source", "title": "...", "content": "..." }
  ],
  "recommendedNextActions": ["..."]
}
```

### Core Skills

- Neue eingebettete Core Skill Dateien:
  - `src-tauri/src/agent/harness_skills/subagents.md`
  - `src-tauri/src/agent/harness_skills/environment.md`
  - `src-tauri/src/agent/harness_skills/shell.md`
  - `src-tauri/src/agent/harness_skills/git.md`
  - `src-tauri/src/agent/harness_skills/web.md`
- `file-access.md` um `workspace_search` ergaenzen.
- `CORE_SKILLS` in `skills_rules/store.rs` erweitern.
- `system_prompt.rs` Core-Skill-Liste und Tool-Index aktualisieren.

### Protocol und UI

- `AgentEvent` und `src/agent_wire.rs` um Subagent-Events erweitern:
  - `SubagentStarted`
  - `SubagentStep`
  - `SubagentToolCall`
  - `SubagentFinished`
- `src/workbench/agent_timeline.rs` um `TimelineItem::SubagentGroup` erweitern.
- Chat View rendert pro Subagent eine Inline-Subcard:
  - Rollen-Displayname
  - Status
  - Live Steps/Tasks
  - Toolcall-Icons fuer Environment, Shell, Git, Web, File, Diff, Memory, Plans, Tasks
  - Details nur aufklappbar, damit die Chat View nicht spammt
- Timeline-Persistenz muss Subagent-Gruppen ohne grosse Toolresult-Payloads speichern.

### I18n

- Neue `I18nKey` Eintraege fuer Subagent UI, Rollenlabels, Toollabels, Environment/Shell/Git/Web Status und Card States.
- Mindest-Rollenlabels:
  - English: `Scout`, `Review`, `Security Analyst`
  - Deutsch: `Scout`, `Review`, `Security-Analyst`
- Alle Locale-Tabellen muessen vollstaendig bleiben; keine hartcodierten Subagent-UI-Texte.

## Tests

- Backend:
  - `subagents.run` ist im Toolkatalog.
  - `environment_detect` erkennt OS, Default-Shell, verfuegbare Shells, Git-Verfuegbarkeit und Workspace Root.
  - `shell_exec` nutzt Workspace-CWD, Timeout und Output-Limit.
  - `shell_exec` blockiert mutierende Nutzung ohne `shell_write`.
  - Git read-only Tools verlassen den Workspace nicht und liefern begrenzte Ausgabe.
  - Mutierende Git-Tools sind ohne `git_write` nicht verfuegbar.
  - Toolgruppen filtern Shell/Git/Web/Workspace/Memory/Plan/Task Tools korrekt.
  - Rollen `scout`, `review`, `security_analyst` erzeugen passende Prompt-Fragmente.
  - Namensvergabe ist stabil, lokalisiert und konfliktfrei innerhalb eines Toolcalls.
  - Subagent JSON wird valide geparst; invalides JSON erzeugt ein `failed` Ergebnis.
  - Cancellation beendet offene Subagent-Runs.

- Frontend:
  - `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
  - Subagent Timeline Items persistieren und rehydrieren korrekt.
  - Live Events aktualisieren bestehende Subcards statt neue Spam-Zeilen zu erzeugen.
  - Toolcalls werden als kompakte Icons dargestellt und bleiben aufklappbar.
  - i18n Tabellen sind vollstaendig.

- Workspace:
  - `cargo test -p blxcode`
  - `cargo check -p blxcode`
  - Manuelle Pruefung: Scout/Review/Security Analyst Subcards in der Agent Chat View, parallel laufende Steps, Cancel, Tool-Icon-Darstellung.

## Tasks

- [ ] `environment-tool-skill` - `environment_detect` Tool und `environment` Core Skill implementieren.
- [ ] `shell-tool-skill` - `shell_exec` Tool und `shell` Core Skill mit Bash/PowerShell-Regeln implementieren.
- [ ] `git-tool-skill` - Read-only und erlaubte mutierende Git Tools plus `git` Core Skill implementieren.
- [ ] `web-tools-skill` - `web_search`, `web_fetch` und `web` Core Skill mit sicheren Limits ergaenzen.
- [ ] `workspace-search-diff` - `workspace_search`, `workspace_git_status` und `workspace_diff` als Agent Tools ergaenzen.
- [ ] `toolgroup-filtering` - Toolgruppenmodell fuer Coordinator und Subagents implementieren und testen.
- [ ] `subagent-orchestrator` - `subagents.run`, Parallelitaetslimit, Cancellation und Provider-Reuse implementieren.
- [ ] `subagent-role-prompts` - Rollenprofile, i18n Displaynamen und `subagent_system_prompt` fuer Scout, Review und Security Analyst ergaenzen.
- [ ] `subagent-protocol` - Backend/Frontend Wire Types und `AgentEvent` Subagent-Events spiegeln.
- [ ] `subagent-chat-ui` - Inline-Subcards mit Live Steps/Tasks und Toolcall-Icons in der Chat Timeline bauen.
- [ ] `core-skill-index-prompt` - `CORE_SKILLS`, Core Skill Docs und `system_prompt.rs` Toolindex aktualisieren.
- [ ] `i18n-subagents-tools` - Alle neuen UI-, Rollen- und Toollabels in allen Locale-Dateien ergaenzen.
- [ ] `verification` - Rust Tests, Frontend Check, Backend Check und manuelle Chat-View-Pruefung durchfuehren.
