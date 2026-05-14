# sessions.json: Tauri-File-Lock (Mutex)

**Status:** Spezifikation + Copy-Paste-Snippets liegen hier; die Rust-Dateien im Repo sind wegen **Plan-Modus** noch nicht automatisch geändert. Nach **Agent-Modus** / Ende Plan-Modus dieselben Snippets anwenden oder Cursor bitten, aus dieser Datei zu patchen.

Ziel: Alle Read-Modify-Write-Zugriffe auf `sessions.json` im Tauri-Prozess **serialisieren**, damit parallele `invoke`-Aufrufe (z. B. `workbench_load_sessions` + `workbench_drop_sessions`) sich nicht gegenseitig überschreiben.

## 1. `src-tauri/src/workbench_state.rs`

- Nach den `use`-Zeilen: `use std::sync::Mutex;`
- Direkt unter `const SESSIONS_FILE`:

```rust
/// Serialises every `sessions.json` load / update from this process so
/// overlapping Tauri commands cannot clobber each other's read-modify-write.
pub struct WorkbenchSessionsFileLock(pub Mutex<()>);

impl Default for WorkbenchSessionsFileLock {
    fn default() -> Self {
        Self(Mutex::new(()))
    }
}
```

- Modul-Docstring um einen Satz ergänzen: alle RMW auf `SESSIONS_FILE` müssen diesen Lock per `State` halten.

- **`workbench_load_sessions`**: zweiter Parameter

```rust
lock: tauri::State<'_, WorkbenchSessionsFileLock>,
```

  Am Anfang des Rumpfs (vor `sessions_path_impl`):

```rust
let _guard = lock
    .0
    .lock()
    .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
```

- **`workbench_drop_sessions`**: ebenfalls `State<WorkbenchSessionsFileLock>`, gleicher Lock-Guard **ganz am Anfang** (nach Signatur, vor `if prefix.is_empty()` — damit auch leeres Prefix den Lock nicht umgeht; optional Lock erst nach empty-check, dann nur bei non-empty locken; **empfohlen**: Lock immer, damit Load+Drop nie ohne Barriere kreuzen).

## 2. `src-tauri/src/lib.rs`

- Import erweitern:

```rust
use workbench_state::{
    agent_session_exists,
    workbench_drop_sessions,
    workbench_load_sessions,
    workbench_load_state,
    workbench_save_state,
    workbench_sessions_path,
    WorkbenchSessionsFileLock,
};
```

- In `tauri::Builder::default()` vor `.run`:

```rust
.manage(WorkbenchSessionsFileLock::default())
```

## 3. Spätere Commands (Extract/Merge aus Welcome-Plan)

Jeder neue `#[tauri::command]`, der `sessions.json` liest oder schreibt, erhält dieselbe `State<'_, WorkbenchSessionsFileLock>` und hält den Guard für die gesamte RMW-Transaktion.

## 4. Grenzen

- Externe Schreiber (z. B. Shell-Hooks) sieht der Mutex **nicht**; er verhindert nur Races **innerhalb** des Tauri-Prozesses.
- `workbench_sessions_path` bleibt unverändert (nur Pfadstring, kein I/O).
