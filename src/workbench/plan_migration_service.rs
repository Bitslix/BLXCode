use crate::tauri_bridge::{
    is_tauri_shell, plan_migration_ensure_started, plan_migration_poll, PlanMigrationProgress,
};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, Copy)]
pub struct PlanMigrationService {
    workspace: RwSignal<Option<String>>,
    progress: RwSignal<PlanMigrationProgress>,
    request: RwSignal<u64>,
}

impl PlanMigrationService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            workspace: RwSignal::new(None),
            progress: RwSignal::new(PlanMigrationProgress::default()),
            request: RwSignal::new(0),
        }
    }

    pub fn progress(&self) -> RwSignal<PlanMigrationProgress> {
        self.progress
    }

    pub fn ensure_for_workspace(&self, workspace_cwd: String, wb: WorkbenchService) {
        if !is_tauri_shell() || workspace_cwd.trim().is_empty() {
            return;
        }
        let current_workspace = self.workspace.get_untracked();
        let current = self.progress.get_untracked();
        if current_workspace.as_deref() == Some(workspace_cwd.as_str())
            && (current.busy || matches!(current.phase.as_str(), "done" | "error"))
        {
            return;
        }
        self.workspace.set(Some(workspace_cwd.clone()));
        self.request
            .update(|request| *request = request.saturating_add(1));
        let request = self.request.get_untracked();
        let service = *self;
        spawn_local(async move {
            match plan_migration_ensure_started(&workspace_cwd).await {
                Ok(progress) => {
                    service.apply_progress(progress.clone(), &wb);
                    if progress.busy {
                        service.poll(workspace_cwd, request, wb);
                    }
                }
                Err(err) => service.set_error(err),
            }
        });
    }

    fn poll(&self, workspace_cwd: String, request: u64, wb: WorkbenchService) {
        let service = *self;
        spawn_local(async move {
            loop {
                TimeoutFuture::new(180).await;
                if service.request.get_untracked() != request {
                    break;
                }
                match plan_migration_poll(&workspace_cwd).await {
                    Ok(progress) => {
                        let busy = progress.busy;
                        service.apply_progress(progress, &wb);
                        if !busy {
                            break;
                        }
                    }
                    Err(err) => {
                        service.set_error(err);
                        break;
                    }
                }
            }
        });
    }

    fn apply_progress(&self, progress: PlanMigrationProgress, wb: &WorkbenchService) {
        let was_busy = self.progress.get_untracked().busy;
        let finished = was_busy && !progress.busy && progress.phase == "done";
        let migrated = progress.migrated;
        self.progress.set(progress);
        if finished && migrated > 0 {
            wb.bump_plans_epoch();
        }
    }

    fn set_error(&self, error: String) {
        self.progress.update(|progress| {
            progress.phase = "error".into();
            progress.busy = false;
            progress.error = Some(error);
        });
    }
}
