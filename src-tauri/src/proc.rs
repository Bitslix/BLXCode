//! Subprocess helper. All child processes (git, rg, npm, tar, …) must be
//! created through [`command`] so the Windows console-window flash is
//! suppressed. Without `CREATE_NO_WINDOW`, every short-lived child pops a
//! console window on Windows; under a busy git-status watcher that becomes a
//! storm of flashing terminals that freezes the UI.

use std::ffi::OsStr;
use std::process::Command;

/// Build a [`Command`] for `program` with platform-appropriate flags.
///
/// On Windows this sets `CREATE_NO_WINDOW` (`0x0800_0000`) so console
/// subprocesses do not spawn a visible terminal window. On other platforms it
/// is a plain [`Command::new`].
#[must_use]
pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    let cmd = Command::new(program);
    apply_no_window(cmd)
}

#[cfg(windows)]
fn apply_no_window(mut cmd: Command) -> Command {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NO_WINDOW` — child runs without allocating a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
#[inline]
fn apply_no_window(cmd: Command) -> Command {
    cmd
}

/// Run blocking work (e.g. a git subprocess) off the main thread.
///
/// Synchronous `#[tauri::command]` handlers execute on the main thread, so a
/// blocking `Command::output()` there stalls the event loop and makes the
/// window stutter (visible when dragging it). Wrapping the work in
/// [`tauri::async_runtime::spawn_blocking`] moves it to a dedicated blocking
/// pool; the command stays `async` and the UI thread keeps pumping events.
pub async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(f).await {
        Ok(result) => result,
        Err(e) => Err(format!("background task failed: {e}")),
    }
}
