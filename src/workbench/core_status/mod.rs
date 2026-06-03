//! Status-bar indicator for the active workspace's agent-adjacent project
//! material: enabled rules/skills, plans, and workspace memory totals.
//!
//! Lives in the centre slot of [`crate::app::App`]'s `.app-statusline`. The
//! [`CoreStatusService`] is provided at the App root (a sibling of the
//! workbench shell), so it re-fetches the counts whenever the active
//! workspace changes. Toggling a rule/skill in the Skills & Rules panel also
//! nudges it via [`CoreStatusService::refresh`] so the badge stays in sync.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, memory_list, plan_list, rules_list, skills_list};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::HashSet;

#[derive(Clone, Copy)]
pub struct CoreStatusService {
    rules: RwSignal<usize>,
    skills: RwSignal<usize>,
    plans: RwSignal<usize>,
    memory_categories: RwSignal<usize>,
    memory_files: RwSignal<usize>,
    memory_size: RwSignal<u64>,
    loaded: RwSignal<bool>,
}

impl Default for CoreStatusService {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreStatusService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: RwSignal::new(0),
            skills: RwSignal::new(0),
            plans: RwSignal::new(0),
            memory_categories: RwSignal::new(0),
            memory_files: RwSignal::new(0),
            memory_size: RwSignal::new(0),
            loaded: RwSignal::new(false),
        }
    }

    #[must_use]
    pub fn rules(&self) -> RwSignal<usize> {
        self.rules
    }

    #[must_use]
    pub fn skills(&self) -> RwSignal<usize> {
        self.skills
    }

    #[must_use]
    pub fn plans(&self) -> RwSignal<usize> {
        self.plans
    }

    #[must_use]
    pub fn memory_categories(&self) -> RwSignal<usize> {
        self.memory_categories
    }

    #[must_use]
    pub fn memory_files(&self) -> RwSignal<usize> {
        self.memory_files
    }

    #[must_use]
    pub fn memory_size(&self) -> RwSignal<u64> {
        self.memory_size
    }

    #[must_use]
    pub fn loaded(&self) -> RwSignal<bool> {
        self.loaded
    }

    /// Re-counts statusline project stats for the active workspace. No-op
    /// outside the Tauri shell or when no workspace is selected.
    pub fn refresh(self, wb: WorkbenchService) {
        if !is_tauri_shell() {
            return;
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            self.loaded.set(false);
            return;
        };
        let rules = self.rules;
        let skills = self.skills;
        let plans = self.plans;
        let memory_categories = self.memory_categories;
        let memory_files = self.memory_files;
        let memory_size = self.memory_size;
        let loaded = self.loaded;
        spawn_local(async move {
            let rules_cwd = cwd.clone();
            let skills_cwd = cwd.clone();
            let plans_cwd = cwd.clone();
            let memory_cwd = cwd;
            let mut any = false;
            if let Ok(list) = rules_list(rules_cwd).await {
                rules.set(list.iter().filter(|r| r.enabled).count());
                any = true;
            }
            if let Ok(list) = skills_list(skills_cwd).await {
                skills.set(list.iter().filter(|s| s.enabled).count());
                any = true;
            }
            if let Ok(list) = plan_list(&plans_cwd).await {
                plans.set(list.iter().filter(|p| !p.is_index).count());
                any = true;
            }
            if let Ok(resp) = memory_list(&memory_cwd).await {
                let workspace_notes = resp
                    .notes
                    .iter()
                    .filter(|note| {
                        note.scope == crate::tauri_bridge::MemoryScope::Workspace
                            && note.enabled
                            && !note.is_template
                    })
                    .collect::<Vec<_>>();
                let mut categories = workspace_notes
                    .iter()
                    .filter_map(|note| {
                        let category = note.category.trim();
                        (!category.is_empty() && category != "memory")
                            .then_some(category.to_owned())
                    })
                    .collect::<HashSet<_>>();
                categories.extend(resp.memory_subcategories.workspace.into_iter().filter(
                    |category| {
                        let category = category.trim();
                        !category.is_empty() && category != "memory"
                    },
                ));
                memory_categories.set(categories.len());
                memory_files.set(workspace_notes.len());
                memory_size.set(workspace_notes.iter().map(|note| note.size).sum());
                any = true;
            }
            if any {
                loaded.set(true);
            }
        });
    }
}

/// VSCode-style status-bar entry rendered in the centre slot. Hidden until the
/// first successful count resolves.
#[component]
pub fn CoreStatusBarItem() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let status = expect_context::<CoreStatusService>();
    let loaded = status.loaded();
    let rules = status.rules();
    let skills = status.skills();
    let plans = status.plans();
    let memory_categories = status.memory_categories();
    let memory_files = status.memory_files();
    let memory_size = status.memory_size();

    // Re-count whenever the active workspace changes.
    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.workspaces().get();
        status.refresh(wb);
    });

    view! {
        <Show when=move || loaded.get()>
            <div class="app-statusline__item core-status-item app-statusline__item--quiet">
                <span class="core-status-item__seg" title=move || i18n.tr(I18nKey::CoreStatusRulesTip)()>
                    <LxIcon icon=icondata::LuShield width="0.78rem" height="0.78rem" />
                    <span>{move || rules.get().to_string()}</span>
                </span>
                <span class="core-status-item__seg" title=move || i18n.tr(I18nKey::CoreStatusSkillsTip)()>
                    <LxIcon icon=icondata::LuSparkles width="0.78rem" height="0.78rem" />
                    <span>{move || skills.get().to_string()}</span>
                </span>
                <span class="core-status-item__seg" title=move || i18n.tr(I18nKey::CoreStatusPlansTip)()>
                    <LxIcon icon=icondata::LuClipboardList width="0.78rem" height="0.78rem" />
                    <span>{move || plans.get().to_string()}</span>
                </span>
                <span class="core-status-item__divider" aria-hidden="true"></span>
                <span class="core-status-item__memory" title=move || i18n.tr(I18nKey::CoreStatusMemoryTip)()>
                    <span class="core-status-item__seg" title=move || i18n.tr(I18nKey::CoreStatusMemoryCategoriesTip)()>
                        <LxIcon icon=icondata::LuFolderTree width="0.78rem" height="0.78rem" />
                        <span>{move || memory_categories.get().to_string()}</span>
                    </span>
                    <span class="core-status-item__seg" title=move || i18n.tr(I18nKey::CoreStatusMemoryFilesTip)()>
                        <LxIcon icon=icondata::LuFileText width="0.78rem" height="0.78rem" />
                        <span>{move || memory_files.get().to_string()}</span>
                    </span>
                    <span class="core-status-item__seg" title=move || i18n.tr(I18nKey::CoreStatusMemorySizeTip)()>
                        <LxIcon icon=icondata::LuHardDrive width="0.78rem" height="0.78rem" />
                        <span>{move || format_memory_size(memory_size.get())}</span>
                    </span>
                </span>
            </div>
        </Show>
    }
}

fn format_memory_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GIB {
        format!("{:.1} GiB", bytes_f / GIB)
    } else if bytes_f >= MIB {
        format!("{:.1} MiB", bytes_f / MIB)
    } else if bytes_f >= KIB {
        format!("{:.1} KiB", bytes_f / KIB)
    } else {
        format!("{bytes} B")
    }
}
