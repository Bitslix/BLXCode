# Coordinated Subagents fuer BLXCode Agent

## Summary

Erweitere den BLXCode Agent um koordinierte Subagents, die ueber denselben Provider, API-Key, Modell und Thinking-Level laufen wie der Hauptagent. Der Coordinator kann mehrere Subagents parallel starten, ihnen Rollen und explizite Toolgruppen geben und deren Live-Fortschritt als Inline-Subcards in der Chat Timeline anzeigen.

Subagents bekommen i18n-faehige Rollen-Namen, rollenbezogene System-Prompts, strukturierte Antworten ueber ein forciertes `submit_result` Tool und eigene Live-Events fuer Steps, Tasks und Toolcalls. Zusaetzlich entstehen globale Core Skills und Toolcalls fuer Environment Detection, Shell/Bash/PowerShell, Git, Web-Recherche, Diff und Workspace-Suche, damit Coordinator und Subagents plattformbewusst arbeiten koennen.

Vor dem Subagent-Feature wird der Tool-Dispatch-Pfad fuer Anthropic und OpenRouter vereinheitlicht, damit neue Toolgruppen und `subagents.run` nur an einer Stelle gepflegt werden muessen.

## Decisions

- Subagents verwenden immer die aktive BLXCode Agent Provider-Konfiguration; es gibt keine separaten Subagent-Provider-Settings in v1.
- Ausfuehrung ist parallel, aber begrenzt: Default `maxConcurrency = 3`, maximal 5 Subagents pro `subagents.run`.
- Subagents duerfen nicht rekursiv weitere Subagents starten; `subagents.run` wird aus deren Toolkatalog entfernt.
- Subagent-Rechte werden pro Toolcall ueber `allowedToolGroups` explizit gesetzt.
- Runtime vergibt stabile, rollenbasierte i18n-Displaynamen; bei Konflikt (gleiche Rolle ohne unique `title`) werden automatisch Suffixe (`Review 1`, `Review 2`) vergeben.
- Mindestrollen sind `scout`, `review` und `security_analyst`.
- Vor Shell- oder Git-Nutzung muss der Agent im selben Turn `environment_detect` nutzen.
- `environment_detect` wird pro Session gecached und bei Workspace-Wechsel invalidiert.
- Shell/Git sind globale BLXCode Agent Tools, werden Subagents aber nur ueber explizite Toolgruppen gegeben.
- Standard fuer Subagents bleibt read-only; mutierende Shell/Git-Rechte sind opt-in.
- Der Coordinator synthetisiert die sichtbare Endantwort; Subagents liefern strukturierte Ergebnisse nur ueber ein forciertes `submit_result` Tool.
- Subagent-Loop endet erst, wenn `submit_result` aufgerufen wurde, eine Iteration- oder Token-Cap erreicht ist oder der Lauf abgebrochen wurde.
- Pro Subagent gelten Hard-Caps: max `20_000` Output-Tokens und max `8` Tool-Iterations. Bei Cap-Erreichen wird `status: "blocked"` mit Begruendung zurueckgegeben.
- `shell_exec` ohne `shell_write` arbeitet ueber eine strikte Allowlist read-only Commands; `git` Subcommands werden separat geparst.
- Cancellation killt laufende Shell-Child-Prozesse aktiv: SIGTERM sofort, nach 2s SIGKILL; Windows nutzt `TerminateProcess`.
- Web-Tools (`web_search`, `web_fetch`) unterstuetzen in v1 zwei Backends: Tavily und Brave. Auswahl und API-Keys werden in den Settings unter Agent-Tab gepflegt; Env-Var-Fallback (`BLX_TAVILY_API_KEY`, `BLX_BRAVE_API_KEY`) bleibt unterstuetzt.
- Web-Tools sind deaktiviert (nicht im Toolkatalog) wenn weder Settings noch Env-Var einen API-Key fuer das gewaehlte Backend liefern; das `web` Core Skill bleibt sichtbar aber markiert sich als `disabled` mit Hinweis auf die Settings.
- Coordinator startet Subagents nicht autonom; er nutzt `subagents.run` nur auf explizite User-Anweisung (z.B. "nutze Subagents", "parallel review", "lass das ein Security-Analyst pruefen"). Der System-Prompt enthaelt klare Aktivierungs-Trigger.
- Anthropic und OpenRouter Tool-Dispatch werden vereinheitlicht, bevor Subagents implementiert werden.

## Implementation Notes

### Backend Agent Orchestration

- Neues Modul `src-tauri/src/agent/subagents.rs` anlegen.
- `session_orchestrator.rs` so erweitern, dass Tool-Dispatch Zugriff auf Provider, API-Key, Modell, Thinking-Level und Workspace Root hat.
- OpenAI/OpenRouter und Anthropic nutzen einen gemeinsamen internen Tool-Dispatch-Pfad (eigener vorgelagerter Refactor), damit `subagents.run` und neue Toolgruppen nicht doppelt gepflegt werden.
- Subagent-Lauf:
  - eigener transienter Message-Verlauf pro Subagent
  - gleicher Provider und gleiche Settings wie der Hauptagent
  - gefilterter Toolkatalog nach erlaubten Toolgruppen plus erzwungenes `submit_result` Tool
  - Iteration- und Output-Token-Cap pro Subagent
  - keine Persistenz in der Haupt-Conversation
  - Cancellation ueber `AgentEngineState.cancelled()` plus aktives Killen laufender Shell-Child-Prozesse
  - finales Toolresult enthaelt alle Subagent-Ergebnisse (geparstes `submit_result` Payload) plus Runtime-Metadaten (genutzte Tokens, Iterationen, Dauer)

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
  - `web_search` (nur verfuegbar wenn API-Key gesetzt)
  - `web_fetch` (nur verfuegbar wenn API-Key gesetzt)

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

- `shell_exec` ist nicht-interaktiv, arbeitet im Workspace-CWD, hat Timeout und Output-Limit.
- Ohne `shell_write` Toolgruppe gilt eine strikte Allowlist read-only Commands. Vorschlag v1: `ls`, `pwd`, `cat`, `head`, `tail`, `wc`, `file`, `which`, `env` (nur lesend), `rg`, `fd`, `find` (ohne `-exec`/`-delete`/`-prune`-Tricks), `tree`, `stat`, `du`, `df`, `node --version`, `npm --version`, `cargo metadata`, `cargo check`, `cargo tree`, `git` mit read-only Subcommands.
- `git`-Aufrufe ueber `shell_exec` werden Subcommand-genau geparst (`status`, `log`, `diff`, `show`, `branch`, `ls-files` etc. = read; `commit`, `push`, `reset`, `rebase`, `clean`, `checkout --` = write).
- Git v1 erlaubt kein `push`, `reset --hard`, `clean`, `checkout --` oder `rebase`.
- Mutierende Git-Tools sind nur ueber `git_write` verfuegbar.
- Cancellation: jeder laufende Shell-Prozess wird beim Abbruch SIGTERM-signalisiert; wenn nach 2s noch lebendig, folgt SIGKILL. Auf Windows `TerminateProcess`. Plattformcode in `shell_exec` Implementierung.

### Subagent submit_result Tool

- Subagent-Tool-Loop registriert ein forciertes `submit_result` Tool mit JSON-Schema:

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

- Der Subagent-Loop endet erst beim `submit_result` Aufruf, sonst bei Cap oder Cancellation.
- Bei Cap-Erreichen synthetisiert die Runtime ein `submit_result`-aequivalentes Ergebnis mit `status: "blocked"` und entsprechender `summary`.
- Bei Schema-Validierungsfehler im `submit_result` Payload: Runtime gibt `status: "failed"` mit Original-Payload als Evidenz zurueck.

### Toolgruppen

- `environment_read`: `environment_detect`
- `workspace_read`: `list_workspace_files`, `read_workspace_file`, `workspace_search`
- `diff_read`: `workspace_git_status`, `workspace_diff`, `git_status`, `git_diff`, `git_show`
- `git_read`: `git_status`, `git_diff`, `git_log`, `git_show`, `git_branch_info`, `git_ls_files`
- `git_write`: `git_apply_patch`, `git_add`, `git_commit`
- `shell_read`: `shell_exec` mit `writes:false` (Allowlist-Modus)
- `shell_write`: `shell_exec` mit `writes:true` (Allowlist aufgehoben, beliebige Commands erlaubt)
- `web_read`: `web_search`, `web_fetch` (nur aktiv wenn API-Key gesetzt)
- `memory_read`, `plans_read`, `tasks_read`, `rules_skills_read`
- `memory_write`, `plans_write`, `tasks_write`, `rules_skills_write`

### Web Tools Backend und Settings

- Backend-Optionen in v1: `tavily` und `brave`.
- Settings-Schema bekommt einen `agent.web` Abschnitt: `provider: "tavily" | "brave" | "none"`, `tavilyApiKey: string`, `braveApiKey: string`.
- Settings UI: neuer Agent-Tab Block "Web Tools" mit Backend-Auswahl und passwordartigen Input-Feldern fuer API-Keys, plus Test-Button. i18n fuer alle Labels.
- Env-Var-Fallback: `BLX_TAVILY_API_KEY`, `BLX_BRAVE_API_KEY` werden gelesen wenn das Settings-Feld leer ist.
- Runtime entscheidet beim Toolkatalog-Bau: ist kein API-Key fuer das gewaehlte Backend verfuegbar, werden `web_search` und `web_fetch` nicht registriert; das `web` Core Skill markiert sich als `disabled` mit Hinweis auf die Settings-Position.

### Rollenprofile und Prompts

- `scout`: erkundet Codebase, Dateien, Architektur, Environment, Rules und Skills. Default Toolgruppen: `environment_read`, `workspace_read`, `git_read`, `memory_read`, `plans_read`, `rules_skills_read`.
- `review`: sucht Bugs, Regressionen, UX-Brueche, fehlende Tests und Diff-Risiken. Default Toolgruppen: `environment_read`, `workspace_read`, `diff_read`, `tasks_read`.
- `security_analyst`: prueft Secrets, Prompt-Injection, ungewollte Shell/Git/Netzwerk-Rechte, Tool-Scope und riskante Datenfluesse. Default Toolgruppen: `environment_read`, `workspace_read`, `diff_read`, `git_read`, optional `web_read`.

- Neuer `subagent_system_prompt(workspace_root, profile, display_name, agent_spec, allowed_tools)`.
- Prompt injiziert Rolle und Displayname, zum Beispiel: `You are Security Analyst, a BLXCode subagent specialized in security review...`.
- Prompt enthaelt expliziten Hinweis, dass die finale Antwort ausschliesslich ueber `submit_result` erfolgt und freier Text ignoriert wird.

### Coordinator Prompt

- `system_prompt.rs` wird um einen Block "Subagents" erweitert.
- Coordinator nutzt `subagents.run` nur bei expliziten User-Triggern wie "nutze Subagents", "parallel review", "lass das einen Security-Analyst pruefen", "spawn N agents". Default: Coordinator arbeitet allein.
- Prompt erklaert die drei Rollen, die Toolgruppen und das parallele Limit.

### Core Skills

- Neue eingebettete Core Skill Dateien:
  - `src-tauri/src/agent/harness_skills/subagents.md`
  - `src-tauri/src/agent/harness_skills/environment.md`
  - `src-tauri/src/agent/harness_skills/shell.md`
  - `src-tauri/src/agent/harness_skills/git.md`
  - `src-tauri/src/agent/harness_skills/web.md` (mit Hinweis dass es deaktiviert ist solange kein API-Key gesetzt ist)
- `file-access.md` um `workspace_search` ergaenzen.
- `CORE_SKILLS` in `skills_rules/store.rs` erweitern; `web` Skill bekommt ein `enabled` Flag, das Runtime-abhaengig gesetzt wird.
- `system_prompt.rs` Core-Skill-Liste und Tool-Index aktualisieren.

### Protocol und UI

- `AgentEvent` und `src/agent_wire.rs` um Subagent-Events erweitern:
  - `SubagentStarted`
  - `SubagentStep`
  - `SubagentToolCall`
  - `SubagentFinished`
- `src/workbench/agent_timeline.rs` um `TimelineItem::SubagentGroup` erweitern.
- Chat View rendert pro Subagent eine Inline-Subcard:
  - Rollen-Displayname (mit Auto-Suffix bei Konflikt)
  - Status
  - Live Steps/Tasks
  - Toolcall-Icons fuer Environment, Shell, Git, Web, File, Diff, Memory, Plans, Tasks
  - Details nur aufklappbar, damit die Chat View nicht spammt
- Frontend-seitiges Event-Batching mit ca. 50ms Debounce, damit Bursts paralleler Subagent-Events die Timeline nicht blockieren.
- Timeline-Persistenz muss Subagent-Gruppen ohne grosse Toolresult-Payloads speichern.

### I18n

- Neue `I18nKey` Eintraege fuer:
  - Subagent UI (Card States, Status-Texte, Empty States)
  - Rollenlabels und Auto-Suffix-Format
  - Toollabels (Environment, Shell, Git, Web, Workspace-Search, Diff)
  - Environment/Shell/Git/Web Status
  - Web-Tools-Disabled-Hinweis mit Link zur Settings-Position
  - Settings Agent-Tab "Web Tools" Block (Backend-Picker, API-Key-Felder, Test-Button)
- Mindest-Rollenlabels:
  - English: `Scout`, `Review`, `Security Analyst`
  - Deutsch: `Scout`, `Review`, `Security-Analyst`
- Alle Locale-Tabellen muessen vollstaendig bleiben; keine hartcodierten Subagent-UI-Texte.

## Tests

- Backend:
  - Vereinheitlichter Tool-Dispatch-Pfad: Anthropic- und OpenRouter-Provider rufen denselben Dispatcher fuer alle Tools.
  - `subagents.run` ist im Toolkatalog des Coordinators, nicht im Subagent-Katalog.
  - `environment_detect` erkennt OS, Default-Shell, verfuegbare Shells, Git-Verfuegbarkeit und Workspace Root.
  - `environment_detect` Cache wird bei Workspace-Wechsel invalidiert.
  - `shell_exec` nutzt Workspace-CWD, Timeout und Output-Limit.
  - `shell_exec` ohne `shell_write` lehnt Commands ausserhalb der Allowlist ab.
  - `shell_exec` parst `git` Subcommands und blockiert mutierende Subcommands ohne `shell_write`.
  - `shell_exec` SIGTERM/SIGKILL Cancellation wirkt auf laufende Child-Prozesse.
  - Git read-only Tools verlassen den Workspace nicht und liefern begrenzte Ausgabe.
  - Mutierende Git-Tools sind ohne `git_write` nicht verfuegbar.
  - Toolgruppen filtern Shell/Git/Web/Workspace/Memory/Plan/Task Tools korrekt.
  - Rollen `scout`, `review`, `security_analyst` erzeugen passende Prompt-Fragmente.
  - Namensvergabe ist stabil und lokalisiert; bei Konflikt wird Auto-Suffix vergeben.
  - `submit_result` ist im Subagent-Toolkatalog; Subagent-Loop terminiert beim Aufruf.
  - Iteration- und Token-Cap fuehrt zu `status: "blocked"` mit Begruendung.
  - Cancellation beendet offene Subagent-Runs und deren Child-Prozesse.
  - `web_search` und `web_fetch` sind nicht im Toolkatalog wenn kein API-Key gesetzt; mit gesetztem Key sind sie verfuegbar und nutzen das gewaehlte Backend.
  - Settings-Schema validiert Web-Backend-Auswahl und API-Keys.

- Frontend:
  - `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
  - Subagent Timeline Items persistieren und rehydrieren korrekt.
  - Live Events aktualisieren bestehende Subcards statt neue Spam-Zeilen zu erzeugen.
  - Event-Debounce verhindert UI-Lag bei parallelen Subagent-Bursts.
  - Toolcalls werden als kompakte Icons dargestellt und bleiben aufklappbar.
  - Settings Agent-Tab "Web Tools" Block speichert, liest und validiert API-Keys.
  - `web` Core Skill zeigt Disabled-Hinweis wenn kein API-Key vorhanden ist.
  - i18n Tabellen sind vollstaendig.

- Workspace:
  - `cargo test -p blxcode`
  - `cargo check -p blxcode`
  - Manuelle Pruefung: Scout/Review/Security Analyst Subcards in der Agent Chat View, parallel laufende Steps, Cancel mit aktivem Shell-Prozess, Tool-Icon-Darstellung, Web-Tools mit/ohne API-Key, Settings-Tab.

## Tasks

- [ ] `dispatch-unify` - Anthropic und OpenRouter Tool-Dispatch-Pfad zu einem gemeinsamen internen Dispatcher refaktorieren; Voraussetzung fuer alle weiteren Tasks.
- [ ] `environment-tool-skill` - `environment_detect` Tool und `environment` Core Skill implementieren, inkl. Session-Cache mit Workspace-Invalidation.
- [ ] `shell-tool-skill` - `shell_exec` Tool und `shell` Core Skill mit Allowlist-Modell, Bash/PowerShell-Regeln, Git-Subcommand-Parsing und SIGTERM/SIGKILL Cancellation.
- [ ] `git-tool-skill` - Read-only und erlaubte mutierende Git Tools plus `git` Core Skill implementieren.
- [ ] `settings-web-api-keys` - Settings-Schema, Agent-Tab UI Block "Web Tools" mit Backend-Picker (Tavily/Brave), API-Key-Feldern, Env-Var-Fallback und i18n-Labels.
- [ ] `web-tools-skill` - `web_search`, `web_fetch` und `web` Core Skill mit sicheren Limits, Backend-Switch und Disabled-State wenn kein API-Key gesetzt.
- [ ] `workspace-search-diff` - `workspace_search`, `workspace_git_status` und `workspace_diff` als Agent Tools ergaenzen.
- [ ] `toolgroup-filtering` - Toolgruppenmodell fuer Coordinator und Subagents implementieren und testen.
- [ ] `subagent-orchestrator` - `subagents.run`, Parallelitaetslimit, Cancellation, Provider-Reuse, `submit_result` Tool und Token/Iteration-Caps implementieren.
- [ ] `subagent-role-prompts` - Rollenprofile, i18n Displaynamen, Auto-Suffix-Naming und `subagent_system_prompt` fuer Scout, Review und Security Analyst; Coordinator-System-Prompt mit expliziten Aktivierungs-Triggern.
- [ ] `subagent-protocol` - Backend/Frontend Wire Types und `AgentEvent` Subagent-Events spiegeln.
- [ ] `subagent-chat-ui` - Inline-Subcards mit Live Steps/Tasks, Toolcall-Icons und Frontend-Event-Debounce in der Chat Timeline bauen.
- [ ] `core-skill-index-prompt` - `CORE_SKILLS`, Core Skill Docs, `web`-Skill Enabled-Flag und `system_prompt.rs` Toolindex aktualisieren.
- [ ] `i18n-subagents-tools` - Alle neuen UI-, Rollen-, Tool-, Settings- und Disabled-State-Labels in allen Locale-Dateien ergaenzen.
- [ ] `verification` - Rust Tests, Frontend Check, Backend Check und manuelle Chat-View-Pruefung durchfuehren.
