# Coordinated Subagents fuer BLXCode Agent

## Summary

Erweitere den BLXCode Agent um koordinierte Subagents, die ueber denselben Provider, API-Key, Modell und Thinking-Level laufen wie der Hauptagent. Der Coordinator kann mehrere Subagents parallel starten, ihnen Rollen und explizite Toolgruppen geben und deren Live-Fortschritt als Inline-Subcards in der Chat Timeline anzeigen.

Subagents bekommen i18n-faehige Rollen-Namen, rollenbezogene System-Prompts, strukturierte Antworten ueber ein forciertes `submit_result` Tool und eigene Live-Events fuer Steps, Tasks und Toolcalls. Zusaetzlich entstehen globale Core Skills und Toolcalls fuer Environment Detection, Shell/Bash/PowerShell, Git, Web-Recherche, Diff und Workspace-Suche, damit Coordinator und Subagents plattformbewusst arbeiten koennen.

**Voraussetzung:** [better-harness.md](better-harness.md) (Core Skills, schlanker System-Prompt) ist erledigt.

Vor dem Subagent-Feature wird der Tool-Dispatch-Pfad fuer Anthropic, OpenRouter und OpenAI (OpenAI-compatible) vereinheitlicht, damit neue Toolgruppen und `subagents.run` nur an einer Stelle gepflegt werden muessen.

### Phasen (empfohlene PR-Reihenfolge)

- **Phase A — Infrastruktur:** `dispatch-unify`, `toolgroup-filtering`, `environment-tool-skill`, `shell-tool-skill`, `git-tool-skill`, `workspace-search-diff`
- **Phase B — Subagents:** `subagent-orchestrator`, `subagent-role-prompts`, `subagent-protocol`, `subagent-chat-ui`, `core-skill-index-prompt`, `i18n-subagents-tools`, `verification`
- **Phase C — Web (optional entkoppelt):** `settings-web-api-keys`, `web-tools-skill` (kann nach Phase B folgen, blockiert Subagents nicht)

## Decisions

- Subagents verwenden immer die aktive BLXCode Agent Provider-Konfiguration; es gibt keine separaten Subagent-Provider-Settings in v1.
- Ausfuehrung ist parallel, aber begrenzt: Default `maxConcurrency = 3`, maximal 5 Subagents pro `subagents.run`.
- Subagents duerfen nicht rekursiv weitere Subagents starten; `subagents.run` wird aus deren Toolkatalog entfernt.
- Subagent-Rechte werden pro Toolcall ueber `allowedToolGroups` explizit gesetzt.
- Runtime vergibt stabile, rollenbasierte i18n-Displaynamen; bei Konflikt (gleiche Rolle ohne unique `title`) werden automatisch Suffixe (`Review 1`, `Review 2`) vergeben.
- Mindestrollen sind `scout`, `review` und `security_analyst`.
- Vor Shell- oder Git-Nutzung muss `environment_detect` gelaufen sein; serverseitig lehnen `shell_exec` und `git_*` den Aufruf ab, wenn kein gueltiger Session-Cache fuer den aktuellen Workspace existiert (nicht nur Prompt-Hinweis).
- `environment_detect` wird pro Session gecached und bei Workspace-Wechsel invalidiert.
- Shell/Git sind globale BLXCode Agent Tools, werden Subagents aber nur ueber explizite Toolgruppen gegeben.
- Standard fuer Subagents bleibt read-only; mutierende Shell/Git-Rechte sind opt-in.
- Der Coordinator synthetisiert die sichtbare Endantwort; Subagents liefern strukturierte Ergebnisse nur ueber ein forciertes `submit_result` Tool.
- Subagent-Loop endet erst, wenn `submit_result` aufgerufen wurde, eine Iteration- oder Token-Cap erreicht ist oder der Lauf abgebrochen wurde.
- Pro Subagent gelten Hard-Caps: max `20_000` Output-Tokens und max `8` Tool-Iterations (Caps gelten **pro Subagent-Lauf**, nicht pro Batch). Bei Cap-Erreichen wird `status: "blocked"` mit Begruendung zurueckgegeben.
- Output-Token-Zaehlung: Provider-`usage` aus der finalen Response/SSE akkumulieren wenn vorhanden; sonst konservative Schaetzung (`assistant`-Text + Tool-Result-Bytes, ceil(chars/4)). Bei Ueberschreitung synthetisches `submit_result` mit `blocked`.
- `subagents.run` Toolresult an den Coordinator: max `20` findings und `10` artifacts **pro Subagent**; laengere Inhalte werden auf Summary gekuerzt. Vollstaendige Steps/Findings bleiben in Subagent-UI-Events, nicht im Coordinator-Toolpayload.
- `shell_write` ist in v1 **nur fuer den Coordinator** erlaubt (nie in Subagent-`allowedToolGroups`). Subagents bleiben read-only fuer Shell; mutierende Git nur ueber explizite `git_write`-Gruppe beim Coordinator.
- `shell_exec` ohne `shell_write` arbeitet ueber eine strikte Allowlist read-only Commands. `git` ueber `shell_exec` nur read-only Subcommands; mutierende Git ausschliesslich ueber dedizierte `git_*` Tools mit `git_write`.
- Cancellation killt laufende Shell-Child-Prozesse aktiv: SIGTERM sofort, nach 2s SIGKILL; Windows nutzt `TerminateProcess`.
- Web-Tools (`web_search`, `web_fetch`) unterstuetzen in v1 zwei Backends: Tavily und Brave. Auswahl wird in Settings unter Agent-Tab gepflegt; API-Keys werden wie Provider-Keys in der **Keyring**-Pipeline gespeichert (`BLXCode` service, accounts `agent:web:tavily` / `agent:web:brave`), nicht als Klartext in `agent_provider_settings.json`. Env-Var-Fallback (`BLX_TAVILY_API_KEY`, `BLX_BRAVE_API_KEY`) bleibt unterstuetzt.
- Web-Tools sind deaktiviert (nicht im Toolkatalog) wenn weder Keyring noch Env-Var einen API-Key fuer das gewaehlte Backend liefern; das `web` Core Skill bleibt in `skills_list` sichtbar mit Runtime-Flag `availability: disabled_no_key` (nicht nur Prompt-Hinweis).
- **Harness vs Shell:** `harness.open_terminal` / `harness.send_terminal_keys` / `harness.send_agent_context` fuer interaktive Terminal-CLIs und Context-Handoff; `shell_exec` fuer einmalige, nicht-interaktive Befehle im Workspace-CWD. Core Skills `harness.md` und `shell.md` dokumentieren die Abgrenzung.
- Parallele Subagents (bis 3 gleichzeitig) vervielfachen API-Kosten/Latenz; Coordinator-Prompt warnt knapp, wenn der User mehrere Subagents anfordert.
- Coordinator startet Subagents nicht autonom; er nutzt `subagents.run` nur auf explizite User-Anweisung (z.B. "nutze Subagents", "parallel review", "lass das ein Security-Analyst pruefen"). Der System-Prompt enthaelt klare Aktivierungs-Trigger.
- Anthropic, OpenRouter und OpenAI (OpenAI-compatible `run_chat_turn`) Tool-Dispatch werden in einem gemeinsamen Modul vereinheitlicht, bevor Subagents implementiert werden.

## Implementation Notes

### Backend Agent Orchestration

- Neues Modul `src-tauri/src/agent/subagents.rs` anlegen.
- `session_orchestrator.rs` so erweitern, dass Tool-Dispatch Zugriff auf Provider, API-Key, Modell, Thinking-Level und Workspace Root hat.
- OpenAI/OpenRouter und Anthropic nutzen einen gemeinsamen internen Tool-Dispatch-Pfad (z. B. `src-tauri/src/agent/tool_dispatch.rs`), damit `subagents.run` und neue Toolgruppen nicht doppelt gepflegt werden.
- `tools.rs` bekommt `registry_filtered(groups: &[ToolGroup]) -> Vec<ToolDef>` und `render_for_openai_filtered` / `render_for_anthropic_filtered`; Coordinator, Subagent-Loop und `subagents.run` waehlen den Katalog ueber explizite Gruppen.
- **Shell-Child-Registry:** `shell_exec` registriert laufende Child-Prozesse (Handle + PID) in einem session-lokalen Registry; `AgentEngineState`-Cancellation und Subagent-Abbruch rufen SIGTERM/SIGKILL (bzw. Windows `TerminateProcess`) ueber diese Registry auf.
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
  - `workspace_git_status` (Git im Workspace-Root, fuer Diff/Review ohne manuelles `cwd`)
  - `workspace_diff` (Working-tree + staged Diff relativ zum Workspace-Root)
  - `git_status` (generisches Git-Tool mit explizitem `cwd` innerhalb des Workspace; gleiche Semantik wie CLI, fuer Subagents mit `git_read`)
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
- `shell_write`: `shell_exec` mit `writes:true` (Allowlist aufgehoben; **nur Coordinator-Katalog**, nie Subagents)
- `web_read`: `web_search`, `web_fetch` (nur aktiv wenn API-Key gesetzt)
- `memory_read`, `plans_read`, `tasks_read`, `rules_skills_read`
- `memory_write`, `plans_write`, `tasks_write`, `rules_skills_write`

### Web Tools Backend und Settings

- Backend-Optionen in v1: `tavily` und `brave`.
- Settings-Schema bekommt einen `agent.web` Abschnitt: `provider: "tavily" | "brave" | "none"` plus Keyring-Status (configured/masked) fuer Tavily/Brave — keine Klartext-Keys in JSON.
- Settings UI: neuer Agent-Tab Block "Web Tools" mit Backend-Auswahl und passwordartigen Input-Feldern fuer API-Keys, plus Test-Button. i18n fuer alle Labels.
- Env-Var-Fallback: `BLX_TAVILY_API_KEY`, `BLX_BRAVE_API_KEY` werden gelesen wenn das Settings-Feld leer ist.
- Runtime entscheidet beim Toolkatalog-Bau: ist kein API-Key fuer das gewaehlte Backend verfuegbar, werden `web_search` und `web_fetch` nicht registriert; `list_skills()` setzt fuer Core-Skill `web` ein Runtime-Feld `availability: disabled_no_key` mit Hinweis auf die Settings-Position.

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
- `CORE_SKILLS` in `skills_rules/store.rs` erweitern; `list_skills()` merged fuer Core-Skill `web` Runtime-`availability` (Keyring/Env), unabhaengig vom Workspace-`enabled`-Index.
- `system_prompt.rs` Core-Skill-Liste und Tool-Index aktualisieren.

### Protocol und UI

- `AgentEvent` und `src/agent_wire.rs` um Subagent-Events erweitern:
  - `SubagentStarted`
  - `SubagentStep`
  - `SubagentToolCall`
  - `SubagentFinished`
- `src/workbench/agent_timeline.rs` um `TimelineItem::SubagentGroup` erweitern (`#[serde(default)]` / tolerant deserializing fuer bestehende `sessions.json` ohne Subagent-Eintraege).
- `friendly_label` und `summarize_args` in `agent_timeline.rs` fuer alle neuen Tool-Namen (Environment, Shell, Git, Web, Workspace-Search, Diff, `subagents.run`).
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
  - Vereinheitlichter Tool-Dispatch-Pfad: Anthropic-, OpenRouter- und OpenAI-compatible-Provider rufen denselben Dispatcher fuer alle Tools.
  - `registry_filtered` liefert nur Tools der angegebenen Gruppen; Coordinator vs Subagent-Katalog unterscheiden sich korrekt.
  - `shell_exec`/`git_*` ohne vorherigen `environment_detect`-Cache fuer den Workspace werden abgelehnt.
  - Output-Token-Cap: bei simuliertem Ueberschreiten wird `submit_result` mit `blocked` synthetisiert.
  - `subagents.run` Toolresult kuerzt findings/artifacts pro Subagent gemaess Cap; UI-Events behalten Vollstaendigkeit.
  - Mock-Provider: paralleler Lauf mit 2 Subagents terminiert beide und liefert strukturierte Ergebnisse.
  - `subagents.run` ist im Toolkatalog des Coordinators, nicht im Subagent-Katalog.
  - `environment_detect` erkennt OS, Default-Shell, verfuegbare Shells, Git-Verfuegbarkeit und Workspace Root.
  - `environment_detect` Cache wird bei Workspace-Wechsel invalidiert.
  - `shell_exec` nutzt Workspace-CWD, Timeout und Output-Limit.
  - `shell_exec` ohne `shell_write` lehnt Commands ausserhalb der Allowlist ab.
  - `shell_exec` parst `git` Subcommands und blockiert mutierende Subcommands ohne `shell_write`.
  - Shell-Child-Registry: SIGTERM/SIGKILL Cancellation wirkt auf alle registrierten laufenden Child-Prozesse.
  - Git read-only Tools verlassen den Workspace nicht und liefern begrenzte Ausgabe.
  - Mutierende Git-Tools sind ohne `git_write` nicht verfuegbar.
  - Toolgruppen filtern Shell/Git/Web/Workspace/Memory/Plan/Task Tools korrekt.
  - Rollen `scout`, `review`, `security_analyst` erzeugen passende Prompt-Fragmente.
  - Namensvergabe ist stabil und lokalisiert; bei Konflikt wird Auto-Suffix vergeben.
  - `submit_result` ist im Subagent-Toolkatalog; Subagent-Loop terminiert beim Aufruf.
  - Iteration- und Token-Cap fuehrt zu `status: "blocked"` mit Begruendung.
  - Cancellation beendet offene Subagent-Runs und deren Child-Prozesse.
  - `web_search` und `web_fetch` sind nicht im Toolkatalog wenn kein API-Key gesetzt; mit gesetztem Key sind sie verfuegbar und nutzen das gewaehlte Backend.
  - Settings-Schema validiert Web-Backend-Auswahl; API-Keys nur via Keyring + masked status in View.
  - `shell_write` erscheint nicht im Subagent-Toolkatalog.

- Frontend:
  - `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
  - Subagent Timeline Items persistieren und rehydrieren korrekt; alte `sessions.json` ohne `SubagentGroup` deserialisieren fehlerfrei.
  - `friendly_label` zeigt lokalisierte Kurzlabels fuer neue Environment/Shell/Git/Web/Diff-Tools.
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

Abhaengigkeiten: `A → B` bedeutet B startet erst nach A. Phase C (Web) ist optional parallel zu Phase B nach Phase A.

- [x] `dispatch-unify` - Anthropic, OpenRouter und OpenAI-compatible Tool-Dispatch zu einem gemeinsamen internen Dispatcher (`tool_dispatch.rs`) refaktorieren. **Blockiert:** alle weiteren Tasks.
- [x] `toolgroup-filtering` - Toolgruppenmodell, `registry_filtered`, Coordinator/Subagent-Kataloge implementieren und testen. **Abhaengig von:** `dispatch-unify`. **Blockiert:** `subagent-orchestrator`, `shell-tool-skill` (Allowlist via Gruppen).
- [x] `environment-tool-skill` - `environment_detect` Tool und `environment` Core Skill, Session-Cache, serverseitige Ablehnung von Shell/Git ohne Cache. **Abhaengig von:** `dispatch-unify`. Phase A.
- [x] `shell-tool-skill` - `shell_exec`, Shell-Child-Registry, `shell` Core Skill, Allowlist, read-only `git` via shell, SIGTERM/SIGKILL Cancellation; `shell_write` nur Coordinator. **Abhaengig von:** `dispatch-unify`, `toolgroup-filtering`, `environment-tool-skill`. Phase A.
- [x] `git-tool-skill` - Read-only und erlaubte mutierende Git Tools, `workspace_git_*` vs `git_*` Semantik, `git` Core Skill. **Abhaengig von:** `dispatch-unify`, `environment-tool-skill`. Phase A.
- [x] `workspace-search-diff` - `workspace_search`, `workspace_git_status`, `workspace_diff`. **Abhaengig von:** `dispatch-unify`. Phase A.
- [x] `subagent-orchestrator` - `subagents.run`, Parallelitaetslimit, Cancellation, Provider-Reuse, `submit_result`, Token/Iteration-Caps, Toolresult-Size-Caps. **Abhaengig von:** `toolgroup-filtering`, `environment-tool-skill`. Phase B.
- [x] `subagent-role-prompts` - Rollenprofile, i18n Displaynamen, Auto-Suffix, `subagent_system_prompt`, Coordinator-Trigger, Harness-vs-Shell in Core Skills. **Abhaengig von:** `subagent-orchestrator` (Prompt-Vertrag). Phase B.
- [x] `subagent-protocol` - Backend/Frontend Wire Types, `AgentEvent` Subagent-Events. **Abhaengig von:** `subagent-orchestrator`. **Blockiert:** `subagent-chat-ui`. Phase B.
- [x] `subagent-chat-ui` - Inline-Subcards, Live Steps/Tasks, Toolcall-Icons, 50ms Debounce, `TimelineItem::SubagentGroup` mit Serde-Migration, `friendly_label`/`summarize_args`. **Abhaengig von:** `subagent-protocol`, `i18n-subagents-tools` (Labels). Phase B.
- [x] `core-skill-index-prompt` - `CORE_SKILLS`, Core Skill Docs (inkl. Harness-vs-Shell), `list_skills` web-`availability`, `system_prompt.rs` Toolindex. **Abhaengig von:** `environment-tool-skill`, `shell-tool-skill`, `git-tool-skill`; Web-Teil optional nach `web-tools-skill`. Phase B.
- [x] `i18n-subagents-tools` - Alle neuen UI-, Rollen-, Tool-, Settings- und Disabled-State-Labels in allen Locale-Dateien. **Blockiert:** `subagent-chat-ui`, `verification`. Phase B.
- [x] `settings-web-api-keys` - Settings `agent.web`, Keyring fuer Tavily/Brave, Agent-Tab UI, Env-Fallback, i18n. Phase C; **blockiert:** `web-tools-skill`.
- [x] `web-tools-skill` - `web_search`, `web_fetch`, `web` Core Skill, `availability: disabled_no_key` in `list_skills`. **Abhaengig von:** `settings-web-api-keys`, `toolgroup-filtering`. Phase C.
- [x] `verification` - Rust Tests, Frontend Check, Backend Check, manuelle Chat-View-Pruefung. **Abhaengig von:** allen Phase-B-Tasks; Web-Checks wenn Phase C erledigt.
