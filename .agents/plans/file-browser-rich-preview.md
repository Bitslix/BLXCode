# File Browser Rich Preview (Images, Video, Markdown, Mermaid)

## Summary

Erweiterung des Center-Tab `CenterTabKind::FilePreview`, sodass der Sidebar-File-Browser nicht nur UTF-8-Text, sondern auch Bilder, Videos, gerendertes Markdown und Mermaid-Diagramme anzeigt. Heute schlägt der Preview bei Binärdateien mit `file is not valid UTF-8 text` fehl (siehe `dist/public/blxcode.png`), SVGs werden als Roh-XML angezeigt statt gerendert, Markdown bleibt monospaced.

Ziel ist ein einheitlicher Preview-Dock mit:

- **Topbar** mit Datei-Metadaten (Name, relativer Pfad, Größe, mtime, optional Typ-Icon).
- **Renderer-Dispatch** nach Dateityp: Image, Video, Markdown, Mermaid, Text-Fallback.
- **Sandbox** über die bestehende `canonical_root` / `resolve_under_root`-Logik in `fs_entries.rs`.

## Decisions

- **Dispatch-Quelle:** Renderer-Auswahl im Frontend per Extension; Backend liefert zusätzlich einen MIME-Hint (über `infer` oder Extension-Map) für Edge-Cases ohne Extension.
- **Bytes-Transport:** Base64 im JSON-Response. Vorteile: kein Asset-Protokoll-Scope nötig, keine CSP-Änderung, einfache Sandbox-Kette. Nachteil ~33% Overhead — akzeptabel für Bilder ≤ Cap.
- **Größen-Caps:**
  - Bilder: 16 MiB (`MAX_IMAGE_PREVIEW_BYTES`). Größer → Topbar zeigt Hinweis + Download-Hint.
  - Video: 64 MiB (`MAX_VIDEO_PREVIEW_BYTES`). Größer → Hinweis, kein Auto-Play. Video bleibt Base64-Pfad in v1 (siehe Open Questions).
  - Markdown/Text/Mermaid: bestehender 512 KiB-Cap aus `read_workspace_text_file` bleibt.
- **Image-Pfad:**
  - SVG → inline als sanitisierter String in einen `<div>` mounten (nicht `<img>`, damit CSS-Themes greifen).
  - Raster (png/jpg/jpeg/webp/gif/avif/bmp/ico) → `data:<mime>;base64,…` in `<img>`.
- **Mermaid:**
  - Eigenständige `.mmd` / `.mermaid` Dateien → komplettes Diagramm.
  - Markdown ` ```mermaid ` Code-Blöcke → inline rendern statt als `<code>`.
  - Mermaid-Lib lazy laden (vendored ES-Modul in `public/vendor/mermaid/`), kein CDN, damit Offline + CSP-konform.
- **Markdown-Renderer:** `pulldown-cmark` (bereits Dep). Eigene Pipeline für File-Preview, nicht der `chat_markdown.rs`-Pfad (kein Wikilink-Expansion, keine `<details>`-Verpackung für Code-Blöcke). Code-Blöcke bleiben offene `<pre><code>`-Blöcke mit Sprachen-Klasse.
- **Sanitization:** HTML aus Markdown wird über eine kleine Allowlist (oder `ammonia`, falls leicht zu Wasm-bauen ist) gefiltert. Inline-Skripte, `on*`-Attribute, `javascript:` Links blockieren. **Hinweis:** Erst prüfen, ob `ammonia` als WASM kompiliert (depends on `html5ever`). Wenn nein → eigene konservative Allowlist auf dem schon gerenderten `pulldown-cmark`-Output.
- **Topbar-Metadaten:** Aus neuem Command `stat_workspace_file`. Felder: `name`, `rel_path`, `byte_len`, `modified_ms` (Unix-ms, optional `None`), `kind` (Enum). Frontend formatiert Größe (`1.4 MiB`) und Datum lokal.
- **Refresh-Button** der heutigen Topbar bleibt erhalten und triggert auch Metadaten-Reload.
- **Fallback** bei unbekanntem Typ: bestehender `<pre><code>`-Pfad.

## Implementation Notes

### Backend (`src-tauri/src/fs_entries.rs`)

Neue Typen:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub name: String,
    pub rel_path: String,
    pub byte_len: u64,
    pub modified_ms: Option<i64>,
    pub kind: FileKind,
    pub mime: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind { Image, Video, Markdown, Mermaid, Text, Binary }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryFilePreview {
    pub base64: String,
    pub mime: String,
    pub byte_len: u64,
    pub truncated: bool,
}
```

Neue Commands (sandboxed über bestehende Helfer):

- `stat_workspace_file(workspace_root, path) -> FileMeta`
  - Klassifiziert nach Extension; Mermaid für `.mmd`/`.mermaid`; Markdown für `.md`/`.markdown`; Image-Set und Video-Set per `const`-Arrays.
  - `modified_ms` aus `metadata.modified()` → Unix-ms; `None` falls plattformbedingt nicht verfügbar.
- `read_workspace_image_file(workspace_root, path) -> BinaryFilePreview`
  - Cap `MAX_IMAGE_PREVIEW_BYTES = 16 * 1024 * 1024`; `mime` aus Extension-Map; SVG bleibt Bytes+`image/svg+xml` (Frontend behandelt SVG separat via Text-Pfad — siehe Bridge-Note).
- `read_workspace_video_file(workspace_root, path) -> BinaryFilePreview`
  - Cap `MAX_VIDEO_PREVIEW_BYTES = 64 * 1024 * 1024`; mime aus Extension-Map.
- Bestehendes `read_workspace_text_file` wird **nicht** geändert — Markdown/Mermaid/Text reuse den Command. Topbar ruft separat `stat_workspace_file`.

Extension-Map (in `fs_entries.rs`):

| Kind | Extensions |
|---|---|
| Image | `png`, `jpg`, `jpeg`, `webp`, `gif`, `avif`, `bmp`, `ico`, `svg` |
| Video | `mp4`, `webm`, `mov`, `m4v`, `mkv` (Hinweis: `mkv` Browser-Support variabel — Open Question) |
| Markdown | `md`, `markdown` |
| Mermaid | `mmd`, `mermaid` |

Registrierung in `src-tauri/src/lib.rs` neben `read_workspace_text_file`.

### Bridge (`src/tauri_bridge.rs`)

Neue Wrapper:

```rust
pub async fn stat_workspace_file(root: String, path: String) -> Result<FileMeta, String>;
pub async fn read_workspace_image_file(root: String, path: String) -> Result<BinaryFilePreview, String>;
pub async fn read_workspace_video_file(root: String, path: String) -> Result<BinaryFilePreview, String>;
```

Plus Serde-spiegelnde Typen (`FileMeta`, `FileKind`, `BinaryFilePreview`). SVG nutzt `read_workspace_text_file` (SVG ist Text), nicht den Image-Endpoint — vermeidet Doppel-Encoding.

### Frontend (`src/workbench/workspace_panel.rs` + neues Modul)

Refactor von `FilePreviewDock` zu einem Dispatcher. Neues Modul `src/workbench/file_preview/`:

- `mod.rs` — `FilePreviewDock` Component (öffentlich), lädt `FileMeta`, rendert `FilePreviewHeader` + dispatcht Renderer.
- `header.rs` — Topbar mit Icon (typabhängig), Name, kopierbarer Pfad, formatierter Größe, mtime, Refresh-Button.
- `image_view.rs` — SVG-Inline + Raster-`<img>`-Pfad; CSS-Klassen `.file-preview__image-stage` mit `display:grid; place-items:center; min-height:0`.
- `video_view.rs` — `<video controls preload="metadata">` mit `src="data:…;base64,…"`. Bei `byte_len > cap` Hinweis statt Player.
- `markdown_view.rs` — `pulldown-cmark` Options: `ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS | ENABLE_FOOTNOTES | ENABLE_SMART_PUNCTUATION`. Mermaid-Block-Detection: vor `html::push_html` Stream filtern, `Code(CodeKind::Fenced(lang))` mit `lang.starts_with("mermaid")` → Sentinel-`<div class="file-preview__mermaid" data-graph="…">` ersetzen. Nach Rendern: Sanitizer, dann `inner_html` setzen und Mermaid auf den Container anwenden.
- `mermaid_view.rs` — `<div class="file-preview__mermaid-stage">`; lädt `public/vendor/mermaid/mermaid.esm.min.mjs` lazy (per `js-sys`/`wasm-bindgen` dynamischer Import oder `<script type="module">`-Inject), ruft `mermaid.run({ nodes: [el] })`. Diagramm-Source: gesamter Dateiinhalt aus `read_workspace_text_file`.
- `util.rs` — `format_bytes(u64) -> String`, `format_mtime(Option<i64>) -> String`, `classify_extension(&str) -> FileKind`.

`workspace_panel.rs` importiert nur noch `FilePreviewDock` aus dem Modul; alte Inline-Implementierung entfällt.

### Mermaid-Asset

- Vendored: `public/vendor/mermaid/mermaid.esm.min.mjs` (aktuelle Mermaid 11.x). Wird von Trunk in `dist/` mitkopiert.
- Lazy-Loader gated über `OnceCell<Promise>`: erstes `mermaid_view`-Mount lädt das Modul; weitere Renderer wiederverwenden den Cache.
- Theme: Mermaid `initialize({ startOnLoad: false, theme: 'dark' })`; späteres Mapping auf BLXCode-Theme ist Follow-up.

### Sanitization

- Primäransatz: **eigene Allowlist** auf dem `pulldown-cmark`-Output (Tags: `a`, `p`, `h1`–`h6`, `ul`, `ol`, `li`, `code`, `pre`, `blockquote`, `strong`, `em`, `del`, `hr`, `br`, `table`, `thead`, `tbody`, `tr`, `th`, `td`, `img`, `div`, `span`; Attribute: `href`, `src`, `alt`, `title`, `class`, `align`, `data-graph`). Block: `on*`-Attribute, `style`, `javascript:`-URLs.
- Mermaid-Sentinel-`div` darf `class="file-preview__mermaid"` und `data-graph` behalten.
- Falls `ammonia` problemlos WASM-baut: stattdessen `ammonia` mit identischer Konfiguration.

### CSS (`styles.css`)

Neue Klassen unter `.file-preview`:

- `.file-preview__header` (bereits da) — erweitern um `.file-preview__meta` (size, mtime, path).
- `.file-preview__stage` — Flex/Grid centered, `min-height:0`, `overflow:auto`.
- `.file-preview__image` / `.file-preview__image-svg` — `max-width:100%; max-height:100%; object-fit:contain`.
- `.file-preview__video` — `max-width:100%; max-height:100%`.
- `.file-preview__markdown` — Typografie (Headings, Tabellen, Code-Blöcke).
- `.file-preview__mermaid` / `.file-preview__mermaid-stage` — centered, dunkler Hintergrund.
- `.file-preview__error` / `.file-preview__notice` — bleibt.

### i18n (`src/i18n/keys.rs` + alle `locales/*.rs`)

Neue Keys:

- `file_preview_size` — formatierte Größe-Label.
- `file_preview_modified` — `Modified` / `Geändert` Prefix.
- `file_preview_too_large` — "Datei zu groß für Vorschau ({size})".
- `file_preview_unsupported` — "Vorschau für diesen Typ nicht verfügbar".
- `file_preview_loading_mermaid` — "Mermaid wird geladen…".

Erinnerung aus `CLAUDE.md`: Alle Sprach-Locale-Files müssen die neuen Keys haben (compile-time exhaustiveness).

### Betroffene Dateien

| Datei | Änderung |
|---|---|
| `src-tauri/src/fs_entries.rs` | `FileMeta`, `FileKind`, `BinaryFilePreview`, neue Commands |
| `src-tauri/src/lib.rs` | Command-Registrierung |
| `src/tauri_bridge.rs` | Wrapper + Serde-Spiegel |
| `src/workbench/file_preview/mod.rs` | neu (Dispatcher) |
| `src/workbench/file_preview/header.rs` | neu |
| `src/workbench/file_preview/image_view.rs` | neu |
| `src/workbench/file_preview/video_view.rs` | neu |
| `src/workbench/file_preview/markdown_view.rs` | neu |
| `src/workbench/file_preview/mermaid_view.rs` | neu |
| `src/workbench/file_preview/util.rs` | neu (format, classify, sanitize) |
| `src/workbench/workspace_panel.rs` | alte `FilePreviewDock`-Inline-Impl entfernen, Modul importieren |
| `src/workbench/mod.rs` | `pub mod file_preview` |
| `public/vendor/mermaid/mermaid.esm.min.mjs` | vendored Asset |
| `styles.css` | Stage-/Markdown-/Mermaid-Stile |
| `src/i18n/keys.rs` + `src/i18n/locales/*.rs` | neue Keys |
| `Cargo.toml` | optional `ammonia` (wenn WASM-tauglich); `infer` für MIME-Hint optional |

## Tests

### Backend (`src-tauri/src/fs_entries.rs`)

| Szenario | Erwartung |
|---|---|
| `stat_workspace_file` auf `.md` | `kind = Markdown`, `mime = text/markdown` |
| `stat_workspace_file` auf `.png` | `kind = Image`, `mime = image/png`, `byte_len` korrekt |
| `stat_workspace_file` auf `.mmd` | `kind = Mermaid` |
| `stat_workspace_file` auf unbekannte Extension | `kind = Binary` oder `Text` (Heuristik) |
| `read_workspace_image_file` über Cap | `truncated = true`, kein Crash |
| `read_workspace_image_file` außerhalb Root | Error wie bestehender Pfad |
| `read_workspace_video_file` auf Nicht-Video-Extension | Error / Empty-Reject |

### Frontend / Manuell (`cargo tauri dev`)

| Szenario | Erwartung |
|---|---|
| Klick auf `dist/public/blxcode.png` | Bild gerendert, centered, kein UTF-8-Fehler |
| Klick auf `dist/public/brand-icons/anthropic.svg` | SVG inline gerendert, nicht als Roh-XML |
| Klick auf `README.md` | Gerendertes Markdown mit Tabellen und Code-Blöcken |
| Markdown mit ```` ```mermaid ``` Block | Block wird als Diagramm gerendert, anderer Code bleibt `<pre><code>` |
| Klick auf `.mmd`-Datei | Diagramm gerendert, Topbar zeigt `.mmd`-Icon |
| Klick auf `.mp4`-Datei ≤ 64 MiB | `<video controls>` spielt ab |
| Klick auf `.mp4` > Cap | Hinweis "zu groß", kein Player |
| Klick auf unbekannte Binärdatei (`.bin`) | Fallback-Hinweis statt UTF-8-Fehler |
| Topbar | Zeigt Name, Pfad, Größe (`1.4 MiB`), mtime |
| Refresh-Button | Lädt Bytes + Meta neu |
| Theme-Wechsel | Markdown/Mermaid bleiben lesbar |

### Sicherheit

- Markdown mit `<script>alert(1)</script>` → wird gestrippt.
- Markdown mit `<a href="javascript:…">` → Link entfernt oder neutralisiert.
- Mermaid-Quelle mit eingebettetem `<script>` → Mermaid sanitisiert intern; zusätzlich Sentinel-Container ohne `dangerouslySetInnerHTML` außerhalb der Mermaid-Lib.

## Open Questions

- **Video-Streaming:** Base64 für ≥30 MiB Videos ist suboptimal (Memory + Decode-Latenz). Alternativen für v2:
  - Tauri `asset:`-Protokoll mit Scope auf `workspace_root` → schnellerer Stream, kleine Config-Änderung in `tauri.conf.json`.
  - Eigener `http://localhost:…` Range-Server.
  - Entscheidung jetzt: v1 = Base64 + 64 MiB Cap; v2-Folgeplan, falls UX nicht reicht.
- **Mermaid Theme Mapping:** v1 = `dark`. Mapping auf alle BLXCode-Themes optional in Folge-Plan.
- **`.mkv`-Wiedergabe:** Browser-Support ist inkonsistent. v1: in Map enthalten, aber bei Decode-Fehler Fallback-Hinweis im Player.
- **`ammonia` als WASM-Dep:** Vor Implementierung mit `cargo check -p blxcode-ui --target wasm32-unknown-unknown` prüfen; bei Problemen eigene Allowlist.

## Tasks

- [x] `backend-meta` - `FileMeta`, `FileKind`, `stat_workspace_file` Command + Unit-Tests
- [x] `backend-image` - `BinaryFilePreview` + `read_workspace_image_file` + Cap + Tests
- [x] `backend-video` - `read_workspace_video_file` + Cap + Tests
- [x] `backend-register` - Commands in `lib.rs` registrieren
- [x] `bridge-types` - Serde-Spiegel + async Wrapper in `tauri_bridge.rs`
- [x] `frontend-module` - Neues `file_preview/`-Modul mit `mod.rs` Dispatcher + `util.rs` (classify, format)
- [x] `frontend-header` - Topbar mit Name/Pfad/Größe/mtime + Refresh
- [x] `frontend-image` - SVG-Inline + Raster-`<img>` mit Data-URL + Centered-Stage
- [x] `frontend-video` - `<video>`-Renderer mit Cap-Hinweis
- [x] `frontend-markdown` - `pulldown-cmark` Pipeline + Sanitization + Mermaid-Sentinel
- [x] `mermaid-asset` - Vendored Mermaid-Bundle unter `public/vendor/mermaid/` (UMD `mermaid.min.js`)
- [x] `frontend-mermaid` - Lazy-Loader + `mermaid.run` Integration für `.mmd` und Markdown-Blöcke
- [x] `workspace-panel-wire` - Alte `FilePreviewDock`-Impl in `workspace_panel.rs` entfernen, neues Modul nutzen
- [x] `i18n-keys` - Neue Keys in `keys.rs` + alle `locales/*.rs`
- [x] `styles` - `.file-preview__stage`, `__image`, `__video`, `__markdown`, `__mermaid` Stile
- [x] `code-vendor` - Vendor `public/vendor/highlight/highlight.min.js` (highlight.js 11 common bundle)
- [x] `code-backend` - Neue `FileKind::Code`, erweiterte `classify_kind`, MIME für ts/js/yaml/toml/csv
- [x] `code-bridge` - `FileKind::Code` im Frontend-Bridge-Spiegel
- [x] `code-util` - `hljs_lang_for_ext`, `html_escape`, `split_highlighted_into_lines` + Unit-Tests
- [x] `code-glue` - `hljs_glue.rs` mit `ensure_hljs_loaded` + `highlight`
- [x] `code-view` - `CodeView` mit Line-Numbers, Syntax-Highlighting, Row-Selection
- [x] `code-dispatch` - Dispatcher routet `Code | Text` → `CodeView`
- [x] `code-styles` - `.code-view` Layout + hljs-Token-Mapping mit Theme-Tokens (dark + light)
- [x] `code-drag-selection` - Drag-Range-Selection (mousedown→mousemove→window-mouseup) auf `.code-view` mit Single-Click-Toggle erhalten
- [x] `code-snippet-util` - `build_file_snippet_block` in `file_preview/util.rs` mit fenced Markdown + Source-Workspace-Header für Cross-Workspace
- [x] `code-envelope` - `render_file_snippet_envelope` in `agent_context_handoff.rs` als Mini-Variante des Handoff-Blocks
- [x] `code-context-item` - `AgentContextKind::FileSnippet` + optionales `content`-Feld auf Frontend-Bridge + Backend-Protocol + Prompt-Renderer (`session_orchestrator.rs`)
- [x] `code-cross-workspace-terms` - `list_terminal_targets_all_workspaces` enumeriert alle Workspaces, gruppiert + Shell-Workspaces ausgefiltert
- [x] `code-context-menu` - Neues `code_context_menu.rs`-Modul: gruppiertes Menü (Snippet→Terminal, Envelope→Terminal, Attach→Agent, Clipboard) mit "current"-Badge für Preview-Workspace
- [x] `code-menu-wire` - `on:contextmenu` in `CodeView`, Menu-State-Signal, Window-mousedown/Escape zum Schließen, Range-Capture
- [x] `code-actions` - `pty_write` für Snippet & Envelope, `upsert_workspace_agent_context` für Attach, `navigator.clipboard.writeText` für Copy-Snippet/Range/Raw mit Toast-Feedback
- [x] `code-i18n-handoff` - 22 neue Keys (`CodeViewMenu*` + `CodeViewToast*`) + Übersetzungen in allen 13 Locales (en/de/fr/es/it/pt_br/pl/hu/ru/ja/ko/zh_cn/zh_tw)
- [x] `code-css-menu` - `user-select:none` auf `.code-view` + `.code-context-menu` Stile (Sektionen, Workspace-Gruppen, Slot-Items, Badge)
- [x] `manual-verify` - Manuelle Checks aus Test-Tabelle inkl. PNG, SVG, MD, MMD, MP4, sowie TS/JS/RS-Files mit Line-Selection + Drag-Selection + Rechtsklick → Terminal/Agent/Clipboard (auch cross-workspace)
