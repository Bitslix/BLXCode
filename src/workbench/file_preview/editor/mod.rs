//! Lightweight in-app code editor layered over the existing file-preview
//! highlight.js pipeline.
//!
//! [`EditorSession`] is the shared state created once per open document by
//! `FilePreviewDock` and threaded into the header (edit controls + chips) and
//! the body renderer (`CodeView` / `MarkdownView`). The body owns highlighting
//! and the textarea overlay; this module owns load / save / revert / conflict
//! and the derived dirty flag. The pure sub-modules are unit-tested in
//! isolation.

pub mod buffer;
pub mod code_mirror;
pub mod folding;
pub mod policy;

use buffer::Baseline;
use folding::FoldState;
use policy::Editability;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_conflict_error, read_workspace_text_file, write_workspace_text_file};
use crate::workbench::state::ConfirmRequest;
use crate::workbench::toast::ToastService;
use crate::workbench::{HarnessUiService, WorkbenchService};
use leptos::prelude::*;
use leptos::task::spawn_local;

/// Whether the open document is currently rendered for reading or editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMode {
    View,
    Edit,
}

/// Coarse lifecycle status surfaced as header chips / banners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocStatus {
    Loading,
    Ready,
    Error(String),
    /// File exceeded the preview cap; shown read-only.
    TooLarge,
    Saving,
    Saved,
    /// Last save was refused because the file changed on disk.
    Conflict,
}

/// Shared, `Copy` editor state for one open document. All fields are reactive
/// signals so the same handle can be cloned into the header and the body.
#[derive(Clone, Copy)]
pub struct EditorSession {
    pub workspace_id: u64,
    rel_path: StoredValue<String>,
    pub mode: RwSignal<EditMode>,
    pub editability: RwSignal<Editability>,
    pub buffer: RwSignal<String>,
    pub baseline: RwSignal<Option<Baseline>>,
    pub byte_len: RwSignal<u64>,
    pub status: RwSignal<DocStatus>,
    pub folds: RwSignal<FoldState>,
    pub dirty: Memo<bool>,
}

impl EditorSession {
    /// Create a fresh session for `rel_path`. Must run inside a reactive owner
    /// (i.e. from a component body).
    #[must_use]
    pub fn new(workspace_id: u64, rel_path: String) -> Self {
        let buffer = RwSignal::new(String::new());
        let baseline = RwSignal::new(None::<Baseline>);
        let dirty = Memo::new(move |_| {
            baseline.with(|b| match b {
                Some(b) => buffer.with(|buf| b.is_dirty(buf)),
                None => false,
            })
        });
        Self {
            workspace_id,
            rel_path: StoredValue::new(rel_path),
            mode: RwSignal::new(EditMode::View),
            editability: RwSignal::new(Editability::NeverEdit),
            buffer,
            baseline,
            byte_len: RwSignal::new(0),
            status: RwSignal::new(DocStatus::Loading),
            folds: RwSignal::new(FoldState::new(Vec::new())),
            dirty,
        }
    }

    #[must_use]
    pub fn rel_path(&self) -> String {
        self.rel_path.get_value()
    }

    /// `true` when Save should be enabled right now.
    #[must_use]
    pub fn can_save(&self) -> bool {
        matches!(self.mode.get(), EditMode::Edit)
            && self.dirty.get()
            && !matches!(self.editability.get(), Editability::NeverEdit)
            && !matches!(self.status.get(), DocStatus::Saving)
    }

    /// Switch into edit mode when the document allows it. Editing always starts
    /// with folds expanded (overlay editors can't hide lines — see the plan).
    pub fn enter_edit(&self) {
        if self.editability.get_untracked().is_editable_eventually() {
            self.folds.update(|f| f.collapsed.clear());
            self.mode.set(EditMode::Edit);
        }
    }

    /// Return to view mode (caller is responsible for any unsaved-changes
    /// confirmation before calling this).
    pub fn exit_edit(&self) {
        self.mode.set(EditMode::View);
    }

    /// Restore the buffer to the on-disk baseline, discarding edits.
    pub fn revert(&self) {
        let disk = self
            .baseline
            .with_untracked(|b| b.as_ref().map(|b| b.disk_text.clone()));
        if let Some(text) = disk {
            self.buffer.set(text);
        }
    }

    /// Resolve `(workspace_root, connection_id)` for this session's workspace.
    fn root_conn(&self, wb: &WorkbenchService) -> Option<(String, Option<String>)> {
        wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == self.workspace_id)
                .map(|w| (w.cwd.clone(), w.remote_connection_id.clone()))
        })
    }

    /// (Re)load the file from disk into `buffer` + `baseline`, discarding any
    /// in-memory edits. Called on mount, on Refresh, and on conflict-reload.
    pub fn reload(self, wb: WorkbenchService) {
        self.status.set(DocStatus::Loading);
        let Some((root, conn)) = self.root_conn(&wb) else {
            self.status
                .set(DocStatus::Error("workspace not found".into()));
            return;
        };
        let rel = self.rel_path.get_value();
        let me = self;
        spawn_local(async move {
            match read_workspace_text_file(root, rel, conn).await {
                Ok(t) => {
                    me.byte_len.set(t.byte_len);
                    me.buffer.set(t.content.clone());
                    me.baseline
                        .set(Some(Baseline::new(t.content, t.hash, t.modified_ms)));
                    me.status.set(if t.truncated {
                        DocStatus::TooLarge
                    } else {
                        DocStatus::Ready
                    });
                }
                Err(e) => me.status.set(DocStatus::Error(e)),
            }
        });
    }

    /// Persist the buffer. On an on-disk conflict, opens a confirmation dialog
    /// offering to overwrite; on other errors, surfaces a toast. `force`
    /// bypasses the conflict guard (used by the overwrite branch).
    pub fn save(
        self,
        wb: WorkbenchService,
        toast: ToastService,
        ui: HarnessUiService,
        i18n: I18nService,
        force: bool,
    ) {
        if matches!(self.editability.get_untracked(), Editability::NeverEdit) {
            return;
        }
        if matches!(self.status.get_untracked(), DocStatus::Saving) {
            return;
        }
        let Some((root, conn)) = self.root_conn(&wb) else {
            return;
        };
        let rel = self.rel_path.get_value();
        let content = self.buffer.get_untracked();
        let expected = if force {
            None
        } else {
            self.baseline
                .with_untracked(|b| b.as_ref().map(|b| b.hash.clone()))
        };
        self.status.set(DocStatus::Saving);
        let me = self;
        spawn_local(async move {
            match write_workspace_text_file(root, rel, content.clone(), expected, conn).await {
                Ok(res) => {
                    me.byte_len.set(res.byte_len);
                    me.baseline.update(|b| match b {
                        Some(b) => b.after_save(content.clone(), res.hash.clone(), res.modified_ms),
                        None => {
                            *b = Some(Baseline::new(
                                content.clone(),
                                res.hash.clone(),
                                res.modified_ms,
                            ))
                        }
                    });
                    me.status.set(DocStatus::Saved);
                    toast.success(i18n.tr(I18nKey::FilePreviewEditorSaved)().to_string());
                }
                Err(e) if is_conflict_error(&e) => {
                    me.status.set(DocStatus::Conflict);
                    let on_confirm = Callback::new(move |()| {
                        me.save(wb, toast, ui, i18n, true);
                    });
                    ui.request_confirm(ConfirmRequest {
                        title: i18n.tr(I18nKey::FilePreviewEditorConflictTitle)().to_string(),
                        body: i18n.tr(I18nKey::FilePreviewEditorConflictBody)().to_string(),
                        confirm_label: i18n.tr(I18nKey::FilePreviewEditorConflictOverwrite)()
                            .to_string(),
                        cancel_label: i18n.tr(I18nKey::FilePreviewEditorCancel)().to_string(),
                        danger: true,
                        on_confirm,
                    });
                }
                Err(e) => {
                    let msg =
                        i18n.tr(I18nKey::FilePreviewEditorSaveError)().replace("{detail}", &e);
                    me.status.set(DocStatus::Error(e));
                    toast.error(msg);
                }
            }
        });
    }

    /// Prompt before leaving a dirty document (close tab / exit edit). Runs
    /// `on_discard` if the user accepts losing the edits.
    pub fn confirm_discard(
        &self,
        ui: HarnessUiService,
        i18n: I18nService,
        on_discard: Callback<()>,
    ) {
        ui.request_confirm(ConfirmRequest {
            title: i18n.tr(I18nKey::FilePreviewEditorUnsavedTitle)().to_string(),
            body: i18n.tr(I18nKey::FilePreviewEditorUnsavedBody)().to_string(),
            confirm_label: i18n.tr(I18nKey::FilePreviewEditorDiscard)().to_string(),
            cancel_label: i18n.tr(I18nKey::FilePreviewEditorKeep)().to_string(),
            danger: true,
            on_confirm: on_discard,
        });
    }
}
