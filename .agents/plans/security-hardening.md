# Security Hardening

## Summary

Zwei parallele Security-Reviews (Agent-Subsystem + Tauri/Frontend) identifizierten **1 Critical**, **5 High**, **~12 Medium** und mehrere Low-Befunde. Relevante Luecken: **Subagent-Sandbox-Umgehungen**, **Read-only-Shell-Bypass** (`bash -lc`), **XSS → volle IPC** (keine CSP, unsanitized Chat-Markdown).

Positiv: `fs_entries.rs` mit `canonicalize` + Prefix-Check; API-Keys in Keyring; Updater mit Minisign; File-Preview-Sanitizer vorhanden.

Phasen: **P0 Critical** sofort → **P1 High** parallel zu [v2-roadmap.md](v2-roadmap.md) V2.0 → **P2 Medium** mit V2.1 → **P3 Low**.

Siehe Befund-IDs SEC-01 … SEC-20 in Implementation Notes.

## Decisions

- **P0 zuerst:** Subagent `writes:true`-Bypass und fehlende Runtime-Tool-Allowlist — widersprechen dokumentiertem Harness ([coordinated-subagents.md](coordinated-subagents.md)).
- **`writes`-Argument:** Nur aus `ToolExecOpts.shell_writes` / Gruppen ableiten; LLM-JSON darf `writes` nicht setzen ohne `shell_write`-Gruppe.
- **Shell Read-only:** Metazeichen `;`, `&&`, `|`, `` ` ``, `$()` verbieten; `node`/`npm`/`cargo` verschärfen oder entfernen.
- **XSS:** `sanitize_markdown_html` auf alle Chat/Memory/Plans/Skills `inner_html`-Pfade (wie File-Preview).
- **CSP:** Strikte Policy; `withGlobalTauri: false`; IPC nur via `tauri_bridge.rs`.
- **URL-Allowlist:** Nur `http`/`https` (optional `mailto` fuer `open_external_url`); Backend + Frontend.
- **Symlink-Policy:** Einheitlich `resolve_under_root` aus `fs_entries.rs` in Agent-Tools, Memory, Plans, Git.
- **Coordinator `shell_write`:** By design hochprivilegiert — kein Bug.
- **`web_fetch` (v2-roadmap):** SSRF-Schutz **vor** Merge (`sec-web-fetch-ssrf` blockiert `web-fetch-impl`).

## Implementation Notes

### Befunde (Kurz)

| ID | Sev | Befund | Dateien |
|----|-----|--------|---------|
| SEC-01 | Critical | Subagent `shell_exec` + `writes:true` umgeht `shell_write`-Gruppe | `shell_exec.rs`, `subagent_runner.rs` |
| SEC-02 | High | Read-only-Allowlist: `bash -lc` + `;`/`&&` | `shell_exec.rs` |
| SEC-03 | High | `node`/`npm`/`cargo` ohne Subcommand-Check | `shell_exec.rs` |
| SEC-04 | High | Keine serverseitige Runtime-Tool-Allowlist | `subagent_runner.rs`, `tool_dispatch.rs` |
| SEC-05 | High | Chat-Markdown ohne Sanitizer → `inner_html` | `chat_markdown.rs`, `timeline.rs`, `memory_panel.rs` |
| SEC-06 | High | `csp: null` + `withGlobalTauri: true` | `tauri.conf.json` |
| SEC-07 | High | `javascript:`/`file:` in Browser + `open_external_url` | `state.rs`, `lib.rs`, `browser_tab.rs` |
| SEC-08–17 | Medium | Symlinks, git paths, `agent_read_image_file`, PTY injection, web key order, `browser_run_js`, preview gaps, PTY cwd, workbench.json | diverse |
| SEC-18–20 | Low | `find` allowlist, hook integrity, broad capabilities | diverse |

### P0 Critical (~1 PR)

**SEC-01** — In `tool_shell_exec`: `writes` nur wenn `default_writes == true`; sonst `false` und `args["writes"]` ignorieren. Schema in `tools_extra.rs` anpassen.

**SEC-04** — Vor `execute_server_tool`: `registry_filtered(groups)` als Allowlist; unbekannte Tool-Namen ablehnen (Subagent + Coordinator).

### P1 High (2–3 PRs)

- Shell: Metazeichen-Reject; Binaries verschärfen; Unit-Tests.
- XSS: `sanitize_markdown_html` nach `render_markdown_to_html`; alle `inner_html`-Pfade auditieren.
- CSP in `tauri.conf.json`; Scripts nur self + vendored.
- Shared URL-Validator Rust + WASM; iframe `sandbox`.

### P2 Medium (2–3 PRs)

- Shared `resolve_under_root`; Git-Pfad-Sandbox; `agent_read_image_file` Workspace-only.
- PTY Control-Char-Filter; Web-Key Keyring vor Env; `browser_run_js` entfernen.
- Preview-Sanitizer: `data:`, `<base>`, `<meta refresh>`; `pty_spawn` cwd binden; workbench.json 0600.

### P3 Low

- `find` aus read-only; Hook-Checksum; Capability-Split; SSRF bei `web_fetch`; Docs.

### Abhaengigkeiten v2-roadmap

| Security Task | V2 Task | Hinweis |
|---------------|---------|---------|
| `sec-web-fetch-ssrf` | `web-fetch-impl` | SSRF vor Merge |
| `sec-subagent-writes-fix` | — | Sofort (Subagents shipped) |
| P0/P1 Tests | `ci-cargo-test` | In gleicher CI-PR |

## Tests

- **P0:** Subagent `shell_exec { writes: true }` ohne `shell_write` → rejected; `git_commit` ohne `git_write` → rejected; Coordinator mit `shell_write` weiterhin ok.
- **P1:** `git status && curl` rejected; `node -e` rejected; XSS in Chat-MD sanitized; `javascript:` URL rejected.
- **P2:** Symlink read rejected; `git_add` mit `..` rejected; `agent_read_image_file` outside workspace rejected.
- **Regression:** `cargo test --workspace`; bestehende plans/memory traversal tests.

## Tasks

### P0 Critical

- [ ] `sec-subagent-writes-fix` - writes an default_writes/shell_write-Gruppe binden; Schema anpassen
- [ ] `sec-runtime-tool-allowlist` - Runtime Tool-Name-Pruefung vor execute_server_tool

### P1 High

- [ ] `sec-shell-metachar-block` - Read-only: Metazeichen verbieten; Tests
- [ ] `sec-shell-binaries-tighten` - node/npm/cargo Allowlist verschaerfen oder entfernen
- [ ] `sec-chat-markdown-sanitize` - sanitize_markdown_html auf Chat/Memory/Plans inner_html
- [ ] `sec-csp-global-tauri` - CSP setzen; withGlobalTauri false
- [ ] `sec-url-schema-allowlist` - http/https only Browser + open_external_url + iframe sandbox

### P2 Medium

- [ ] `sec-resolve-under-root-unify` - Shared path resolver; Agent/Memory/Plans/Git
- [ ] `sec-git-path-sandbox` - git_add/git_ls_files Pfadnormalisierung
- [ ] `sec-agent-read-image-sandbox` - agent_read_image_file auf Workspace beschraenken
- [ ] `sec-pty-injection-filter` - Control-Chars in Handoff/send_terminal_keys filtern
- [ ] `sec-web-key-order` - Keyring vor Env in web_settings.rs
- [ ] `sec-browser-run-js-remove` - browser_run_js entfernen oder internal-only
- [ ] `sec-preview-sanitizer-harden` - data:/base/meta in file_preview/util.rs blockieren
- [ ] `sec-pty-cwd-sandbox` - pty_spawn cwd + list_directory einschraenken
- [ ] `sec-workbench-json-perms` - workbench.json 0600; optional timeline opt-out

### P3 Low / Future

- [ ] `sec-find-readonly-remove` - find aus read-only Allowlist
- [ ] `sec-hooks-integrity` - Hook-Skripte Bundle-Checksum
- [ ] `sec-capabilities-split` - Feingranulare Tauri capabilities
- [ ] `sec-web-fetch-ssrf` - SSRF-Schutz bei web_fetch (blockiert v2 web-fetch-impl)
- [ ] `sec-docs-security` - CLAUDE.md + docs/developer/agent-harness.md
