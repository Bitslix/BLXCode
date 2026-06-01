# AI-gestützte Plan- & Task-Generierung

Zwei Buttons im Plans-Panel ("AI Plan" / "AI Tasks") öffnen einen Dialog mit
Prompt-Textbox. Der konfigurierte Provider/Model aus den BLXCode-Settings
generiert daraus einen Markdown-Plan, der live (mit Loading-Animation) als
scrollbare Preview in der Textbox erscheint. Ein Toggle entscheidet, ob zum
Plan zusätzlich Tasks angelegt werden. Speichern persistiert Plan (und ggf.
Tasks) nach `.agents/plans` über die bestehenden `plan_*`-Tools.

## Kontext & Designprinzipien

- **Provider-Reuse, kein neuer Stack**: Die Generierung läuft über
  [`oneshot::complete_text`](../../src-tauri/src/agent/oneshot.rs) — exakt das
  Muster, das [`git_commit_ai.rs`](../../src-tauri/src/git_commit_ai.rs) bereits
  für AI-Commit-Messages nutzt (non-streaming, ein Request, wiederverwendet
  `AgentProviderSettings` + `provider_key_pub`). Kein zweiter LLM-Pfad, keine
  Chat-Conversation, keine Events.
- **Einheitlichkeit mit dem Plan-Skill**: Der System-Prompt zwingt das Modell
  in das Format des built-in Plan-Skills
  ([`harness_skills/plans.md`](../../src-tauri/src/agent/harness_skills/plans.md)):
  `# Titel`, Prosa-Sektionen und eine `## Tasks`-Sektion mit der exakten
  Task-Line-Syntax `- [ ] \`task-id\` - Titel`. Dadurch ist die Ausgabe mit
  manuell erstellten Plänen und mit `plan_load` / `plan_sync_from_tasks`
  kompatibel.
- **Persistenz nur über bestehende Tools**: Gespeichert wird mit dem schon
  vorhandenen `plan_create`; Tasks werden — wenn der Toggle aktiv ist — durch
  `plan_load` aus der `## Tasks`-Sektion in den Task-Manager synchronisiert. Es
  wird KEIN neuer Schreibpfad nach `.agents/plans` gebaut.
- **Regelkonform**: Eigener Komponenten-Subfolder + eigene CSS-Datei
  (`rule-reusable-components`), keine Monolith-Erweiterung von
  [`plans_panel/mod.rs`](../../src/workbench/plans_panel/mod.rs)
  (`rule-no-monolith-structure`), ausschließlich `var(--token)` aus
  [`tokens.css`](../../themes/tokens.css) (`rule-theme-tokens`).

## Architektur

```
Frontend (blxcode-ui)                     Backend (blxcode)
─────────────────────                     ─────────────────
plans_panel/header
  ├─ Button "AI Plan"   ─┐
  └─ Button "AI Tasks"  ─┤ open(mode)
                          ▼
plans_panel/ai_generate_dialog/   invoke   plan_ai.rs
  ├─ mod.rs   ───────────────────────────► plan_generate_ai {prompt, with_tasks}
  │   textarea + toggle + loading            └─ load_settings_pub + provider_key_pub
  │                                          └─ oneshot::complete_text(SYSTEM_PROMPT…)
  │   on save:                               └─ parse → GeneratedPlan{title, markdown}
  │     plan_create(path, markdown) ◄────────────── (bestehender Command)
  │     if with_tasks: plan_load(path) ◄──────────── (bestehender Command, Task-Sync)
  └─ ai-generate-dialog.css
```

## Backend

### B1 — Neues Modul `src-tauri/src/agent/plan_ai.rs`

Spiegelt `git_commit_ai.rs`. Kein Streaming, keine Conversation.

- Zwei System-Prompt-Konstanten (oder ein Prompt mit Mode-Schalter):
  - **Plan-only** (`with_tasks = false`): erzeugt `# Titel` + Prosa-Sektionen
    (`## Ziel`, `## Schritte`/`## Vorgehen`) und eine leere, aber vorhandene
    `## Tasks`-Sektion (für späteres manuelles/agentengetriebenes Befüllen).
  - **Plan + Tasks** (`with_tasks = true`): zusätzlich eine befüllte
    `## Tasks`-Sektion mit konkreten Task-Lines im Skill-Format:
    `- [ ] \`kebab-id\` - Imperativer Titel`. Task-IDs kurz, eindeutig,
    kebab-case.
- Prompt-Härtung (analog `clean_message`): Antwort soll **reines Markdown**
  sein, keine ```` ```fences ```` ums Gesamtdokument, kein Vorspann/Kommentar.
- Die Task-Line-Syntax wird wörtlich im Prompt vorgegeben (aus `plans.md`):
  `- [ ]` pending · `- [>]` in-progress · `- [!]` blocked · `- [x]` done ·
  `- [-]` cancelled. Generiert wird ausschließlich `- [ ]`.
- `MAX_TOKENS` großzügiger als beim Commit (z. B. 2048), `REQUEST_TIMEOUT`
  ggf. höher (Plan ist länger als eine Commit-Message).

Signatur:

```rust
#[derive(Serialize)]
pub struct GeneratedPlan { pub title: String, pub markdown: String }

#[tauri::command]
pub async fn plan_generate_ai(
    app: AppHandle,
    prompt: String,
    with_tasks: bool,
) -> Result<GeneratedPlan, String>
```

- Lädt `load_settings_pub(&app)` + `provider_key_pub(&app, settings.provider)`;
  leerer Key → freundlicher Fehler ("no API key configured for {provider}",
  identisch zu `git_commit_ai`).
- Ruft `oneshot::complete_text(&settings, &api_key, system, &prompt, MAX_TOKENS)`.
- Bereinigt die Ausgabe (Fences/Whitespace), extrahiert den Titel aus der
  ersten `# `-Zeile (Fallback: aus dem Prompt abgeleitet). Stellt sicher, dass
  eine `## Tasks`-Sektion existiert (anhängen, falls das Modell sie vergisst —
  damit `plan_load` nie leer läuft).

### B2 — Command registrieren

`plan_generate_ai` in [`lib.rs`](../../src-tauri/src/lib.rs) zur
`invoke_handler!`-Liste hinzufügen (neben den bestehenden `plan_*`/`git_*`).

### B3 — Modul einhängen

`pub mod plan_ai;` in [`src-tauri/src/agent/mod.rs`](../../src-tauri/src/agent/mod.rs).

### B4 — Unit-Tests

In `plan_ai.rs` (wie `git_commit_ai`): `clean_markdown` strippt Gesamt-Fences;
`extract_title` zieht den `# `-Titel; `ensure_tasks_section` hängt fehlende
`## Tasks` an. Reiner String-Logik-Test, kein Netz.

## Frontend

### F1 — Tauri-Bridge-Wrapper

In [`tauri_bridge.rs`](../../src/tauri_bridge.rs) analog
`git_generate_commit_message`:

```rust
pub async fn plan_generate_ai(prompt: String, with_tasks: bool)
    -> Result<GeneratedPlan, String>
```

mit lokaler `GeneratedPlan { title, markdown }`-Deserialisierung.

### F2 — Neue Komponente `src/workbench/plans_panel/ai_generate_dialog/`

Eigener Subfolder mit `mod.rs` + `ai-generate-dialog.css` (Komponentenregel).

Props:
- `open: RwSignal<bool>`
- `mode: Signal<AiGenMode>` (`Plan` | `Tasks`)
- `on_saved: Callback<()>` — damit das Panel die Plan-Liste neu lädt.
- benötigter Kontext: `WorkbenchContext`/`workspace_cwd`, `I18nService`, Toast.

State (wie `commit_dialog`):
- `prompt: RwSignal<String>` — Eingabe des Users.
- `result: RwSignal<Option<GeneratedPlan>>` — generiertes Markdown (Preview).
- `with_tasks: RwSignal<bool>` — Toggle; im `Tasks`-Mode default `true`.
- `generating` / `saving: RwSignal<bool>`.
- Effect: bei `open` → State zurücksetzen; `with_tasks` aus `mode` vorbelegen.

UI-Fluss:
1. **Eingabephase**: scrollbare `<textarea>` für den Prompt
   (Placeholder z. B. "Beschreibe was der Plan abdecken soll…").
2. **Toggle/Switch** "Tasks zu diesem Plan anlegen" — bestehende
   Switch-Komponente wiederverwenden (siehe
   [`workspace_settings_pane`](../../src/workbench/workspace_settings_pane/)
   nutzt Toggles); im `Tasks`-Mode disabled+on.
3. **Generate-Button** → `generating=true`, `plan_generate_ai(prompt, with_tasks)`.
   Während `generating`: Textbox bekommt eine **Loading-Animation**
   (Shimmer-Overlay, CSS) und ist gesperrt.
4. **Ergebnis**: `result` füllt die Textbox als **scrollbare Markdown-Preview**
   (read-only oder leicht editierbar). Aktionen: **Speichern**,
   **Neu generieren** (zurück zu Schritt 3 mit gleichem Prompt), **Abbrechen**.
5. **Speichern** → `saving=true`:
   - `path = next_plan_name(result.title, existing_plans)` (Helper aus
     `plans_panel` wiederverwenden — ggf. `pub(crate)` machen).
   - `plan_create(ws, path, Some(result.markdown))`.
   - falls `with_tasks`: anschließend `plan_load(ws, path)` → Task-Sync.
   - Erfolg: Dialog schließen, `on_saved` feuern (Liste neu laden), Erfolgs-Toast.
   - Fehler: Inline-Error + Error-Toast (Strings analog `commit_dialog`).

### F3 — Buttons im Plans-Panel-Header

In [`plans_panel/mod.rs`](../../src/workbench/plans_panel/mod.rs) `header` →
`blx-sr-pane__actions`: zwei Buttons **vor** dem bestehenden `+`-Button:
- "AI Plan" — Icon `LuSparkles`, öffnet Dialog mit `mode = Plan`.
- "AI Tasks" — Icon `LuListPlus`, öffnet Dialog mit `mode = Tasks`.

Dialog-Komponente einmal am Panel-Ende rendern, gesteuert über
`ai_dialog_open` + `ai_dialog_mode`-Signale. `on_saved` ruft
`load_plans_list(state, ws)`.

### F4 — CSS `ai-generate-dialog.css`

- Nur `var(--token)`, keine Literale (Theme-Regel).
- Loading-Shimmer: `@keyframes` mit Token-basiertem Gradient über der Textbox;
  reduziert bei `prefers-reduced-motion`.
- Scrollbare Preview: `overflow:auto`, feste max-height, Monospace für Markdown.
- CSS über Projekt-Standard einhängen (Trunk-`<link>` in `index.html` bzw.
  zentrale `@import`, wie die anderen Komponenten-CSS).

### F5 — i18n-Keys

Neue `I18nKey`-Varianten in [`keys.rs`](../../src/i18n/keys.rs) **und** in
**jeder** `locales/*.rs` (Compile-Time-Exhaustiveness):
`PlansAiPlanBtn`, `PlansAiTasksBtn`, `PlansAiDialogTitlePlan`,
`PlansAiDialogTitleTasks`, `PlansAiPromptPh`, `PlansAiWithTasksToggle`,
`PlansAiGenerate`, `PlansAiGenerating`, `PlansAiRegenerate`, `PlansAiSave`,
`PlansAiPreviewHint`, `PlansAiFailed`, `PlansAiEmptyPrompt`, `PlansAiSaved`.
Erst `en_us.rs` pflegen, dann
[`scripts/render_i18n_locales_from_en.py`](../../scripts/render_i18n_locales_from_en.py)
für die übrigen Sprachen laufen lassen.

## Reuse-Notizen

- `oneshot::complete_text` — fertig, kein neuer Provider-Code.
- `plan_create` / `plan_load` — fertige Persistenz + Task-Sync.
- `next_plan_name` / `slugify_title` / `normalize_plan_content` aus
  `plans_panel/mod.rs` — wiederverwenden statt duplizieren (ggf. Sichtbarkeit
  anheben).
- Loading-/Busy-Pattern aus
  [`commit_dialog/mod.rs`](../../src/workbench/commit_dialog/mod.rs) als Vorlage.
- Switch/Toggle aus `workspace_settings_pane` wiederverwenden.

## Risiken & Entscheidungen

- **Leerer/fehlender Provider-Key**: gleicher freundlicher Fehler wie bei
  AI-Commit; Buttons bleiben sichtbar, Fehler erscheint erst beim Generieren.
- **Modell ignoriert das Format**: Backend erzwingt `# Titel` + `## Tasks`
  per Post-Processing (`ensure_tasks_section`), damit `plan_load` nie leer
  läuft.
- **Lange Pläne / Timeout**: `MAX_TOKENS`/Timeout in `plan_ai.rs` höher als
  beim Commit; bei Abbruch sauberer Fehler-Toast, Prompt bleibt erhalten.
- **Tasks-Mode vs. Toggle**: Im `Tasks`-Mode ist der Toggle erzwungen `on`
  (und disabled), damit die beiden Buttons eindeutige Erwartungen erfüllen.

## Tasks

- [ ] `plan-ai-backend` - plan_ai.rs Modul mit plan_generate_ai (oneshot reuse, Skill-konformer System-Prompt, Post-Processing)
- [ ] `plan-ai-register` - Command in lib.rs registrieren und Modul in agent/mod.rs einhängen
- [ ] `plan-ai-backend-tests` - Unit-Tests für clean_markdown / extract_title / ensure_tasks_section
- [ ] `plan-ai-bridge` - tauri_bridge::plan_generate_ai Wrapper + GeneratedPlan-Typ
- [ ] `plan-ai-dialog` - ai_generate_dialog Komponente (mod.rs): Prompt, Toggle, Loading, scrollbare Preview, Save→plan_create(+plan_load)
- [ ] `plan-ai-dialog-css` - ai-generate-dialog.css mit Shimmer-Loading, nur Theme-Tokens, reduced-motion
- [ ] `plan-ai-buttons` - Zwei Header-Buttons (AI Plan / AI Tasks) im Plans-Panel + Dialog-Verdrahtung, on_saved reload
- [ ] `plan-ai-i18n` - I18nKeys in keys.rs + en_us.rs, danach render-Skript für übrige Locales
- [ ] `plan-ai-verify` - cargo check (wasm + tauri) und manueller Smoke-Test des Generier-/Speicher-Flows
