//! Service holding the cached `.agents/{rules,skills}/` listings for the
//! active workspace plus async actions that round-trip through the Tauri
//! command surface.
//!
//! The service is provided as a Leptos context in `WorkbenchShell` and
//! consumed by the two tab dock components plus the install dialog.

use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::skills_rules_wire::{RuleEntry, SkillEntry, SkillSourceInput};
use crate::tauri_bridge::{self, PointerResult};
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
    pointer_status: RwSignal<Option<Vec<PointerResult>>>,
    pointers_open: RwSignal<bool>,
    pointers_notice_dismissed: RwSignal<bool>,
    pointers_busy: RwSignal<bool>,
    selected_pointer_agents: RwSignal<HashSet<String>>,
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
            pointer_status: RwSignal::new(None),
            pointers_open: RwSignal::new(false),
            pointers_notice_dismissed: RwSignal::new(false),
            pointers_busy: RwSignal::new(false),
            selected_pointer_agents: RwSignal::new(HashSet::new()),
        }
    }

    #[must_use]
    pub fn pointer_status(&self) -> RwSignal<Option<Vec<PointerResult>>> {
        self.pointer_status
    }
    #[must_use]
    pub fn pointers_open(&self) -> RwSignal<bool> {
        self.pointers_open
    }
    #[must_use]
    pub fn pointers_notice_dismissed(&self) -> RwSignal<bool> {
        self.pointers_notice_dismissed
    }
    #[must_use]
    pub fn pointers_busy(&self) -> RwSignal<bool> {
        self.pointers_busy
    }
    #[must_use]
    pub fn selected_pointer_agents(&self) -> RwSignal<HashSet<String>> {
        self.selected_pointer_agents
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

    /// Refreshes the per-agent rules-pointer installation status.
    /// Called on workspace switch and after install/uninstall actions so
    /// the banner and dialog reflect the on-disk state.
    pub fn refresh_pointer_status(self, wb: WorkbenchService) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            self.pointer_status.set(None);
            return;
        };
        let status = self.pointer_status;
        let err = self.rules_error;
        spawn_local(async move {
            match tauri_bridge::rules_pointer_status(&cwd).await {
                Ok(results) => status.set(Some(results)),
                Err(e) => {
                    status.set(Some(Vec::new()));
                    err.set(Some(e));
                }
            }
        });
    }

    /// Clears pointer UI state — called on workspace switch so a stale
    /// "dismissed" flag from the previous workspace doesn't hide the
    /// banner in the new one.
    pub fn reset_pointer_ui(self) {
        self.pointer_status.set(None);
        self.pointers_open.set(false);
        self.pointers_notice_dismissed.set(false);
        self.pointers_busy.set(false);
        self.selected_pointer_agents.set(HashSet::new());
    }

    /// Runs the install or uninstall action for the currently selected
    /// agent ids, then merges the result into `pointer_status`.
    pub fn run_pointer_action(self, wb: WorkbenchService, install: bool, agents: Vec<String>) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        if agents.is_empty() || self.pointers_busy.get_untracked() {
            return;
        }
        self.pointers_busy.set(true);
        let busy = self.pointers_busy;
        let status = self.pointer_status;
        let err = self.rules_error;
        spawn_local(async move {
            let result = if install {
                tauri_bridge::rules_install_pointers(&cwd, agents).await
            } else {
                tauri_bridge::rules_uninstall_pointers(&cwd, agents).await
            };
            match result {
                Ok(results) => {
                    status.update(|cur| {
                        let mut merged = cur.take().unwrap_or_default();
                        for r in results {
                            if let Some(existing) = merged.iter_mut().find(|e| e.agent == r.agent) {
                                *existing = r;
                            } else {
                                merged.push(r);
                            }
                        }
                        *cur = Some(merged);
                    });
                    err.set(None);
                }
                Err(e) => err.set(Some(e)),
            }
            busy.set(false);
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

    /// Reads a rule markdown body and writes it into the given signal.
    /// Errors are surfaced through the shared rules_error signal so the panel
    /// renders them in its standard error slot.
    pub fn read_rule_into(
        self,
        wb: WorkbenchService,
        name: String,
        body: RwSignal<Option<String>>,
        loading: RwSignal<bool>,
    ) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        loading.set(true);
        let err = self.rules_error;
        spawn_local(async move {
            match tauri_bridge::rules_read(cwd, name).await {
                Ok(content) => body.set(Some(content)),
                Err(e) => err.set(Some(e)),
            }
            loading.set(false);
        });
    }

    pub fn write_rule(
        self,
        wb: WorkbenchService,
        name: String,
        content: String,
        body: RwSignal<Option<String>>,
        editing: RwSignal<bool>,
        saving: RwSignal<bool>,
    ) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        saving.set(true);
        let err = self.rules_error;
        let svc = self;
        spawn_local(async move {
            match tauri_bridge::rules_write(cwd, name, content.clone()).await {
                Ok(_) => {
                    body.set(Some(content));
                    editing.set(false);
                    svc.refresh_rules(wb);
                }
                Err(e) => err.set(Some(e)),
            }
            saving.set(false);
        });
    }

    pub fn create_rule(
        self,
        wb: WorkbenchService,
        name: String,
        content: String,
        saving: RwSignal<bool>,
        on_done: impl Fn(Result<(), String>) + 'static,
    ) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            on_done(Err("no workspace selected".into()));
            return;
        };
        saving.set(true);
        let err = self.rules_error;
        let svc = self;
        spawn_local(async move {
            // `rules_write` writes the file and records/updates the rule in
            // `.agents/rules/index.json`, so creation stays index-aware.
            let result = tauri_bridge::rules_write(cwd, name, content).await;
            saving.set(false);
            match result {
                Ok(_) => {
                    err.set(None);
                    svc.refresh_rules(wb);
                    on_done(Ok(()));
                }
                Err(e) => {
                    err.set(Some(e.clone()));
                    on_done(Err(e));
                }
            }
        });
    }

    /// Reads a skill's `SKILL.md` body and writes it into the given signal.
    /// Errors are surfaced through the shared skills_error signal so the panel
    /// renders them in its standard error slot.
    pub fn read_skill_into(
        self,
        wb: WorkbenchService,
        name: String,
        body: RwSignal<Option<String>>,
        loading: RwSignal<bool>,
    ) {
        let Some(cwd) = Self::workspace_cwd(&wb) else {
            return;
        };
        loading.set(true);
        let err = self.skills_error;
        spawn_local(async move {
            match tauri_bridge::skills_read(cwd, name).await {
                Ok(content) => body.set(Some(content)),
                Err(e) => err.set(Some(e)),
            }
            loading.set(false);
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
