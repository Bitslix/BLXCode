# Skills & Rules Tabs für das RightPanel

**Status:** done

## Summary

Zwei neue Tabs im `RightPanel` (neben **Agent / Browser / Memory**):

- **Rules** — listet `.agents/rules/*.md` als Cards mit Detailtext und Aktivierungs-Toggle.
- **Skills** — listet `.agents/skills/<name>/SKILL.md` als Cards mit Detailtext, Toggle und einem **Install**-Button für neue Skills.

Der BLXCode Agent bekommt einen vollen Toolcall-Satz zum Lesen, Erstellen/Updaten, Aktivieren/Deaktivieren, Entfernen und Installieren von Skills/Rules. Aktivierungs-Status wird in zwei Manifest-Dateien gepflegt:

- `.agents/skills/index.json`
- `.agents/rules/index.json`

## Storage Layout

### `.agents/rules/index.json`

```json
{
  "version": 1,
  "rules": {
    "rule-no-monolith-structure.md": { "enabled": true,  "updated_at": "2026-05-20T11:00:00Z" },
    "rule-reusable-components.md":   { "enabled": false, "updated_at": "2026-05-20T11:00:00Z" }
  }
}
```

- Key = Dateiname relativ zu `.agents/rules/`.
- Fehlt eine Datei im Index: gilt als **enabled** (Default-Aktiv für bestehende Rules).
- Steht eine Datei im Index, aber nicht mehr auf der Platte: Eintrag wird beim nächsten Listing automatisch entfernt (Self-Heal).

### `.agents/skills/index.json`

```json
{
  "version": 1,
  "skills": {
    "leptos-guide": {
      "enabled": true,
      "source": { "kind": "git", "url": "https://github.com/...", "ref": "main" },
      "installed_at": "2026-05-20T11:00:00Z",
      "updated_at":   "2026-05-20T11:00:00Z"
    },
    "create-agentsmd": {
      "enabled": true,
      "source": { "kind": "local" },
      "installed_at": "2026-05-20T11:00:00Z",
      "updated_at":   "2026-05-20T11:00:00Z"
    }
  }
}
```

- Key = Ordnername unter `.agents/skills/`.
- Skills ohne `SKILL.md` werden zwar geduldet, aber in der UI mit Warn-Badge markiert.
- `source.kind`: `"git" | "npm" | "local" | "agent-created"`.

### Sandbox / Workspace-Grenze

Alle Pfade laufen über `WorkspaceRootGuard` (siehe `src-tauri/src/agent/tools.rs`). Schreib-Operationen sind hart auf `.agents/skills/**` bzw. `.agents/rules/**` unter dem aktiven Workspace-Root beschränkt; `..` und absolute Pfade werden verworfen.

## Backend (`src-tauri/`)

### Neues Modul `agent/skills_rules.rs`

Reine Logik, kein Tauri-Code direkt darin (Trennung lt. `rule-no-monolith-structure`):

- `RulesStore`, `SkillsStore` mit:
  - `list() -> Vec<RuleEntry | SkillEntry>` (merge Disk-State + Index, repariert verwaiste Index-Einträge).
  - `read(name) -> String` (lädt Markdown).
  - `write(name, content)` (anlegen oder überschreiben).
  - `remove(name)` (Datei/Ordner löschen + Index bereinigen).
  - `set_enabled(name, enabled)`.
  - `install_skill(name, source: SkillSource)` für `git` / `npm` / `local`.

### Install-Skript (fix verdrahtet, kein Free-Form-Shell)

- `SkillSource::Git { url, ref? }`:
  - `git clone --depth=1` in temporäres Verzeichnis, dann atomar nach `.agents/skills/<name>/` verschieben.
  - Verpflichtende Validierung: das gezogene Repo enthält `SKILL.md` auf Top-Level. Sonst → Abbruch, kein Move.
- `SkillSource::Npm { package, version? }`:
  - `npm pack <pkg>@<ver>` in Temp-Dir, dann tarball auspacken, falls `SKILL.md` enthalten → Move.
  - Wird nur ausgeführt, wenn `npm` im PATH ist; sonst sauberer Fehler.
- `SkillSource::Local { path }`:
  - Verzeichnis liegt schon irgendwo unter `WORKSPACE/`, wird nach `.agents/skills/<name>/` kopiert.
- Kein Pfad ruft jemals shell-evaluierte User-Strings; Args werden direkt an `std::process::Command` übergeben.
- Konfigurierbares Timeout (z. B. 90 s) + Abbruch + Aufräumen des Temp-Verzeichnisses bei Fehler.

### Tauri-Commands (in `src-tauri/src/commands.rs` registriert in `lib.rs`)

```rust
// Reads
skills_list() -> Vec<SkillEntry>
skills_read(name) -> String
rules_list() -> Vec<RuleEntry>
rules_read(name) -> String

// Mutations
skills_write(name, content) -> SkillEntry
rules_write(name, content) -> RuleEntry
skills_set_enabled(name, enabled) -> SkillEntry
rules_set_enabled(name, enabled) -> RuleEntry
skills_remove(name) -> ()
rules_remove(name) -> ()

// Install
skills_install(name, source: SkillSource) -> SkillEntry
```

Alle Commands sind synchron mit `Mutex`-Schutz pro Manifest, damit konkurrierende Aufrufe sich nicht überschreiben (analog `AgentEngineState`).

### Agent-Toolcalls (server-side)

Erweiterung von `agent/tools.rs` (bzw. neue Geschwister-Datei, falls sonst zu groß) um:

| Tool | Beschreibung |
|------|--------------|
| `rules_list` | gibt JSON-Array mit `{name, enabled, summary, size, updated_at}` |
| `rules_read {name}` | Markdown-Body (4 KiB truncate, gleiches Verhalten wie `read_workspace_file`) |
| `rules_write {name, content}` | erzeugt oder überschreibt `rule-*.md`; verlangt Präfix `rule-` und `.md`-Endung |
| `rules_set_enabled {name, enabled}` | Schalter |
| `rules_remove {name}` | löscht Datei + Index-Eintrag |
| `skills_list` | wie oben, plus `source`, `installed_at` |
| `skills_read {name}` | liest `SKILL.md` |
| `skills_write {name, content}` | erzeugt/überschreibt `SKILL.md` im Ordner (`source = "agent-created"`) |
| `skills_set_enabled {name, enabled}` | Schalter |
| `skills_remove {name}` | löscht Ordner inkl. Inhalt + Index-Eintrag |
| `skills_install {name, source: {kind, url?, ref?, package?, version?, path?}}` | s. Install-Skript |

JSON-Schemas werden im selben Stil wie `memory_*` Tools an die LLM-Requests gehängt; Implementierung dispatched in `session_orchestrator.rs`/Provider-Pfaden.

### System-Prompt (`agent/system_prompt.rs`)

Neuer Abschnitt nach `Workspace memory`:

```
## Workspace skills & rules (server-side)
Two roots under `.agents/`:
- `rules/` — Markdown rules the user wants this agent to respect. Active rules
  in `rules/index.json` MUST shape your behaviour for the current workspace.
- `skills/<name>/SKILL.md` — additional capabilities/instructions the user has
  installed. Treat enabled skills the same as rules: read them when relevant.

Tools:
- `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove`
- `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`,
  `skills_remove`, `skills_install { name, source }`

Behaviour:
- On the first turn of a session (or when the user changes the workspace),
  call `rules_list` and `skills_list`; read the enabled entries that look
  relevant to the request before acting.
- Treat disabled entries as if they did not exist — do not apply them.
- Use `skills_install` only when the user explicitly asks to install a skill,
  and confirm `name + source` back to the user in your final reply.
- Never store secrets in rule or skill files.
```

Der bestehende Sicherheits-Block bleibt unverändert; nur der Tool-Katalog wächst.

## Shared Wire Types (`src/agent_wire.rs` + `src-tauri/src/agent/protocol.rs`)

Neu, in beiden Crates spiegelgleich:

```rust
pub struct RuleEntry {
    pub name: String,        // dateiname
    pub title: String,       // erste H1 oder dateiname
    pub summary: String,     // erste 200 Zeichen ohne H1
    pub enabled: bool,
    pub size_bytes: u64,
    pub updated_at: String,  // RFC3339
}

pub struct SkillEntry {
    pub name: String,        // ordnername
    pub title: String,
    pub summary: String,
    pub enabled: bool,
    pub source: SkillSourceMeta, // {kind, url?, ref?, package?, version?}
    pub installed_at: String,
    pub updated_at: String,
}
```

## Frontend (`src/`)

### Tauri-Bridge (`src/tauri_bridge.rs`)

Wrapper für alle elf neuen Commands, jeweils `async fn`, gleiches Pattern wie die bestehenden `memory_*` Wrapper.

### State (`src/workbench/state.rs`)

- `RightPanelTab` Enum um `Skills` und `Rules` erweitern.
- Neuer `WorkbenchService` State:
  - `rules: RwSignal<Vec<RuleEntry>>`, `skills: RwSignal<Vec<SkillEntry>>`
  - `rules_loading`, `skills_loading`, `rules_error`, `skills_error`
- Aktionen: `refresh_rules`, `refresh_skills`, `set_rule_enabled`, `set_skill_enabled`, `remove_rule`, `remove_skill`, `install_skill`.

### Neuer Subfolder `src/workbench/skills_rules_panel/`

Pflicht laut `rule-no-monolith-structure` + `rule-reusable-components`:

```
skills_rules_panel/
├── mod.rs              -- pub use Komponenten
├── rule_card.rs        -- <RuleCard entry=... />
├── skill_card.rs       -- <SkillCard entry=... />
├── rules_tab.rs        -- <RulesTabDock />
├── skills_tab.rs       -- <SkillsTabDock />  inkl. Install-Button
├── install_dialog.rs   -- <SkillInstallDialog />
└── styles.css          -- gekapseltes Styling
```

#### Card-Layout

Pro Eintrag:

- Header-Zeile: Titel + kleines Status-Pill (`enabled`/`disabled`) + Quelle-Badge (nur Skills, z. B. `git`/`npm`/`local`).
- Subtext: `summary` (max 2 Zeilen, `text-overflow: ellipsis`).
- Footer: Toggle (re-uses bestehende Toggle-Komponente), `Read`-Button (öffnet Markdown im Workspace-Editor oder Modal), `Remove`-Button (mit Confirm).
- Visuelle Abgrenzung zwischen `enabled` und `disabled` (Opacity + Akzent-Border).

#### Install-Dialog (`<SkillInstallDialog />`)

Felder:

- `name` (Pflicht, regex `^[a-z0-9][a-z0-9-_]{1,40}$`).
- `source.kind` Select: `git` (Default) / `npm` / `local`.
- Quellen-Felder je nach Auswahl:
  - **git**: `url`, optional `ref` (default `main`).
  - **npm**: `package`, optional `version`.
  - **local**: `path` relativ zum Workspace (FilePicker oder Text).
- Submit ruft `skills_install`, zeigt Spinner/Progress, bei Erfolg Toggle direkt auf `enabled = true` setzen.
- Fehler-Anzeige im Dialog, kein Auto-Close bei Fehler.

#### Right-Rail Icons

- Rules: `LuShield` oder `LuListChecks`
- Skills: `LuPuzzle` oder `LuPlug`

### `right_panel.rs`

Zwei zusätzliche Tab-Buttons in beiden Tab-Strips (collapsed Rail + offener Tab-Strip), plus zwei zusätzliche `workbench-right-tab-panel`-Container mit `<RulesTabDock/>` und `<SkillsTabDock/>`. Sonst keine Refactor-Welle – minimaler Eingriff.

### i18n

Neue `I18nKey`-Varianten (in `src/i18n/keys.rs` o.ä.):

- `TabRules`, `TabSkills`
- `RulesEmpty`, `SkillsEmpty`
- `SkillInstallTitle`, `SkillInstallNameLabel`, `SkillInstallSourceLabel`, `SkillInstallSubmit`, `SkillInstallCancel`
- `CardEnable`, `CardDisable`, `CardRead`, `CardRemove`, `CardConfirmRemove`

Für **jede** Sprache (`src/i18n/locales/*.rs`) Einträge ergänzen — Compile-Time-Exhaustiveness greift sonst. Für die Nicht-EN-Locales nach EN-Bearbeitung `scripts/render_i18n_locales_from_en.py` (Default-Modus, nur fehlende Keys) laufen lassen.

## Tests

- Unit-Tests in `src-tauri/src/agent/skills_rules.rs`:
  - Index Self-Heal (verwaiste Einträge → entfernt).
  - Default-Enabled bei fehlendem Index-Eintrag.
  - `set_enabled` ist idempotent.
  - `install_skill` (git) lehnt fehlende `SKILL.md` ab und räumt Temp-Dir auf.
  - Pfad-Sandbox: `name = "../foo"` wird abgelehnt.
- Integration-Test für die Tauri-Commands über ein temp `WorkspaceRootGuard`.

## Migration / Bootstrap

Beim Öffnen eines Workspace:

- Falls `.agents/rules/` existiert, aber `index.json` fehlt → einmalig erzeugen, alle vorhandenen Rules `enabled: true`.
- Falls `.agents/skills/` existiert, aber `index.json` fehlt → erzeugen, alle vorhandenen Skill-Ordner als `source: "local"`, `enabled: true`.
- Kein Auto-Migrieren / Verschieben existierender Dateien.

## Out of Scope

- Versionierung/Update existierender Skills (`skills_update`) — wird später ergänzt.
- Marketplace/Discovery von Skills.
- Sandboxing der Skill-Markdowns gegenüber dem Agent (Skills werden vom Agent gelesen wie reguläre Workspace-Dateien).
- Frontmatter-basierte Skill-Metadaten — bleibt zunächst index-basiert.

## Reihenfolge der Umsetzung

1. Wire-Types (Protocol + agent_wire) + Index-Schema-Tests.
2. Backend-Modul `skills_rules.rs` + Tauri-Commands + Bootstrap.
3. Agent-Toolcalls im jeweiligen Provider-Pfad + System-Prompt-Update.
4. Frontend State + Bridge + i18n-Keys (inkl. Locale-Render-Skript).
5. Komponenten-Subfolder `skills_rules_panel/` inkl. Cards + Install-Dialog.
6. `RightPanel` verdrahten, neue Icons, manuelles Dev-Smoke (Toggle, Remove, Install git).
7. Tests grün; `cargo check` für beide Crates.
