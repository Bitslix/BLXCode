# Agent Mermaid Diagrams

## Summary

Der BLXCode-Agent bekommt die Fähigkeit, eigenständig **Mermaid-Diagramme** zu
erzeugen — für Pläne, Tasks, User-Erklärungen (Responses) oder auf direkten
User-Wunsch. Plan-/Task-bezogene Diagramme werden **mit dem Plan** unter
`.agents/plans/<slug>/diagrams/` versioniert gespeichert. Im Agent-Tab werden
einzelne Diagramme **inline** in der Toolcall-Card gerendert; mehrere Diagramme
werden als **aufklappbare Tree-Gruppe** (wie Toolcalls) angezeigt und öffnen per
Klick einen **zentrierten Gallery-Tab** (oben Thumbnail-Slider, unten das aktive
Diagramm groß). Diagramme lassen sich als **.md** und **.pdf** (Orientierung
automatisch aus dem gerenderten SVG) per nativem „Save As“-Dialog exportieren.

Mermaid-Rendering existiert bereits für die File-Preview
([mermaid_glue.rs](../../../src/workbench/file_preview/mermaid_glue.rs),
[mermaid_view.rs](../../../src/workbench/file_preview/mermaid_view.rs),
`FileKind::Mermaid`) — die SVG-Render-Pipeline wird wiederverwendet, nicht neu
gebaut.

## Decisions

Alle aus dem grill-me-Interview (2026-06-03):

- **Generierung „Both":** Primär ein **strukturiertes Agent-Tool**
  (`mermaid_create` / `mermaid_create_many`) mit `{title, code, kind, plan_slug?,
  task_id?}`; zusätzlich **Fence-Detection** als Fallback für ```mermaid-Blöcke,
  die der Agent ohne Toolaufruf in den Chat-Markdown schreibt.
- **Storage:** `.agents/plans/<slug>/diagrams/<id>.mmd` + `diagrams.json`
  Manifest (id, title, kind, linked `task_id`, created). Reist im Git mit dem
  Plan, einfache Bereinigung beim Löschen des Plans.
- **Multi-Gen UX „Role-dependent":** Rollen **architect/coordinator**
  auto-generieren ein sinnvolles Default-Set ohne Rückfrage nach Anzahl/Typ;
  **Plain-Chat** fragt immer zuerst (Anzahl + Typen) via `ask_user_card`.
- **Default-Count (Setting):** Workspace-Setting `default_diagram_count` legt
  fest, wie viele Diagramme die Rollen-Autogen erzeugt bzw. welche Anzahl im
  Plain-Chat `ask_user_card` vorausgewählt ist. Standardwert `1`; der Agent darf
  bei klarem Bedarf abweichen, nutzt den Wert aber als Default.
- **Cost-Gate:** Workspace-Setting `auto-generate diagrams for plans/tasks`
  (`off` / `ask` / `on`). Bei `ask`: **einmalige** Token-Kosten-Bestätigung pro
  Session mit Schätzung; architect/coordinator respektieren das Setting.
- **PDF-Export:** Frontend liefert das **gerenderte SVG** (mermaid.js); ein
  Tauri-Command bettet es orientierungsgerecht in eine PDF-Seite ein
  (`svg2pdf` / `printpdf`). Vollständig offline, kein Headless-Browser.
- **Orientierung:** **Automatisch aus dem gerenderten SVG** (width/height) zum
  Export-Zeitpunkt abgeleitet — kein Tool-Parameter, keine Metadaten nötig.
- **Chat-Rendering (Default + Setting):**
  - Default: **1 Diagramm = inline** in der Toolcall-Card (Preview + Download);
    **2+ = collapsed Tree-Gruppe** (Titel-Liste), Klick → Gallery-Tab.
    Fence-detektierte Diagramme rendern inline.
  - Zusätzliche Option in **BLXCode Settings → Workspace-Tab**: „Always gallery"
    — auch einzelne Diagramme öffnen/zeigen über den Gallery-Tab.
- **Plan-Sicht:** Plans-Panel zeigt pro Plan ein Diagramm-Badge/Count + Button,
  der denselben Gallery-Tab **gefiltert auf den Plan** öffnet.
- **Export-Ziel:** **tauri-plugin-dialog** hinzufügen → natives „Save As" für
  `.md`/`.pdf`.

## Architecture Notes

- **Backend-Modul (neu):** `src-tauri/src/agent/mermaid/` (kein Monolith, eigenes
  Modul):
  - `mod.rs` — Re-Exports + Tauri-Commands-Registrierung in
    [lib.rs](../../../src-tauri/src/lib.rs).
  - `store.rs` — Persistenz unter `.agents/plans/<slug>/diagrams/`, `diagrams.json`
    Manifest lesen/schreiben, Bereinigung bei Plan-Löschung (Hook in
    [plans.rs](../../../src-tauri/src/plans.rs)).
  - `tool.rs` — Agent-Tool-Definitionen `mermaid_create` /
    `mermaid_create_many`; eingebunden in
    [tool_groups.rs](../../../src-tauri/src/agent/tool_groups.rs) +
    [tool_dispatch.rs](../../../src-tauri/src/agent/tool_dispatch.rs).
  - `export.rs` — `mermaid_export_md` + `mermaid_export_pdf` Commands
    (SVG→PDF via `svg2pdf`/`printpdf`, Orientierung aus SVG-Maßen), Save-As via
    tauri-plugin-dialog.
- **Protocol:** `AgentEvent`-Erweiterung bzw. ToolResult-Payload für
  Diagram-Metadaten in [protocol.rs](../../../src-tauri/src/agent/protocol.rs);
  Spiegelung in [agent_wire.rs](../../../src/agent_wire.rs).
- **Frontend-Komponenten (je eigener Ordner + CSS, Theme-Tokens):**
  - `src/workbench/agent_panel/diagram_card/` — inline Toolcall-Card mit
    SVG-Preview + Download-Button (reuse `mermaid_glue`).
  - `src/workbench/agent_panel/diagram_group/` — aufklappbare Tree-Gruppe für
    2+ Diagramme (Muster aus
    [tool_group/mod.rs](../../../src/workbench/agent_panel/tool_group/mod.rs)).
  - `src/workbench/diagram_gallery/` — zentrierter Tab: Thumbnail-Slider oben,
    aktives Diagramm groß unten. Neuer `CenterTabKind::DiagramGallery { scope }`
    in [state.rs](../../../src/workbench/state.rs) +
    [workspace_panel.rs](../../../src/workbench/workspace_panel.rs).
  - `src/workbench/file_preview/mermaid_glue.rs` — ggf. `render_mermaid_to_svg`
    extrahieren, damit Card/Gallery/Export dieselbe Funktion nutzen.
- **Settings:** neue Workspace-Settings (`diagram_render_mode`,
  `auto_generate_plan_diagrams`, `default_diagram_count`) in
  [workspace_settings_pane/mod.rs](../../../src/workbench/workspace_settings_pane/mod.rs)
  + [agent_settings.rs](../../../src-tauri/src/agent_settings.rs).
- **System-Prompt:** Diagramm-Fähigkeit + Rollen-Verhalten in
  [system_prompt.rs](../../../src-tauri/src/agent/system_prompt.rs) und den
  Rollen-Skills (architect/coordinator) dokumentieren.
- **i18n:** Jeder neue UI-String braucht einen `I18nKey` in
  [keys.rs](../../../src/i18n/keys.rs) **und** Einträge in **allen**
  `src/i18n/locales/*.rs` (Compile-Exhaustiveness). Non-EN via
  `scripts/render_i18n_locales_from_en.py` füllen.
- **Theme:** Card/Gallery/Slider ausschließlich `var(--token)` aus
  `themes/tokens.css`; SVG-Hintergrund/Farben respektieren aktives Theme
  (`ThemeService`, `blxcode-theme-changed`).

## Open / Verify during implementation

- Crate-Wahl für SVG→PDF (`svg2pdf` + `printpdf` vs. `resvg`+`pdf`) — beim
  Phaseneinstieg evaluieren (Lizenz, WASM-Irrelevanz, Offline).
- Token-Schätzung für Cost-Gate: grobe Heuristik (Prompt-Länge × Faktor) genügt;
  exakte Pricing-Logik aus [pricing.rs](../../../src-tauri/src/agent/pricing.rs)
  wiederverwenden falls vorhanden.
- Security: Diagramm-Code/SVG sanitisieren vor `inner_html` (gleicher Pfad wie
  File-Preview-Sanitizer) — siehe `security-hardening`-Plan.

## Tasks

- [x] `mermaid-store` - Backend-Store + diagrams.json-Manifest unter plans/<slug>/diagrams/ (mit Tests)
- [x] `mermaid-tool` - Agent-Tools mermaid_create / mermaid_create_many + Toolgroup/Dispatch
- [x] `mermaid-protocol` - Statt Protocol-Enum: Tool gibt JSON `{diagrams:[...]}` zurück (Frontend keyt auf Tool-Name); DiagramRecord in tauri_bridge gespiegelt
- [x] `mermaid-glue-extract` - mermaid_glue als pub(crate) + reusable DiagramRender-Komponente (rendert Inline-Code, gibt SVG zurück)
- [ ] `diagram-card` - Inline-Toolcall-Card (SVG-Preview + Download) mit eigenem CSS — OFFEN (Timeline-Integration)
- [ ] `diagram-group` - Aufklappbare Tree-Gruppe für 2+ Diagramme — OFFEN (Timeline-Integration); GalleryScope::Inline vorbereitet
- [x] `diagram-gallery-tab` - CenterTabKind::DiagramGallery + Thumbnail-Slider/Großansicht + Export-Buttons
- [x] `plans-panel-diagrams` - Gallery-Button pro Plan-Card öffnet zentrierten Gallery-Tab
- [ ] `fence-detection` - ```mermaid-Blöcke im Chat-Markdown inline rendern (Fallback) — OFFEN
- [ ] `cost-gate` - Workspace-Setting + einmalige Token-Kosten-Bestätigung pro Session — OFFEN
- [ ] `render-mode-setting` - Workspace-Setting "Always gallery" vs Default-Inline — OFFEN
- [ ] `default-count-setting` - Workspace-Setting default_diagram_count (Default 1) — OFFEN
- [ ] `role-autogen` - architect/coordinator Auto-Set; Plain-Chat ask_user_card-Flow — TEILWEISE (System-Prompt-Guidance vorhanden; UI-Flow offen)
- [x] `export-md` - .md-Export (Front-Matter + ```mermaid) via tauri-plugin-dialog Save-As
- [x] `export-pdf` - SVG→PDF-Command (svg2pdf), Orientierung aus SVG-Maßen, Save-As-Dialog
- [x] `dialog-plugin` - tauri-plugin-dialog hinzugefügt + `dialog:allow-save` Capability
- [x] `system-prompt` - Diagramm-Fähigkeit + Rollenverhalten + Tool-Index in system_prompt.rs
- [>] `i18n-strings` - I18nKeys + en_us gesetzt; übrige Locales mit EN-Platzhaltern (Übersetzung via Script offen)
- [x] `sanitize-svg` - Mermaid läuft mit `securityLevel: strict` (bestehende Glue) — kein roher inner_html-Pfad
- [ ] `docs-changelog` - Doku (agent-harness.md, user-docs) + Changelog-Eintrag — OFFEN
