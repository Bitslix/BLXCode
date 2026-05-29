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
