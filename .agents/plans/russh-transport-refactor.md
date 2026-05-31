# Russh Transport Refactor

## Summary

Heute startet jedes Remote-Terminal einen **eigenen `ssh`-Client-Prozess** (wrapped system OpenSSH,
PTY pro Terminal) und der Exec-Channel (fs/git/resume) eine weitere `ssh`-Session. Ein Workspace mit
*N* Terminals = **N + 1 SSH-Verbindungen**, je mit vollem Handshake + Auth. Das stößt an
`sshd MaxStartups`/`MaxSessions`, kostet N× Auth-Latenz und (bei Passwort-Auth) N× PTY-Prompt-Injection.

Der **Plan**: den Remote-Transport auf **[russh](https://docs.rs/russh)** (pure-Rust async SSH) umstellen.
Eine **multiplexte Verbindung pro Connection-Preset**: ein authentifizierter `client::Handle`, über den
*alle* Terminals (je ein Session-Channel mit eigenem Remote-PTY) **und** fs/git/resume (je ein
`exec`-Channel) laufen. Ergebnis: **1 TCP + 1 Auth pro Connection**, funktioniert **auch auf Windows**
(kein `ControlMaster` nötig), und der Exec-Layer wird **drastisch einfacher** (echte Channel-`exec` mit
stdout/stderr/exit-status statt Marker-RPC + base64 + Temp-Files).

Bewusste Eingrenzung: **Lokale Terminals bleiben `portable-pty`** (unverändert). Nur der Remote-Pfad
wird getauscht. **Das Frontend bleibt unverändert** — die poll-basierte PTY-API (`pty_drain_wait`,
`pty_resize`, `pty_kill`) und die Exec-API (`run`/`run_text`/`run_check`) behalten ihre Signaturen, nur
die Implementierung dahinter wechselt. Baut auf [remote-ssh-file-git-resume.md](remote-ssh-file-git-resume.md) auf.

## Decisions

- **Transport:** `russh` + `russh-keys`, getrieben von der vorhandenen Tokio-Runtime. Version **pinnen**
  (API-Churn) — Ziel `russh = "0.4x"` kompatibel mit `russh-keys` gleicher Minor.
- **Eine `Handle` pro `connection_id`** (nicht pro Workspace): teilt automatisch auch über Workspaces
  mit gleichem Preset. Ein neuer `SshTransportManager` ersetzt die „wrapped-ssh"-Spawns; der bestehende
  `RemoteExecManager` wird auf diesen Transport umgebaut.
- **Terminals = Session-Channels** mit `request_pty` + `request_shell` (bzw. `exec` der tmux-/cd-Zeile
  für Resume). Reader-Task pro Channel pumpt Bytes in dieselbe Queue (`Mutex<VecDeque>` + Notify), die
  `pty_drain_wait` heute liest → **Frontend-API unverändert**.
- **Exec = ephemerer `exec`-Channel pro Aufruf**: `channel.exec(cmd)` → stdout/stderr/exit-status direkt.
  **Marker-Protokoll, base64-Framing, Temp-Files, `stty -echo`-Handshake entfallen ersatzlos.**
- **Host-Key-Verifikation ist Pflicht** (kein Blind-Accept): TOFU gegen `~/.ssh/known_hosts`
  (`russh-keys` Helpers) — unbekannt → lernen+akzeptieren, **Mismatch → harter Fehler** mit klarer
  UI-Meldung + expliziter „Trust"-Aktion (Folge-Task).
- **Auth:** Password (+ keyboard-interactive-Fallback), Key-File (+ Passphrase via `russh-keys`
  decode), SSH-Agent. Secrets bleiben in Rust (`ssh_secrets`), nie über die Bridge.
- **Keepalive:** `client::Config.keepalive_interval`/`keepalive_max` ersetzt `ServerAliveInterval`.
- **Lokaler Pfad unangetastet**, Frontend unangetastet — minimaler Blast-Radius, leicht reviewbar.

## Risks & Trade-offs (bewusst benannt)

| Risiko | Bewertung / Mitigation |
|--------|------------------------|
| **`MaxSessions` gilt weiter** — Channels zählen gegen `MaxSessions` (Default 10). >10 Terminals auf *einer* Handle blockieren weiterhin. | **Connection-Pool**: ab Schwelle (z. B. 8 Channels) eine zweite Handle pro Connection öffnen. Als eigener Task (RUSSH-P6). Löst zusätzlich `MaxStartups` (Rate) ohnehin. |
| **Verlust von `~/.ssh/config`** (ProxyJump, Include, Host-Aliase, IdentityFile-Auto) — wrapped ssh bekam das gratis. | `russh-config` parst `~/.ssh/config` teilweise; ProxyJump müsste man selbst implementieren. **Entscheidung:** v1 ohne config/ProxyJump; als bekannte Lücke dokumentieren, später nachrüsten. |
| **SSH-Agent auf Windows** (Pageant / OpenSSH-Named-Pipe `\\.\pipe\openssh-ssh-agent`). | `russh-keys` Agent-Support auf Unix sicher; Windows verifizieren. Fallback: Agent-Auth auf Windows zunächst „nicht unterstützt" melden, Key/Password greifen. |
| **russh API-Churn** zwischen Minor-Versionen. | Version pinnen, dünne Adapter-Schicht, Integrationstests gegen echten Host. |
| **Async/Sync-Brücke** zur bestehenden `Condvar`-Queue. | russh-Reader-Task (async) darf die std-`Mutex`-Queue füllen + `Condvar.notify` rufen (kurze Critical Section, unproblematisch). Optional: Queue auf `tokio::sync::Notify` umstellen, falls `pty_drain_wait` Worker zu lange blockiert (heute ≤250 ms — akzeptabel). |
| **PTY-Semantik-Parität** (TERM, Fenster-Resize, Signale). | `request_pty(term="xterm-256color", cols, rows)`, `window_change` bei Resize; `TERM`/`COLORTERM` via PTY-Req statt `env` (sshd `AcceptEnv` oft restriktiv). |
| **Reconnect/Drop**: stirbt die Handle, fallen *alle* Terminals zusammen (gegen heute: unabhängig). | Disconnect sichtbar machen (Terminal-Notice „connection lost"), manueller Reconnect; optional Auto-Reconnect mit Backoff (Folge-Task). Trade-off bewusst akzeptiert (Preis des Multiplexings). |
| **known_hosts** hashed/`@cert-authority`-Einträge. | `russh-keys`-Helper nutzen; nicht selbst parsen. Schreibrechte 0600. |

## Zielarchitektur

```
SshTransportManager  (Tauri-managed, ersetzt wrapped-ssh-Spawn + Exec-Master)
  HashMap<connection_id, Arc<SshConnection>>
        │
        └── SshConnection
              ├── russh client::Handle           (1 TCP, 1 Auth, multiplext)
              ├── channels: Vec<terminal channel> (je Remote-PTY → Terminal)
              └── (exec: ephemere Channels on demand)
```

- **Terminal:** `pty_spawn_remote` → `transport.open_terminal(conn_id, spec)` → Channel + Reader-Task →
  Session-Id in `PtyManager`-kompatibler Map → `write`/`resize`/`kill`/`drain` wie heute.
- **Exec:** `RemoteExecManager::run` → `transport.exec(conn_id, cmd)` → (code, stdout, stderr).
- **Lifecycle:** Handle lazy beim ersten Terminal/Exec; Refcount der Channels; Handle schließen, wenn
  letzter Channel weg / Workspace-Close / App-Exit.

## Phasen & Implementation Notes

### P0 — Dependencies & Transport-Grundgerüst
- `Cargo.toml`: `russh`, `russh-keys` (pinned); `tokio`-Features um `rt-multi-thread`, `net` ergänzen.
- Neues Modul `src-tauri/src/ssh_transport.rs`: `SshTransportManager` + `SshConnection`; `client::Config`
  (keepalive); Connect+Auth-Flow; `Arc`-Caching pro `connection_id`; Tokio-Driver-Task pro Handle.
- `resolve_spec` ([ssh_remotes.rs](src-tauri/src/ssh_remotes.rs)) liefert weiterhin Host/Port/User/Auth/
  Resume/remote_dir; Secrets via [ssh_secrets.rs](src-tauri/src/ssh_secrets.rs).

### P1 — Host-Key-Verifikation (TOFU)
- `client::Handler::check_server_key`: `russh-keys` known_hosts-Abgleich; unbekannt→lernen+akzeptieren,
  Mismatch→`Err` mit stabilem Code (`host_key_changed`). UI-Fehlermeldung + i18n.
- Folge-Task: explizite „Trust new host key"-Aktion (Settings/Toast) zum Überschreiben.

### P2 — Auth (Password / Key+Passphrase / Agent)
- Password: `authenticate_password`; bei Bedarf `keyboard-interactive`-Fallback.
- Key: Keyfile laden (`russh-keys` `load_secret_key` mit Passphrase), `authenticate_publickey`.
- Agent: `russh-keys` Agent-Client (`SSH_AUTH_SOCK`); Windows-Pipe verifizieren (siehe Risiko).
- Saubere Fehlerklassifikation: auth-failed / unreachable / host-key-changed / timeout.

### P3 — Remote-Terminals über Channels
- `pty_host.rs`: `PtySession` zu Enum `Local(portable_pty…)` / `Remote(russh channel + writer)` machen
  ODER eine parallele Remote-Session-Map; `write`/`resize`/`kill`/`drain_output*`/`peek_tail` für beide.
- `open_terminal`: `channel_open_session` → `request_pty(xterm-256color, cols, rows)` →
  `request_shell` (oder `exec` der Resume-/cd-Zeile). Reader-Task: `ChannelMsg::{Data,ExtendedData}` →
  Queue (+Notify); `ExitStatus`/`Eof`/`Close` → EOF. `resize` → `window_change`.
- tmux-Resume + `remote_dir` bleiben: gleiche Kommandozeile wie heute (`sh_quote`/`tmux_session_name`
  bleiben), nur als Channel-`exec` statt ssh-Argv.
- `pty_spawn_remote` ([commands.rs](src-tauri/src/commands.rs)) wird `async` (Channel-Open awaitet).

### P4 — Exec-Layer auf `channel.exec` umstellen
- `ssh_exec.rs`: `RemoteExecManager::run` ruft `transport.exec(conn_id, cmd)` → (code, stdout, stderr)
  **direkt**; **Marker-Protokoll/base64/Temp-Files/Handshake entfernen**. Öffentliche API
  (`run`/`run_text`/`run_check`/`close`) **unverändert** → [git_remote.rs](src-tauri/src/git_remote.rs),
  fs-Remote ([fs_entries.rs](src-tauri/src/fs_entries.rs)) und Resume
  (`agent_remote_latest_session_id`) bleiben **unangetastet**.
- Binär-Reads (Bild/Video) werden sauberes stdout — `base64`-Remote-Pipe entfällt; Bytes direkt.

### P5 — Lifecycle, Teardown, Disconnect
- Lazy-Connect; Channel-Refcount pro Connection; Handle schließen bei letztem Channel/Workspace-Close
  (`remote_exec_close`-Pfad erweitern) und App-Exit (`kill_all` → alle Handles disconnecten).
- Disconnect-Erkennung: Reader-Task-Ende → Terminal-Fallback-Notice; optional Auto-Reconnect+Backoff.

### P6 — Connection-Pool (Skalierung über `MaxSessions`)
- Pro `connection_id` mehrere Handles, sobald Channel-Zahl eine Schwelle (z. B. 8) übersteigt; neue
  Channels round-robin/least-loaded verteilen. Löst große Grids trotz `MaxSessions`.

### P7 — Altlasten entfernen (wrapped ssh)
- Aus `pty_host.rs` entfernen: `build_ssh_args`, `Injector` (Passwort-PTY-Injection),
  `validate_remote`, `probe_failure`, `RemoteSpawnSpec`/`RemoteAuthMode`/`ResumeMode` ggf. nach
  `ssh_transport.rs` verschieben. `ServerAliveInterval`/`BatchMode`/`StrictHostKeyChecking`-Argbau weg.
  `sh_quote`/`tmux_session_name` **behalten** (Remote-Command-Bau in Exec/Git/Resume).
- `ssh_remote_test` ([ssh_remotes.rs](src-tauri/src/ssh_remotes.rs)) auf russh (connect+auth+`exec true`).

### P8 — Tests, Security-Review, Docs
- Unit: known_hosts TOFU (match/unknown/mismatch), Auth-Spec-Bau, Exec-Output (ohne Marker), Error-
  Klassifikation. Integration gegen echten Host (Key/Agent/Password): **eine** TCP-Verbindung mit N
  Channels (per `ss`/sshd-Log verifizieren), Resize, tmux-Resume, fs/git, Teardown, MaxSessions-Pool.
- Security: Host-Key-Pflicht, Secrets nur in-memory, known_hosts 0600, Agent-Socket-Handling.
- Docs: ssh-config/ProxyJump-Lücke; `MaxSessions`-Hinweis; Windows-Agent-Status.

## Tasks

> IDs als Commit-/PR-Anker. Reihenfolge ≈ Phasen; P0–P4 bilden den lauffähigen Kern, P5–P8 vervollständigen.

- [ ] **RUSSH-01** (P0) `russh`/`russh-keys` pinnen, `tokio`-Features (`rt-multi-thread`,`net`) ergänzen; baut.
- [ ] **RUSSH-02** (P0) `ssh_transport.rs`: `SshTransportManager` + `SshConnection` + `client::Config`
      (keepalive); `.manage(...)` in [lib.rs](src-tauri/src/lib.rs).
- [ ] **RUSSH-03** (P0) Connect + Handle-Caching pro `connection_id`; Driver-Task; Fehler-Typen.
- [ ] **RUSSH-04** (P1) `check_server_key` TOFU gegen known_hosts; `host_key_changed`-Fehlercode + i18n.
- [ ] **RUSSH-05** (P1) UI/Toast-Pfad für geänderten Host-Key + explizite Trust-Aktion.
- [ ] **RUSSH-06** (P2) Password-Auth (+ keyboard-interactive-Fallback).
- [ ] **RUSSH-07** (P2) Key-File + Passphrase-Auth (`russh-keys`).
- [ ] **RUSSH-08** (P2) SSH-Agent-Auth (Unix + Windows-Pipe verifizieren; sonst sauberer Fallback).
- [ ] **RUSSH-09** (P3) `PtySession`-Enum Local/Remote (oder Remote-Map); `write`/`resize`/`kill`/`drain`/`peek` für beide.
- [ ] **RUSSH-10** (P3) `open_terminal`: Channel + `request_pty` + shell/exec + Reader-Task → Queue/Notify.
- [ ] **RUSSH-11** (P3) Resize → `window_change`; tmux-Resume + `remote_dir` über Channel-`exec`.
- [ ] **RUSSH-12** (P3) `pty_spawn_remote` async auf Transport umstellen ([commands.rs](src-tauri/src/commands.rs)).
- [ ] **RUSSH-13** (P4) `RemoteExecManager::run` auf `channel.exec` (code/stdout/stderr); Marker-RPC/base64/Temp-Files/Handshake entfernen — **öffentliche API stabil**.
- [ ] **RUSSH-14** (P4) Binär-Reads (Bild/Video) ohne Remote-`base64` (direktes stdout) verifizieren.
- [ ] **RUSSH-15** (P5) Lazy-Connect + Channel-Refcount; Handle-Close bei Workspace-Close/Exit; `kill_all` → disconnect.
- [ ] **RUSSH-16** (P5) Disconnect-Notice im Terminal + optional Auto-Reconnect/Backoff.
- [ ] **RUSSH-17** (P6) Connection-Pool (mehrere Handles pro Connection ab Channel-Schwelle).
- [ ] **RUSSH-18** (P7) Wrapped-ssh-Altlasten entfernen (`build_ssh_args`/`Injector`/`validate_remote`/`probe_failure`); `sh_quote`/`tmux_session_name` behalten.
- [ ] **RUSSH-19** (P7) `ssh_remote_test` auf russh (connect+auth+`exec true`) mit Error-Klassifikation.
- [ ] **RUSSH-20** (P8) Unit-Tests (known_hosts/auth/exec/errors).
- [ ] **RUSSH-21** (P8) Integrationstest: 1 TCP / N Channels verifiziert; Resize/Resume/fs/git/Teardown/Pool.
- [ ] **RUSSH-22** (P8) Security-Review + Docs (ssh-config-Lücke, MaxSessions, Windows-Agent).

## Verification

1. `cargo check -p blxcode` und `cargo check -p blxcode-ui --target wasm32-unknown-unknown` kompilieren;
   **Frontend ungeändert** (API-Parität).
2. `cargo test -p blxcode` grün inkl. neuer Transport-Unit-Tests.
3. `cargo tauri dev` gegen echten Host (Key, Agent, Password):
   - **Multiplexing:** 9-Terminal-Workspace öffnen → auf dem Host **eine** TCP-Verbindung mit 9 Channels
     (`ss -tnp` / `sshd -ddd`-Log / `who`), **eine** Auth.
   - **Terminals:** Eingabe/Resize/Farben/`top` korrekt; Schließen beendet nur den jeweiligen Channel.
   - **Exec/fs/git:** Explorer, Preview (Text/Bild), Diff, Stage/Commit/Push, Graph — unverändert grün.
   - **Resume:** Keepalive-Modus → `claude --resume`; tmux-Modus reattacht live.
   - **Host-Key:** geänderter Key wird **abgelehnt** mit klarer Meldung; Trust-Aktion akzeptiert neu.
   - **Teardown:** Workspace-Close schließt die Handle (kein verwaister sshd-Channel); App-Exit trennt alle.
   - **Pool:** 16-Terminal-Grid funktioniert (zweite Handle ab Schwelle), auch bei `MaxSessions 10`.
