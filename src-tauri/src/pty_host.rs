//! Native PTY sessions for workspace terminal grid (Tauri only).
use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNavResult {
    pub cwd: String,
    pub log_line: String,
}

pub struct PtyManager {
    inner: Mutex<PtyInner>,
}

struct PtyInner {
    next_id: u64,
    sessions: HashMap<u64, PtySession>,
}

struct PtySession {
    pair: PtyPair,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    /// Shared so the reader-thread prompt injector (ssh password/passphrase)
    /// can write without racing the foreground `pty_write` path.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    output_ready: Arc<Condvar>,
    /// Non-destructive rolling tail of recent output. Filled in parallel
    /// with `queue` by the reader thread so the agent can `peek` without
    /// stealing bytes from the live terminal view.
    tail: Arc<Mutex<VecDeque<u8>>>,
}

const TAIL_CAP_BYTES: usize = 64 * 1024;

impl Default for PtyManager {
    fn default() -> Self {
        Self {
            inner: Mutex::new(PtyInner {
                next_id: 1,
                sessions: HashMap::new(),
            }),
        }
    }
}

impl PtyManager {
    pub fn spawn_session(
        &self,
        cwd: String,
        extra_env: Vec<(String, String)>,
    ) -> Result<u64, String> {
        let cwd = PathBuf::from(cwd.trim().trim_matches('\0'));
        if cwd.as_os_str().is_empty() {
            return Err("cwd empty".into());
        }
        if !cwd.is_dir() {
            return Err("cwd is not a directory".into());
        }

        let pty_system = native_pty_system();
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size).map_err(|e| e.to_string())?;

        #[cfg(windows)]
        let (shell, login_args): (String, &[&str]) = (
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()),
            &[],
        );
        #[cfg(not(windows))]
        let (shell, login_args): (String, &[&str]) = (
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
            &["-l"],
        );
        let mut cmd = CommandBuilder::new(&shell);
        cmd.args(login_args);
        cmd.cwd(&cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in extra_env {
            if k.is_empty() {
                continue;
            }
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn shell: {e}"))?;

        self.register_session(pair, child, None)
    }

    /// Shared session bookkeeping for both local and remote (ssh) PTYs:
    /// registers the session, spawns the output reader thread, and wires an
    /// optional prompt `injector` that answers ssh's password/passphrase
    /// prompt from the reader side.
    fn register_session(
        &self,
        pair: PtyPair,
        child: Box<dyn portable_pty::Child + Send + Sync>,
        mut injector: Option<Injector>,
    ) -> Result<u64, String> {
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().map_err(|e| e.to_string())?));

        let queue: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));
        let q_reader = Arc::clone(&queue);
        let output_ready = Arc::new(Condvar::new());
        let output_ready_reader = Arc::clone(&output_ready);
        let tail: Arc<Mutex<VecDeque<u8>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(TAIL_CAP_BYTES)));
        let tail_reader = Arc::clone(&tail);

        // Let the injector reply on the same master writer the foreground uses.
        if let Some(inj) = injector.as_mut() {
            inj.writer = Some(Arc::clone(&writer));
        }

        let id = {
            let mut g = self.inner.lock().map_err(|_| "pty lock")?;
            let id = g.next_id;
            g.next_id = g.next_id.saturating_add(1);
            g.sessions.insert(
                id,
                PtySession {
                    pair,
                    child: Mutex::new(child),
                    writer: Arc::clone(&writer),
                    queue: Arc::clone(&queue),
                    output_ready: Arc::clone(&output_ready),
                    tail: Arc::clone(&tail),
                },
            );
            id
        };

        thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = buf[..n].to_vec();
                        if let Some(inj) = injector.as_mut() {
                            inj.observe(&chunk);
                        }
                        if let Ok(mut t) = tail_reader.lock() {
                            for &b in &chunk {
                                if t.len() == TAIL_CAP_BYTES {
                                    t.pop_front();
                                }
                                t.push_back(b);
                            }
                        }
                        if let Ok(mut q) = q_reader.lock() {
                            q.push_back(chunk);
                            const MAX_QUEUED: usize = 256;
                            while q.len() > MAX_QUEUED {
                                q.pop_front();
                            }
                            output_ready_reader.notify_all();
                        }
                    }
                    Err(_) => {
                        output_ready_reader.notify_all();
                        break;
                    }
                }
            }
            output_ready_reader.notify_all();
        });

        Ok(id)
    }

    pub fn write(&self, session_id: u64, data: Vec<u8>) -> Result<(), String> {
        let g = self.inner.lock().map_err(|_| "pty lock")?;
        let s = g
            .sessions
            .get(&session_id)
            .ok_or_else(|| "unknown session".to_string())?;
        let mut w = s.writer.lock().map_err(|_| "writer lock")?;
        w.write_all(&data).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn resize(&self, session_id: u64, rows: u16, cols: u16) -> Result<(), String> {
        let g = self.inner.lock().map_err(|_| "pty lock")?;
        let s = g
            .sessions
            .get(&session_id)
            .ok_or_else(|| "unknown session".to_string())?;
        s.pair
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    /// Drain up to `max_bytes` of PTY output as base64 (may be empty).
    pub fn drain_output(&self, session_id: u64, max_bytes: usize) -> Result<String, String> {
        let g = self.inner.lock().map_err(|_| "pty lock")?;
        let s = g
            .sessions
            .get(&session_id)
            .ok_or_else(|| "unknown session".to_string())?;
        let cap = max_bytes.max(1).min(65536);
        let mut q = s.queue.lock().map_err(|_| "queue lock")?;
        let out = drain_queue(&mut q, cap);
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    }

    /// Drain output, waiting briefly in the backend for the PTY reader
    /// thread to produce data. This avoids frontend timer polling becoming
    /// the thing that "starts" terminals after a GUI event.
    pub fn drain_output_wait(
        &self,
        session_id: u64,
        max_bytes: usize,
        timeout_ms: u64,
    ) -> Result<String, String> {
        let (queue, output_ready) = {
            let g = self.inner.lock().map_err(|_| "pty lock")?;
            let s = g
                .sessions
                .get(&session_id)
                .ok_or_else(|| "unknown session".to_string())?;
            (Arc::clone(&s.queue), Arc::clone(&s.output_ready))
        };
        let cap = max_bytes.max(1).min(65536);
        let mut q = queue.lock().map_err(|_| "queue lock")?;
        if q.is_empty() && timeout_ms > 0 {
            let timeout = Duration::from_millis(timeout_ms.min(5_000));
            let (guard, _) = output_ready
                .wait_timeout(q, timeout)
                .map_err(|_| "queue lock")?;
            q = guard;
        }
        let out = drain_queue(&mut q, cap);
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    }

    /// Non-destructive read of the last `max_bytes` bytes of output as
    /// UTF-8 (lossy). Does NOT consume the live `queue` that the terminal
    /// view drains, so agent and human can read in parallel.
    pub fn peek_tail(&self, session_id: u64, max_bytes: usize) -> Result<String, String> {
        let g = self.inner.lock().map_err(|_| "pty lock")?;
        let s = g
            .sessions
            .get(&session_id)
            .ok_or_else(|| "unknown session".to_string())?;
        let cap = max_bytes.max(1).min(TAIL_CAP_BYTES);
        let t = s.tail.lock().map_err(|_| "tail lock")?;
        let len = t.len();
        let start = len.saturating_sub(cap);
        let mut out: Vec<u8> = Vec::with_capacity(len - start);
        for (i, b) in t.iter().enumerate() {
            if i >= start {
                out.push(*b);
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    pub fn kill(&self, session_id: u64) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|_| "pty lock")?;
        if let Some(s) = g.sessions.remove(&session_id) {
            if let Ok(mut ch) = s.child.lock() {
                let _ = ch.kill();
            }
        }
        Ok(())
    }

    /// Kill every live session. Called on app exit so no `ssh`/shell child
    /// processes are orphaned. For tmux-resume connections the remote session
    /// survives anyway because it lives in the server-side tmux, not the
    /// (now-killed) local ssh client.
    pub fn kill_all(&self) {
        if let Ok(mut g) = self.inner.lock() {
            for (_, s) in g.sessions.drain() {
                if let Ok(mut ch) = s.child.lock() {
                    let _ = ch.kill();
                }
            }
        }
    }

    fn child_finished(&self, session_id: u64) -> bool {
        if let Ok(g) = self.inner.lock() {
            if let Some(s) = g.sessions.get(&session_id) {
                if let Ok(mut ch) = s.child.lock() {
                    return matches!(ch.try_wait(), Ok(Some(_)));
                }
            }
        }
        false
    }

    /// Spawn a remote (ssh) PTY session. The local terminal is just the ssh
    /// client; the agent CLI runs on the remote host. Reuses all downstream
    /// terminal plumbing (drain/resize/kill).
    pub fn spawn_remote_session(
        &self,
        spec: RemoteSpawnSpec,
        extra_env: Vec<(String, String)>,
    ) -> Result<u64, String> {
        self.spawn_remote_inner(spec, extra_env, None)
    }

    fn spawn_remote_inner(
        &self,
        spec: RemoteSpawnSpec,
        extra_env: Vec<(String, String)>,
        remote_command_override: Option<String>,
    ) -> Result<u64, String> {
        validate_remote(&spec)?;
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size).map_err(|e| e.to_string())?;

        let mut cmd = CommandBuilder::new("ssh");
        for arg in build_ssh_args(&spec, remote_command_override.as_deref()) {
            cmd.arg(arg);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in extra_env {
            if k.is_empty() {
                continue;
            }
            cmd.env(k, v);
        }
        // ssh ignores cwd, but portable-pty wants a valid one.
        if let Some(home) = home_dir_string() {
            cmd.cwd(home);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn ssh: {e}"))?;

        let injector = Injector::for_auth(&spec.auth);
        self.register_session(pair, child, injector)
    }

    /// One-shot reachability/auth probe used by the "Test connection" button.
    /// Runs the exact same spawn path as a real terminal (so password/key
    /// injection is validated too), echoes a marker, and reports ok/err.
    pub fn run_remote_probe(&self, spec: RemoteSpawnSpec, timeout_ms: u64) -> Result<(), String> {
        const MARKER: &str = "__BLX_SSH_OK__";
        let id = self.spawn_remote_inner(spec, vec![], Some(format!("echo {MARKER}")))?;
        let deadline = Instant::now() + Duration::from_millis(timeout_ms.clamp(1_000, 30_000));
        let mut acc = String::new();
        let result = loop {
            if Instant::now() >= deadline {
                break Err("connection test timed out".to_string());
            }
            let b64 = self.drain_output_wait(id, 65536, 400)?;
            if !b64.is_empty() {
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
                    acc.push_str(&String::from_utf8_lossy(&bytes));
                }
            }
            if acc.contains(MARKER) {
                break Ok(());
            }
            if let Some(reason) = probe_failure(&acc) {
                break Err(reason);
            }
            if self.child_finished(id) {
                break Err(probe_failure(&acc)
                    .unwrap_or_else(|| "ssh exited before authentication completed".to_string()));
            }
        };
        let _ = self.kill(id);
        result
    }
}

// ---------------------------------------------------------------------------
// Remote (ssh) support
// ---------------------------------------------------------------------------

/// Authentication mode for a remote spawn. Secrets are carried inline so the
/// reader-thread injector can answer ssh's prompt; they never reach argv/env.
pub enum RemoteAuthMode {
    Password(String),
    Key {
        key_path: String,
        passphrase: Option<String>,
    },
    Agent,
}

/// How the remote session persists across reconnects.
pub enum ResumeMode {
    /// Attach/create a server-side tmux session keyed by terminal — survives
    /// app restart, idle, and workspace close.
    Tmux,
    /// Plain login shell; reconnect starts fresh. No remote dependency.
    KeepaliveOnly,
}

/// Everything needed to launch one ssh terminal.
pub struct RemoteSpawnSpec {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: RemoteAuthMode,
    pub resume: ResumeMode,
    pub remote_dir: Option<String>,
    /// Stable per-pane key (`{storage_key}:{slot}:{pane}`) → tmux session name.
    pub terminal_key: String,
}

impl RemoteSpawnSpec {
    fn default_remote_command(&self) -> Option<String> {
        match self.resume {
            ResumeMode::Tmux => {
                let session = tmux_session_name(&self.terminal_key);
                let cd = self
                    .remote_dir
                    .as_deref()
                    .filter(|d| !d.is_empty())
                    .map(|d| format!(" -c {}", sh_quote(d)))
                    .unwrap_or_default();
                // tmux if present (attach-or-create), else fall back to a login
                // shell so the terminal still works on hosts without tmux.
                Some(format!(
                    "command -v tmux >/dev/null 2>&1 && exec tmux new-session -A -s {}{} \
                     || exec ${{SHELL:-/bin/sh}} -l",
                    sh_quote(&session),
                    cd
                ))
            }
            ResumeMode::KeepaliveOnly => self
                .remote_dir
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| format!("cd {} ; exec ${{SHELL:-/bin/sh}} -l", sh_quote(d))),
        }
    }
}

/// Watches ssh client output for the password/passphrase prompt and replies
/// once. One-shot; bounded scan buffer so a noisy banner can't grow memory.
struct Injector {
    writer: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
    secret: String,
    needle: &'static str,
    done: bool,
    scan: String,
}

impl Injector {
    fn for_auth(auth: &RemoteAuthMode) -> Option<Self> {
        match auth {
            RemoteAuthMode::Password(p) if !p.is_empty() => Some(Self::new(p.clone(), "password")),
            RemoteAuthMode::Key {
                passphrase: Some(p),
                ..
            } if !p.is_empty() => Some(Self::new(p.clone(), "passphrase")),
            _ => None,
        }
    }

    fn new(secret: String, needle: &'static str) -> Self {
        Self {
            writer: None,
            secret,
            needle,
            done: false,
            scan: String::new(),
        }
    }

    fn observe(&mut self, chunk: &[u8]) {
        if self.done {
            return;
        }
        self.scan.push_str(&String::from_utf8_lossy(chunk));
        if self.scan.len() > 1024 {
            // Keep only the tail, on a char boundary, for prompt detection.
            self.scan = self
                .scan
                .chars()
                .rev()
                .take(256)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
        }
        let hay = self.scan.to_ascii_lowercase();
        if hay.contains(self.needle) && hay.contains(':') {
            if let Some(w) = &self.writer {
                if let Ok(mut w) = w.lock() {
                    let _ = w.write_all(self.secret.as_bytes());
                    let _ = w.write_all(b"\n");
                    let _ = w.flush();
                }
            }
            self.done = true;
            self.scan.clear();
        }
    }
}

fn validate_remote(spec: &RemoteSpawnSpec) -> Result<(), String> {
    let bad = |s: &str| s.is_empty() || s.chars().any(|c| c.is_whitespace() || c.is_control());
    if bad(&spec.host) {
        return Err("invalid host".into());
    }
    if bad(&spec.username) {
        return Err("invalid username".into());
    }
    if spec.port == 0 {
        return Err("invalid port".into());
    }
    if let RemoteAuthMode::Key { key_path, .. } = &spec.auth {
        if key_path.trim().is_empty() {
            return Err("key auth requires a key file path".into());
        }
    }
    Ok(())
}

/// Build the ssh argv. Secrets are never included — auth prompts are answered
/// via the PTY by `Injector`.
fn build_ssh_args(spec: &RemoteSpawnSpec, remote_command_override: Option<&str>) -> Vec<String> {
    fn push_opt(args: &mut Vec<String>, k: &str) {
        args.push("-o".into());
        args.push(k.to_string());
    }

    let mut args: Vec<String> = Vec::new();
    args.push("-p".into());
    args.push(spec.port.to_string());

    push_opt(&mut args, "ServerAliveInterval=30");
    push_opt(&mut args, "ServerAliveCountMax=3");
    push_opt(&mut args, "ConnectTimeout=10");
    push_opt(&mut args, "StrictHostKeyChecking=accept-new");

    match &spec.auth {
        RemoteAuthMode::Password(_) => {
            push_opt(&mut args, "PreferredAuthentications=password,keyboard-interactive");
            push_opt(&mut args, "PubkeyAuthentication=no");
        }
        RemoteAuthMode::Key {
            key_path,
            passphrase,
        } => {
            args.push("-i".into());
            args.push(key_path.clone());
            push_opt(&mut args, "IdentitiesOnly=yes");
            // No passphrase → fail fast instead of hanging on a hidden prompt.
            if passphrase.as_deref().map(str::is_empty).unwrap_or(true) {
                push_opt(&mut args, "BatchMode=yes");
            }
        }
        RemoteAuthMode::Agent => {
            push_opt(&mut args, "BatchMode=yes");
        }
    }

    let remote_command =
        remote_command_override
            .map(str::to_string)
            .or_else(|| spec.default_remote_command());
    if remote_command.is_some() {
        // Force remote PTY allocation for tmux / interactive shell.
        args.push("-t".into());
    }
    args.push(format!("{}@{}", spec.username, spec.host));
    if let Some(cmd) = remote_command {
        args.push(cmd);
    }
    args
}

/// Map a terminal key to a tmux-safe session name (alnum/underscore only).
fn tmux_session_name(terminal_key: &str) -> String {
    let mut s = String::with_capacity(terminal_key.len() + 4);
    s.push_str("blx_");
    for c in terminal_key.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    s
}

/// POSIX single-quote a string for safe interpolation into a remote command.
fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Detect a fatal ssh failure in accumulated output, returning a short reason.
fn probe_failure(output: &str) -> Option<String> {
    const FAILURES: &[(&str, &str)] = &[
        ("permission denied", "authentication failed (permission denied)"),
        ("could not resolve hostname", "could not resolve hostname"),
        ("name or service not known", "could not resolve hostname"),
        ("connection refused", "connection refused"),
        ("connection timed out", "connection timed out"),
        ("operation timed out", "connection timed out"),
        ("connection closed", "connection closed by remote host"),
        ("host key verification failed", "host key verification failed"),
        ("no such file or directory", "key file not found"),
        ("too many authentication failures", "too many authentication failures"),
    ];
    let hay = output.to_ascii_lowercase();
    FAILURES
        .iter()
        .find(|(needle, _)| hay.contains(needle))
        .map(|(_, reason)| (*reason).to_string())
}

fn drain_queue(q: &mut VecDeque<Vec<u8>>, cap: usize) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    while out.len() < cap {
        let Some(chunk) = q.pop_front() else {
            break;
        };
        let remain = cap - out.len();
        if chunk.len() <= remain {
            out.extend_from_slice(&chunk);
        } else {
            out.extend_from_slice(&chunk[..remain]);
            let rest = chunk[remain..].to_vec();
            q.push_front(rest);
            break;
        }
    }
    out
}

fn home_dir_string() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|s| !s.is_empty())
}

/// Safe `cd`-style navigation: only `cd` with one argument (or `cd` / `cd ~`), plus empty line = pwd.
pub fn path_nav_exec(base: String, line: String) -> Result<PathNavResult, String> {
    let base_pb = if base.trim().is_empty() {
        std::env::current_dir().map_err(|e| e.to_string())?
    } else {
        PathBuf::from(base.trim())
    };
    let base_canon = base_pb.canonicalize().unwrap_or(base_pb);
    let line = line.trim();
    if line.is_empty() {
        let cwd = base_canon.to_string_lossy().into_owned();
        return Ok(PathNavResult {
            cwd: cwd.clone(),
            log_line: cwd,
        });
    }
    let lower = line.to_ascii_lowercase();
    let rest = if lower == "cd" {
        ""
    } else if lower.starts_with("cd ") {
        line[3..].trim()
    } else {
        return Err("only 'cd' is supported".into());
    };
    let target = resolve_cd_path(&base_canon, rest)?;
    let cwd = target.to_string_lossy().into_owned();
    Ok(PathNavResult {
        cwd: cwd.clone(),
        log_line: format!("cd -> {cwd}"),
    })
}

fn resolve_cd_path(base: &Path, arg: &str) -> Result<PathBuf, String> {
    let arg = arg.trim();
    if arg.is_empty() || arg == "~" {
        let h = home_dir_string().ok_or_else(|| "HOME not set".to_string())?;
        return PathBuf::from(h).canonicalize().map_err(|e| e.to_string());
    }
    let joined = if let Some(stripped) = arg.strip_prefix("~/") {
        let h = home_dir_string().ok_or_else(|| "HOME not set".to_string())?;
        Path::new(&h).join(stripped)
    } else if Path::new(arg).is_absolute() {
        PathBuf::from(arg)
    } else if arg == ".." {
        base.join("..")
    } else if arg == "." {
        base.to_path_buf()
    } else {
        base.join(arg)
    };
    joined
        .canonicalize()
        .map_err(|e| format!("cd: {e}"))
        .and_then(|p| {
            if p.is_dir() {
                Ok(p)
            } else {
                Err("not a directory".into())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(auth: RemoteAuthMode, resume: ResumeMode) -> RemoteSpawnSpec {
        RemoteSpawnSpec {
            host: "example.com".into(),
            port: 2222,
            username: "deploy".into(),
            auth,
            resume,
            remote_dir: None,
            terminal_key: "abc123:1:1001".into(),
        }
    }

    #[test]
    fn ssh_args_password_avoids_batchmode_and_keys() {
        let s = spec(
            RemoteAuthMode::Password("secret".into()),
            ResumeMode::KeepaliveOnly,
        );
        let args = build_ssh_args(&s, None);
        let joined = args.join(" ");
        assert!(joined.contains("-p 2222"));
        assert!(joined.contains("deploy@example.com"));
        assert!(joined.contains("ServerAliveInterval=30"));
        assert!(joined.contains("PubkeyAuthentication=no"));
        assert!(!joined.contains("BatchMode=yes"));
        // The password must never leak into argv.
        assert!(!joined.contains("secret"));
    }

    #[test]
    fn ssh_args_agent_uses_batchmode() {
        let s = spec(RemoteAuthMode::Agent, ResumeMode::KeepaliveOnly);
        let args = build_ssh_args(&s, None);
        assert!(args.join(" ").contains("BatchMode=yes"));
    }

    #[test]
    fn ssh_args_key_with_passphrase_skips_batchmode() {
        let with_pp = spec(
            RemoteAuthMode::Key {
                key_path: "/home/u/.ssh/id_ed25519".into(),
                passphrase: Some("pp".into()),
            },
            ResumeMode::KeepaliveOnly,
        );
        let joined = build_ssh_args(&with_pp, None).join(" ");
        assert!(joined.contains("-i /home/u/.ssh/id_ed25519"));
        assert!(joined.contains("IdentitiesOnly=yes"));
        assert!(!joined.contains("BatchMode=yes"));
        assert!(!joined.contains("pp"));

        let no_pp = spec(
            RemoteAuthMode::Key {
                key_path: "/k".into(),
                passphrase: None,
            },
            ResumeMode::KeepaliveOnly,
        );
        assert!(build_ssh_args(&no_pp, None).join(" ").contains("BatchMode=yes"));
    }

    #[test]
    fn tmux_resume_emits_attach_or_create_with_stable_name() {
        let s = spec(RemoteAuthMode::Agent, ResumeMode::Tmux);
        let args = build_ssh_args(&s, None);
        // -t forces remote pty; the remote command attaches/creates tmux.
        assert!(args.iter().any(|a| a == "-t"));
        let cmd = args.last().expect("remote command present");
        assert!(cmd.contains("tmux new-session -A -s 'blx_abc123_1_1001'"));
        assert!(cmd.contains("exec ${SHELL:-/bin/sh} -l"));
    }

    #[test]
    fn tmux_session_name_sanitizes() {
        assert_eq!(tmux_session_name("ab:1:2"), "blx_ab_1_2");
        assert_eq!(tmux_session_name("a/b c"), "blx_a_b_c");
    }

    #[test]
    fn sh_quote_escapes_single_quotes() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn probe_failure_detects_common_errors() {
        assert!(probe_failure("user@host: Permission denied (publickey)").is_some());
        assert!(probe_failure("ssh: Could not resolve hostname foo").is_some());
        assert!(probe_failure("__BLX_SSH_OK__\n").is_none());
    }

    #[test]
    fn validate_remote_rejects_bad_fields() {
        let mut s = spec(RemoteAuthMode::Agent, ResumeMode::KeepaliveOnly);
        assert!(validate_remote(&s).is_ok());
        s.host = "bad host".into();
        assert!(validate_remote(&s).is_err());
    }
}
