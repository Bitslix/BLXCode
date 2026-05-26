//! Non-interactive shell execution in the workspace with allowlist and child registry.

use crate::agent::environment;
use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use serde_json::Value;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const EXEC_TIMEOUT: Duration = Duration::from_secs(120);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct ChildRegistry {
    children: HashMap<u64, Child>,
}

static REGISTRY: OnceLock<Mutex<ChildRegistry>> = OnceLock::new();

fn registry() -> &'static Mutex<ChildRegistry> {
    REGISTRY.get_or_init(|| {
        Mutex::new(ChildRegistry {
            children: HashMap::new(),
        })
    })
}

pub fn kill_all_children() {
    if let Ok(mut reg) = registry().lock() {
        for (_, mut child) in reg.children.drain() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn read_only_allowlist(program: &str) -> bool {
    matches!(
        program,
        "ls" | "pwd"
            | "cat"
            | "head"
            | "tail"
            | "wc"
            | "file"
            | "which"
            | "env"
            | "rg"
            | "fd"
            | "find"
            | "tree"
            | "stat"
            | "du"
            | "df"
            | "node"
            | "npm"
            | "cargo"
            | "git"
    )
}

fn git_subcommand_read_only(args: &[&str]) -> bool {
    let Some(sub) = args.first().copied() else {
        return false;
    };
    matches!(
        sub,
        "status" | "log" | "diff" | "show" | "branch" | "ls-files" | "rev-parse" | "describe"
    )
}

fn bash_command_allowed(command: &str, writes: bool) -> bool {
    if writes {
        return true;
    }
    let parts: Vec<&str> = command.split_whitespace().collect();
    let Some(bin) = parts.first().copied() else {
        return false;
    };
    if !read_only_allowlist(bin) {
        return false;
    }
    if bin == "git" {
        return git_subcommand_read_only(&parts[1..]);
    }
    if bin == "find" {
        return !command.contains("-exec")
            && !command.contains("-delete")
            && !command.contains("-prune");
    }
    true
}

pub fn tool_shell_exec(
    args: &Value,
    root: Option<&WorkspaceRootGuard>,
    default_writes: bool,
) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(out) = environment::require_environment(&root.as_str()) {
        return out;
    }

    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return ToolOutcome {
                ok: false,
                content: "missing command".into(),
            };
        }
    };

    let writes = args
        .get("writes")
        .and_then(|v| v.as_bool())
        .unwrap_or(default_writes);

    if !bash_command_allowed(command, writes) {
        return ToolOutcome {
            ok: false,
            content: format!("command not allowed in read-only mode: {command}"),
        };
    }

    let shell = if cfg!(windows) { "powershell" } else { "bash" };
    let shell_arg: Vec<&str> = if cfg!(windows) {
        vec!["-NoProfile", "-Command", command]
    } else {
        vec!["-lc", command]
    };

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let child = match Command::new(shell)
        .args(&shell_arg)
        .current_dir(root.as_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: format!("spawn failed: {e}"),
            };
        }
    };

    if let Ok(mut reg) = registry().lock() {
        reg.children.insert(id, child);
    }

    let wait_result: Option<Result<std::process::Output, String>> = registry()
        .lock()
        .ok()
        .and_then(|mut r| r.children.remove(&id))
        .map(|mut c| {
            let handle = thread::spawn(move || {
                let start = std::time::Instant::now();
                loop {
                    match c.try_wait() {
                        Ok(Some(_)) => break c.wait_with_output().map_err(|e| e.to_string()),
                        Ok(None) => {}
                        Err(e) => return Err(e.to_string()),
                    }
                    if start.elapsed() > EXEC_TIMEOUT {
                        let _ = c.kill();
                        return Err("timeout".into());
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
            match handle.join() {
                Ok(r) => r,
                Err(_) => Err("command thread panicked".into()),
            }
        });

    match wait_result {
        Some(Ok(output)) => {
            let mut body = String::from_utf8_lossy(&output.stdout).into_owned();
            if !output.stderr.is_empty() {
                if !body.is_empty() {
                    body.push_str("\n--- stderr ---\n");
                }
                body.push_str(&String::from_utf8_lossy(&output.stderr));
            }
            if body.len() > MAX_OUTPUT_BYTES {
                body.truncate(MAX_OUTPUT_BYTES);
                body.push_str("\n… (output truncated)");
            }
            ToolOutcome {
                ok: output.status.success(),
                content: body,
            }
        }
        Some(Err(msg)) => {
            if msg == "timeout" {
                kill_all_children();
            }
            ToolOutcome {
                ok: false,
                content: msg,
            }
        }
        None => ToolOutcome {
            ok: false,
            content: "child process lost".into(),
        },
    }
}
