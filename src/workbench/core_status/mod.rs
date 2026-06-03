//! Status-bar indicator for the agent's loaded core configuration: the
//! count of enabled `.agents/rules/` and `.agents/skills/` entries for the
//! active workspace.
//!
//! Lives in the centre slot of [`crate::app::App`]'s `.app-statusline`. The
//! [`CoreStatusService`] is provided at the App root (a sibling of the
//! workbench shell), so it re-fetches the counts whenever the active
//! workspace changes. Toggling a rule/skill in the Skills & Rules panel also
//! nudges it via [`CoreStatusService::refresh`] so the badge stays in sync.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, rules_list, skills_list};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[derive(Clone, Copy)]
pub struct CoreStatusService {
    rules: RwSignal<usize>,
    skills: RwSignal<usize>,
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
    pub fn loaded(&self) -> RwSignal<bool> {
        self.loaded
    }

    /// Re-counts enabled rules and skills for the active workspace. No-op
    /// outside the Tauri shell or when no workspace is selected (the badge
    /// hides via `loaded` staying `false`).
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
        let loaded = self.loaded;
        spawn_local(async move {
            let rules_cwd = cwd.clone();
            let mut any = false;
            if let Ok(list) = rules_list(rules_cwd).await {
                rules.set(list.iter().filter(|r| r.enabled).count());
                any = true;
            }
            if let Ok(list) = skills_list(cwd).await {
                skills.set(list.iter().filter(|s| s.enabled).count());
                any = true;
            }
            if any {
                loaded.set(true);
            }
        });
    }
}

/// VSCode-style status-bar entry rendered in the centre slot: a shield icon
/// with the enabled-rules count and a sparkles icon with the enabled-skills
/// count. Hidden until the first successful count resolves.
#[component]
pub fn CoreStatusBarItem() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let status = expect_context::<CoreStatusService>();
    let loaded = status.loaded();
    let rules = status.rules();
    let skills = status.skills();

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
            </div>
        </Show>
    }
}
