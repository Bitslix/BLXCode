//! Service holding the cached `.agents/{rules,skills}/` listings for the
//! active workspace plus async actions that round-trip through the Tauri
//! command surface.
//!
//! The service is provided as a Leptos context in `WorkbenchShell` and
//! consumed by the two tab dock components plus the install dialog.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::skills_rules_wire::{RuleEntry, SkillEntry, SkillSourceInput};
use crate::tauri_bridge;
use crate::workbench::WorkbenchService;

#[derive(Clone, Copy)]
pub struct SkillsRulesService {
    rules: RwSignal<Vec<RuleEntry>>,
    skills: RwSignal<Vec<SkillEntry>>,
    rules_loading: RwSignal<bool>,
    skills_loading: RwSignal<bool>,
    rules_error: RwSignal<Option<String>>,
    skills_error: RwSignal<Option<String>>,
    install_busy: RwSignal<bool>,
    install_error: RwSignal<Option<String>>,
}

impl SkillsRulesService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: RwSignal::new(Vec::new()),
            skills: RwSignal::new(Vec::new()),
            rules_loading: RwSignal::new(false),
            skills_loading: RwSignal::new(false),
            rules_error: RwSignal::new(None),
            skills_error: RwSignal::new(None),
            install_busy: RwSignal::new(false),
            install_error: RwSignal::new(None),
        }
    }

    #[must_use]
    pub fn rules(&self) -> RwSignal<Vec<RuleEntry>> {
        self.rules
    }
    #[must_use]
    pub fn skills(&self) -> RwSignal<Vec<SkillEntry>> {
        self.skills
    }
    #[must_use]
    pub fn rules_loading(&self) -> RwSignal<bool> {
        self.rules_loading
    }
    #[must_use]
    pub fn skills_loading(&self) -> RwSignal<bool> {
        self.skills_loading
    }
    #[must_use]
    pub fn rules_error(&self) -> RwSignal<Option<String>> {
        self.rules_error
    }
    #[must_use]
    pub fn skills_error(&self) -> RwSignal<Option<String>> {
        self.skills_error
    }
    #[must_use]
    pub fn install_busy(&self) -> RwSignal<bool> {
        self.install_busy
    }
    #[must_use]
    pub fn install_error(&self) -> RwSignal<Option<String>> {
        self.install_error
    }

    /// Returns the active workspace cwd if one is selected.
    fn workspace_cwd(wb: &WorkbenchService) -> Option<String> {
        let id = wb.active_id().get()?;
        wb.workspaces()
            .get()
            .into_iter()
            .find(|w| w.id == id)
            .map(|w| w.cwd.clone())
            .filter(|s| !s.trim().is_empty())
    }

    pub fn refresh_rules(self, wb: WorkbenchService) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            self.rules.set(Vec::new());
            self.rules_error.set(None);
            return;
        };
        self.rules_loading.set(true);
        self.rules_error.set(None);
        let rules = self.rules;
        let err = self.rules_error;
        let loading = self.rules_loading;
        spawn_local(async move {
            match tauri_bridge::rules_list(cwd).await {
                Ok(list) => rules.set(list),
                Err(e) => err.set(Some(e)),
            }
            loading.set(false);
        });
    }

    pub fn refresh_skills(self, wb: WorkbenchService) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            self.skills.set(Vec::new());
            self.skills_error.set(None);
            return;
        };
        self.skills_loading.set(true);
        self.skills_error.set(None);
        let skills = self.skills;
        let err = self.skills_error;
        let loading = self.skills_loading;
        spawn_local(async move {
            match tauri_bridge::skills_list(cwd).await {
                Ok(list) => skills.set(list),
                Err(e) => err.set(Some(e)),
            }
            loading.set(false);
        });
    }

    pub fn set_rule_enabled(self, wb: WorkbenchService, name: String, enabled: bool) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        let err = self.rules_error;
        let svc = self;
        spawn_local(async move {
            match tauri_bridge::rules_set_enabled(cwd, name, enabled).await {
                Ok(_) => svc.refresh_rules(wb),
                Err(e) => err.set(Some(e)),
            }
        });
    }

    pub fn set_skill_enabled(self, wb: WorkbenchService, name: String, enabled: bool) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        let err = self.skills_error;
        let svc = self;
        spawn_local(async move {
            match tauri_bridge::skills_set_enabled(cwd, name, enabled).await {
                Ok(_) => svc.refresh_skills(wb),
                Err(e) => err.set(Some(e)),
            }
        });
    }

    pub fn remove_rule(self, wb: WorkbenchService, name: String) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        let err = self.rules_error;
        let svc = self;
        spawn_local(async move {
            match tauri_bridge::rules_remove(cwd, name).await {
                Ok(()) => svc.refresh_rules(wb),
                Err(e) => err.set(Some(e)),
            }
        });
    }

    pub fn remove_skill(self, wb: WorkbenchService, name: String) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        let err = self.skills_error;
        let svc = self;
        spawn_local(async move {
            match tauri_bridge::skills_remove(cwd, name).await {
                Ok(()) => svc.refresh_skills(wb),
                Err(e) => err.set(Some(e)),
            }
        });
    }

    /// Triggers an install and refreshes the skill list on success.
    /// `on_done(Ok(()))` is called when the install committed; the caller
    /// can use it to close the dialog only on success.
    pub fn install_skill(
        self,
        wb: WorkbenchService,
        name: String,
        source: SkillSourceInput,
        on_done: impl Fn(Result<(), String>) + 'static,
    ) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            on_done(Err("no workspace selected".into()));
            return;
        };
        self.install_busy.set(true);
        self.install_error.set(None);
        let busy = self.install_busy;
        let err = self.install_error;
        let svc = self;
        spawn_local(async move {
            let result = tauri_bridge::skills_install(cwd, name, source).await;
            busy.set(false);
            match result {
                Ok(_) => {
                    err.set(None);
                    svc.refresh_skills(wb);
                    on_done(Ok(()));
                }
                Err(e) => {
                    err.set(Some(e.clone()));
                    on_done(Err(e));
                }
            }
        });
    }
}

impl Default for SkillsRulesService {
    fn default() -> Self {
        Self::new()
    }
}
