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
use crate::tauri_bridge::{
    git_branch, is_tauri_shell, memory_list, plan_list, rules_list, skills_list,
};
use crate::workbench::state::CenterTabKind;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorCursorPosition {
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy)]
pub struct CoreStatusService {
    rules: RwSignal<usize>,
    skills: RwSignal<usize>,
    plans: RwSignal<usize>,
    memory_categories: RwSignal<usize>,
    memory_files: RwSignal<usize>,
    memory_size: RwSignal<u64>,
    branch: RwSignal<Option<String>>,
    editor_cursors: RwSignal<HashMap<(u64, String), EditorCursorPosition>>,
    loaded: RwSignal<bool>,
    refresh_generation: RwSignal<u64>,
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
            branch: RwSignal::new(None),
            editor_cursors: RwSignal::new(HashMap::new()),
            loaded: RwSignal::new(false),
            refresh_generation: RwSignal::new(0),
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
    pub fn branch(&self) -> RwSignal<Option<String>> {
        self.branch
    }

    pub fn set_editor_cursor(&self, workspace_id: u64, rel_path: &str, line: u32, column: u32) {
        let rel_path = rel_path.trim().trim_start_matches(['/', '\\']);
        if rel_path.is_empty() {
            return;
        }
        let position = EditorCursorPosition {
            line: line.max(1),
            column: column.max(1),
        };
        self.editor_cursors.update(|cursors| {
            cursors.insert((workspace_id, rel_path.to_string()), position);
        });
    }

    pub fn clear_editor_cursor(&self, workspace_id: u64, rel_path: &str) {
        let rel_path = rel_path.trim().trim_start_matches(['/', '\\']);
        if rel_path.is_empty() {
            return;
        }
        self.editor_cursors.update(|cursors| {
            cursors.remove(&(workspace_id, rel_path.to_string()));
        });
    }

    #[must_use]
    pub fn active_editor_cursor(&self, wb: WorkbenchService) -> Option<EditorCursorPosition> {
        let active = wb.active_id().get()?;
        let rel_path = wb.workspaces().with(|workspaces| {
            let workspace = workspaces.iter().find(|w| w.id == active)?;
            let active_tab = workspace
                .center_tabs
                .iter()
                .find(|tab| tab.id == workspace.center_active_tab_id)?;
            match &active_tab.kind {
                CenterTabKind::FilePreview { rel_path } => Some(rel_path.clone()),
                _ => None,
            }
        })?;
        self.editor_cursors
            .with(|cursors| cursors.get(&(active, rel_path)).copied())
    }

    #[must_use]
    pub fn loaded(&self) -> RwSignal<bool> {
        self.loaded
    }

    /// Re-counts statusline project stats for the active workspace. No-op
    /// outside the Tauri shell or when no workspace is selected.
    pub fn refresh(self, wb: WorkbenchService) {
        let refresh_generation = self.refresh_generation;
        let generation = refresh_generation.get_untracked().wrapping_add(1);
        refresh_generation.set(generation);
        if !is_tauri_shell() {
            return;
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            self.branch.set(None);
            self.loaded.set(false);
            return;
        };
        let connection_id = wb.active_remote_connection_id();
        let rules = self.rules;
        let skills = self.skills;
        let plans = self.plans;
        let memory_categories = self.memory_categories;
        let memory_files = self.memory_files;
        let memory_size = self.memory_size;
        let branch = self.branch;
        let loaded = self.loaded;
        branch.set(None);
        spawn_local(async move {
            let rules_cwd = cwd.clone();
            let skills_cwd = cwd.clone();
            let plans_cwd = cwd.clone();
            let memory_cwd = cwd.clone();
            let mut any = false;
            if let Ok(name) = git_branch(cwd, connection_id).await {
                if refresh_generation.get_untracked() != generation {
                    return;
                }
                if name.is_some() {
                    any = true;
                }
                branch.set(name);
            }
            if let Ok(list) = rules_list(rules_cwd).await {
                if refresh_generation.get_untracked() != generation {
                    return;
                }
                rules.set(list.iter().filter(|r| r.enabled).count());
                any = true;
            }
            if let Ok(list) = skills_list(skills_cwd).await {
                if refresh_generation.get_untracked() != generation {
                    return;
                }
                skills.set(list.iter().filter(|s| s.enabled).count());
                any = true;
            }
            if let Ok(list) = plan_list(&plans_cwd).await {
                if refresh_generation.get_untracked() != generation {
                    return;
                }
                plans.set(list.iter().filter(|p| !p.is_index).count());
                any = true;
            }
            if let Ok(resp) = memory_list(&memory_cwd).await {
                if refresh_generation.get_untracked() != generation {
                    return;
                }
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
    let branch = status.branch();
    let cursor = Memo::new(move |_| status.active_editor_cursor(wb));

    // Re-count whenever the active workspace changes.
    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.workspaces().get();
        status.refresh(wb);
    });

    view! {
        <Show when=move || loaded.get() || cursor.get().is_some()>
            <div class="app-statusline__item core-status-item app-statusline__item--quiet">
                <Show when=move || cursor.get().is_some()>
                    <span
                        class="core-status-item__seg core-status-item__seg--cursor"
                        title=move || cursor.get().map(format_cursor_title).unwrap_or_default()
                    >
                        <LxIcon icon=icondata::LuTextCursor width="0.78rem" height="0.78rem" />
                        <span>{move || cursor.get().map(format_cursor_label).unwrap_or_default()}</span>
                    </span>
                    <Show when=move || loaded.get()>
                        <span class="core-status-item__divider" aria-hidden="true"></span>
                    </Show>
                </Show>
                <Show when=move || loaded.get()>
                    <Show when=move || branch.with(|b| b.is_some())>
                        <span class="core-status-item__seg core-status-item__seg--branch" title=move || i18n.tr(I18nKey::CoreStatusGitBranchTip)()>
                            <LxIcon icon=icondata::LuGitBranch width="0.78rem" height="0.78rem" />
                            <span>{move || branch.get().unwrap_or_default()}</span>
                        </span>
                        <span class="core-status-item__divider" aria-hidden="true"></span>
                    </Show>
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
                </Show>
            </div>
        </Show>
    }
}

fn format_cursor_label(position: EditorCursorPosition) -> String {
    format!("Ln {}, Col {}", position.line, position.column)
}

fn format_cursor_title(position: EditorCursorPosition) -> String {
    format!("Line {}, Column {}", position.line, position.column)
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
