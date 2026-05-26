//! Native PTY sessions for workspace terminal grid (Tauri only).
use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

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
    writer: Mutex<Box<dyn Write + Send>>,
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

        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let queue: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));
        let q_reader = Arc::clone(&queue);
        let output_ready = Arc::new(Condvar::new());
        let output_ready_reader = Arc::clone(&output_ready);
        let tail: Arc<Mutex<VecDeque<u8>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(TAIL_CAP_BYTES)));
        let tail_reader = Arc::clone(&tail);

        let id = {
            let mut g = self.inner.lock().map_err(|_| "pty lock")?;
            let id = g.next_id;
            g.next_id = g.next_id.saturating_add(1);
            g.sessions.insert(
                id,
                PtySession {
                    pair,
                    child: Mutex::new(child),
                    writer: Mutex::new(writer),
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
