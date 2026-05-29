//! Shared remote-sync state for the sidebar git sections. Fetch/Pull live in
//! the Git Commits (graph) toolbar and Push in the File Diff toolbar; both
//! read the same branch status and serialize through one busy flag so an
//! operation started in one section disables the buttons in the other.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    git_fetch, git_pull, git_push, git_sync_status, SyncStatus, GIT_MISSING_CODE,
};
use crate::workbench::toast::ToastService;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SyncOp {
    Fetch,
    Pull,
    Push,
}

/// Context-provided handle holding the shared sync status and busy flag.
#[derive(Clone, Copy)]
pub struct GitSyncControls {
    pub status: RwSignal<Option<SyncStatus>>,
    pub busy: RwSignal<Option<SyncOp>>,
}

impl Default for GitSyncControls {
    fn default() -> Self {
        Self::new()
    }
}

impl GitSyncControls {
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(None),
            busy: RwSignal::new(None),
        }
    }

    /// Re-read branch/upstream/ahead-behind/dirty state for `cwd`.
    pub fn refresh(&self, cwd: String) {
        let status = self.status;
        spawn_local(async move {
            if let Ok(s) = git_sync_status(cwd).await {
                status.set(Some(s));
            }
        });
    }

    /// Drop the cached status (e.g. when the active repo is gone).
    pub fn clear(&self) {
        self.status.set(None);
    }

    /// Whether the upstream is missing, so a Push must `--set-upstream`.
    pub fn needs_upstream(&self) -> bool {
        self.status
            .get_untracked()
            .map(|s| s.upstream.is_none())
            .unwrap_or(false)
    }
}

/// Maps a (op, `SyncOutcome.kind`) pair to `(is_success, message key)` so the
/// caller can toast a localized result without parsing git text.
fn sync_message(op: SyncOp, kind: &str) -> (bool, I18nKey) {
    match kind {
        "ok" => (
            true,
            match op {
                SyncOp::Fetch => I18nKey::SbDiffFetched,
                SyncOp::Pull => I18nKey::SbDiffPulled,
                SyncOp::Push => I18nKey::SbDiffPushed,
            },
        ),
        "updated" => (
            true,
            if matches!(op, SyncOp::Push) {
                I18nKey::SbDiffPushed
            } else {
                I18nKey::SbDiffPulled
            },
        ),
        "up_to_date" => (true, I18nKey::SbDiffSyncUpToDate),
        "conflict" => (false, I18nKey::SbDiffPullConflict),
        "dirty" => (false, I18nKey::SbDiffDirtyBlocked),
        "non_fast_forward" => (false, I18nKey::SbDiffNonFastForward),
        "no_upstream" => (false, I18nKey::SbDiffNoUpstream),
        "auth" => (false, I18nKey::SbDiffAuthFailed),
        "no_remote" => (false, I18nKey::SbDiffNoRemote),
        "lock" => (false, I18nKey::SbDiffSyncLocked),
        "network" => (false, I18nKey::SbDiffNetworkError),
        _ => (false, I18nKey::SbDiffSyncError),
    }
}

/// Runs `op` against `cwd`, toasts the localized outcome, then calls `after`
/// (typically bumping the repo epoch so the diff list, graph and status all
/// refresh). No-ops while another sync is in flight.
pub fn run_sync_op(
    controls: GitSyncControls,
    op: SyncOp,
    cwd: String,
    set_upstream: bool,
    toast: ToastService,
    i18n: I18nService,
    after: impl Fn() + 'static,
) {
    if controls.busy.get_untracked().is_some() {
        return;
    }
    controls.busy.set(Some(op));
    spawn_local(async move {
        let res = match op {
            SyncOp::Fetch => git_fetch(cwd).await,
            SyncOp::Pull => git_pull(cwd).await,
            SyncOp::Push => git_push(cwd, set_upstream).await,
        };
        controls.busy.set(None);
        match res {
            Ok(outcome) => {
                let (ok, key) = sync_message(op, &outcome.kind);
                if ok {
                    toast.success(i18n.tr(key)());
                } else {
                    toast.error(i18n.tr(key)());
                }
            }
            Err(e) if e == GIT_MISSING_CODE => {
                toast.error(i18n.tr(I18nKey::SbDiffGitMissing)());
            }
            Err(_) => {
                toast.error(i18n.tr(I18nKey::SbDiffSyncError)());
            }
        }
        after();
    });
}
