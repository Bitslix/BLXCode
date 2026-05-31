//! Persistent SSH "exec channel" — runs commands and reads files on a remote
//! host over a single long-lived, authenticated ssh session (separate from
//! the interactive workspace terminals). This is the shared foundation for
//! remote file browsing, remote git, and remote session resume.
//!
//! Windows OpenSSH has no `ControlMaster` multiplexing, so instead of one
//! short-lived `ssh` per operation we keep one authenticated login shell per
//! connection (reusing `pty_host`'s PTY + password injection) and drive it as
//! a marker-framed request/response RPC: each call writes a one-line command
//! that captures stdout/stderr to temp files and base64-streams them back
//! between unique markers. Base64 + temp files make the protocol robust to
//! PTY echo/newline mangling and to binary payloads.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use tauri::AppHandle;

use crate::pty_host::PtyManager;
use crate::ssh_remotes;

/// Default per-command timeout. Git network ops use the longer variant.
pub const EXEC_TIMEOUT_MS: u64 = 30_000;
pub const EXEC_TIMEOUT_LONG_MS: u64 = 180_000;

/// Result of one remote command.
pub struct ExecOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ExecOutput {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    pub fn stdout_string(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_string(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

struct ExecChannel {
    session_id: u64,
    /// Serializes the request/response RPC — one in-flight command per channel.
    cmd_lock: Mutex<()>,
}

#[derive(Default)]
pub struct RemoteExecManager {
    channels: Mutex<HashMap<String, Arc<ExecChannel>>>,
}

impl RemoteExecManager {
    /// Run a command on the remote, returning captured stdout/stderr + exit code.
    pub fn run(
        &self,
        app: &AppHandle,
        pty: &PtyManager,
        connection_id: &str,
        command: &str,
        timeout_ms: u64,
    ) -> Result<ExecOutput, String> {
        let channel = self.ensure_channel(app, pty, connection_id)?;
        let _guard = channel
            .cmd_lock
            .lock()
            .map_err(|_| "exec channel lock".to_string())?;
        match run_on_session(pty, channel.session_id, command, timeout_ms) {
            Ok(out) => Ok(out),
            Err(err) => {
                // The channel is probably dead (ssh exited, timeout desynced the
                // stream). Drop it so the next call re-authenticates.
                drop(_guard);
                self.drop_channel(pty, connection_id);
                Err(err)
            }
        }
    }

    /// Convenience: run a command, require exit 0, return stdout as UTF-8.
    pub fn run_text(
        &self,
        app: &AppHandle,
        pty: &PtyManager,
        connection_id: &str,
        command: &str,
        timeout_ms: u64,
    ) -> Result<String, String> {
        let out = self.run(app, pty, connection_id, command, timeout_ms)?;
        if out.ok() {
            Ok(out.stdout_string())
        } else {
            Err(remote_error(&out))
        }
    }

    /// Run a command, require exit 0, discard stdout.
    pub fn run_check(
        &self,
        app: &AppHandle,
        pty: &PtyManager,
        connection_id: &str,
        command: &str,
        timeout_ms: u64,
    ) -> Result<(), String> {
        let out = self.run(app, pty, connection_id, command, timeout_ms)?;
        if out.ok() {
            Ok(())
        } else {
            Err(remote_error(&out))
        }
    }

    /// Close + forget a connection's channel (workspace close / app exit).
    pub fn close(&self, pty: &PtyManager, connection_id: &str) {
        self.drop_channel(pty, connection_id);
    }

    fn drop_channel(&self, pty: &PtyManager, connection_id: &str) {
        let removed = self
            .channels
            .lock()
            .ok()
            .and_then(|mut m| m.remove(connection_id));
        if let Some(ch) = removed {
            let _ = pty.kill(ch.session_id);
        }
    }

    fn ensure_channel(
        &self,
        app: &AppHandle,
        pty: &PtyManager,
        connection_id: &str,
    ) -> Result<Arc<ExecChannel>, String> {
        // Fast path: already open.
        if let Some(existing) = self
            .channels
            .lock()
            .map_err(|_| "exec map lock".to_string())?
            .get(connection_id)
            .cloned()
        {
            return Ok(existing);
        }

        // Open + authenticate + handshake without holding the map lock (blocking).
        let spec = ssh_remotes::resolve_spec(app, connection_id, format!("exec:{connection_id}"))?;
        let session_id = pty.spawn_remote_session(spec, Vec::new())?;
        if let Err(err) = handshake(pty, session_id) {
            let _ = pty.kill(session_id);
            return Err(err);
        }
        let channel = Arc::new(ExecChannel {
            session_id,
            cmd_lock: Mutex::new(()),
        });

        // Install, but lose the race gracefully if another caller opened one.
        let mut map = self
            .channels
            .lock()
            .map_err(|_| "exec map lock".to_string())?;
        if let Some(existing) = map.get(connection_id).cloned() {
            drop(map);
            let _ = pty.kill(session_id);
            return Ok(existing);
        }
        map.insert(connection_id.to_string(), Arc::clone(&channel));
        Ok(channel)
    }
}

/// Disable PTY echo and wait for a sentinel so we know the authenticated shell
/// prompt is ready and subsequent command lines won't be echoed back into the
/// output stream (which would otherwise collide with our markers).
fn handshake(pty: &PtyManager, session_id: u64) -> Result<(), String> {
    let nonce = new_nonce();
    let marker = format!("__BLX_HS_{nonce}__");
    let line = format!("stty -echo 2>/dev/null; printf '{marker}\\n'\n");
    pty.write(session_id, line.into_bytes())?;
    let deadline = Instant::now() + Duration::from_millis(EXEC_TIMEOUT_MS);
    drain_until(pty, session_id, marker.as_bytes(), deadline).map(|_| ())
}

/// Execute one command via the marker-framed protocol on an open session.
fn run_on_session(
    pty: &PtyManager,
    session_id: u64,
    command: &str,
    timeout_ms: u64,
) -> Result<ExecOutput, String> {
    let nonce = new_nonce();
    let s = format!("<<BLX:{nonce}:S>>");
    let m = format!("<<BLX:{nonce}:M>>");
    let e = format!("<<BLX:{nonce}:E>>");
    // Single line: capture stdout/stderr to temp files, then frame rc + both
    // base64 blobs between unique markers.
    let line = format!(
        "__bt=$(mktemp); __be=$(mktemp); {{ {command} ; }} >\"$__bt\" 2>\"$__be\"; __brc=$?; \
         printf '\\n{s}\\n%s\\n' \"$__brc\"; base64 <\"$__bt\"; printf '\\n{m}\\n'; \
         base64 <\"$__be\"; printf '\\n{e}\\n'; rm -f \"$__bt\" \"$__be\"\n"
    );
    pty.write(session_id, line.into_bytes())?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1_000));
    let buf = drain_until(pty, session_id, e.as_bytes(), deadline)?;
    parse_frame(&buf, &s, &m, &e)
}

/// Accumulate session output until `needle` appears (or the deadline passes).
fn drain_until(
    pty: &PtyManager,
    session_id: u64,
    needle: &[u8],
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    const MAX_BUF: usize = 160 * 1024 * 1024;
    let mut buf: Vec<u8> = Vec::new();
    let mut search_from = 0usize;
    loop {
        if let Some(pos) = find_subslice(&buf[search_from..], needle) {
            // Found; return everything up to and including the needle.
            let end = search_from + pos + needle.len();
            buf.truncate(end);
            return Ok(buf);
        }
        // Next scan can start a little before the buffer end to catch a needle
        // split across two drains.
        search_from = buf.len().saturating_sub(needle.len());

        if Instant::now() >= deadline {
            return Err("remote exec timed out".into());
        }
        let b64 = pty.drain_output_wait(session_id, 65536, 300)?;
        if !b64.is_empty() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64.as_bytes())
                .map_err(|e| format!("exec drain decode: {e}"))?;
            buf.extend_from_slice(&bytes);
            if buf.len() > MAX_BUF {
                return Err("remote exec output too large".into());
            }
        }
    }
}

fn parse_frame(buf: &[u8], s: &str, m: &str, e: &str) -> Result<ExecOutput, String> {
    let start = find_subslice(buf, s.as_bytes())
        .ok_or_else(|| "remote exec: start marker missing".to_string())?;
    // Skip past the start marker and its trailing newline.
    let after_s = start + s.len();
    let after_s = skip_one_newline(buf, after_s);

    // The exit code occupies the next line.
    let rc_end = find_subslice(&buf[after_s..], b"\n")
        .map(|p| after_s + p)
        .ok_or_else(|| "remote exec: rc line missing".to_string())?;
    let code: i32 = String::from_utf8_lossy(&buf[after_s..rc_end])
        .trim()
        .parse()
        .map_err(|_| "remote exec: bad rc".to_string())?;

    let mid = find_subslice(&buf[rc_end..], m.as_bytes())
        .map(|p| rc_end + p)
        .ok_or_else(|| "remote exec: mid marker missing".to_string())?;
    let end = find_subslice(&buf[mid..], e.as_bytes())
        .map(|p| mid + p)
        .ok_or_else(|| "remote exec: end marker missing".to_string())?;

    let stdout = decode_b64_blob(&buf[rc_end..mid])?;
    let stderr = decode_b64_blob(&buf[mid + m.len()..end])?;
    Ok(ExecOutput {
        code,
        stdout,
        stderr,
    })
}

/// Decode a base64 blob, ignoring any ASCII whitespace (the `base64` tool wraps
/// lines and the PTY may inject CRs).
fn decode_b64_blob(raw: &[u8]) -> Result<Vec<u8>, String> {
    let cleaned: Vec<u8> = raw
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .map_err(|e| format!("remote exec base64: {e}"))
}

fn skip_one_newline(buf: &[u8], mut i: usize) -> usize {
    if buf.get(i) == Some(&b'\r') {
        i += 1;
    }
    if buf.get(i) == Some(&b'\n') {
        i += 1;
    }
    i
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn remote_error(out: &ExecOutput) -> String {
    let err = out.stderr_string();
    let err = err.trim();
    if err.is_empty() {
        format!("remote command failed (exit {})", out.code)
    } else {
        format!("remote command failed (exit {}): {err}", out.code)
    }
}

fn new_nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Close the exec channel for a connection (called when its last remote
/// workspace closes). Idempotent; the underlying ssh session is killed.
#[tauri::command]
pub fn remote_exec_close(
    pty: tauri::State<'_, PtyManager>,
    exec: tauri::State<'_, RemoteExecManager>,
    connection_id: String,
) {
    exec.close(&pty, &connection_id);
}

/// Discover the newest agent-CLI session id for a remote cwd, over the exec
/// channel. No remote hooks are installed, so resume uses the
/// "latest session for this cwd" heuristic (tmux mode handles exact live
/// resume). Currently implemented for Claude; other agents return `None`
/// (fresh session), which is the safe default.
#[tauri::command]
pub fn agent_remote_latest_session_id(
    app: AppHandle,
    pty: tauri::State<'_, PtyManager>,
    exec: tauri::State<'_, RemoteExecManager>,
    connection_id: String,
    agent: String,
    cwd: String,
) -> Result<Option<String>, String> {
    match agent.as_str() {
        "claude" => remote_latest_claude(&app, &pty, &exec, &connection_id, &cwd),
        _ => Ok(None),
    }
}

/// Mirror of the local `latest_claude_session_id` over SSH:
/// `~/.claude/projects/<cwd-with-non-alnum→dash>/<id>.jsonl`, newest by mtime.
fn remote_latest_claude(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    cwd: &str,
) -> Result<Option<String>, String> {
    // Encoded dir is alnum + dashes only → safe to interpolate unquoted (the
    // glob `*` must stay unquoted to expand on the remote).
    let encoded: String = cwd
        .trim_end_matches(['/', '\\'])
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let cmd = format!("ls -t ~/.claude/projects/{encoded}/*.jsonl 2>/dev/null | head -1");
    let out = exec.run(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    if !out.ok() {
        return Ok(None);
    }
    let line = out.stdout_string();
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let base = line.rsplit(['/', '\\']).next().unwrap_or(line);
    let id = base.strip_suffix(".jsonl").unwrap_or(base).trim();
    Ok((!id.is_empty()).then(|| id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(nonce: &str, rc: i32, stdout: &[u8], stderr: &[u8]) -> Vec<u8> {
        let s = format!("<<BLX:{nonce}:S>>");
        let m = format!("<<BLX:{nonce}:M>>");
        let e = format!("<<BLX:{nonce}:E>>");
        let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
        // Prepend banner noise + an echoed line containing marker-like text to
        // prove the parser locks onto the real (nonce'd) markers.
        format!(
            "login banner\nuser@host:~$ stuff\n{s}\n{rc}\n{}\n{m}\n{}\n{e}\n",
            b64(stdout),
            b64(stderr),
        )
        .into_bytes()
    }

    #[test]
    fn parse_roundtrips_stdout_stderr_rc() {
        let nonce = "abc123";
        let s = format!("<<BLX:{nonce}:S>>");
        let m = format!("<<BLX:{nonce}:M>>");
        let e = format!("<<BLX:{nonce}:E>>");
        let buf = frame(nonce, 7, b"hello world\n", b"some error");
        let out = parse_frame(&buf, &s, &m, &e).expect("parse");
        assert_eq!(out.code, 7);
        assert_eq!(out.stdout, b"hello world\n");
        assert_eq!(out.stderr, b"some error");
        assert!(!out.ok());
    }

    #[test]
    fn parse_handles_binary_stdout() {
        let nonce = "deadbeef";
        let s = format!("<<BLX:{nonce}:S>>");
        let m = format!("<<BLX:{nonce}:M>>");
        let e = format!("<<BLX:{nonce}:E>>");
        let payload: Vec<u8> = (0u8..=255).cycle().take(1000).collect();
        let buf = frame(nonce, 0, &payload, b"");
        let out = parse_frame(&buf, &s, &m, &e).expect("parse");
        assert!(out.ok());
        assert_eq!(out.stdout, payload);
        assert!(out.stderr.is_empty());
    }

    #[test]
    fn decode_b64_ignores_whitespace_wrapping() {
        let raw = b"aGVs\r\nbG8g\nd29ybGQ=\n";
        assert_eq!(decode_b64_blob(raw).unwrap(), b"hello world");
    }

    #[test]
    fn find_subslice_basic() {
        assert_eq!(find_subslice(b"hello", b"ll"), Some(2));
        assert_eq!(find_subslice(b"hello", b"x"), None);
    }
}
