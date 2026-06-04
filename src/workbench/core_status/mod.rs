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
    git_branch, is_tauri_shell, memory_list, plan_list, rules_list, skills_list, MemoryScope,
    NoteMeta,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveEditorStatus {
    pub rel_path: String,
    pub position: EditorCursorPosition,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryScopeStats {
    pub categories: usize,
    pub files: usize,
    pub size: u64,
}

#[derive(Clone, Copy)]
pub struct CoreStatusService {
    rules: RwSignal<usize>,
    skills: RwSignal<usize>,
    plans: RwSignal<usize>,
    workspace_memory: RwSignal<HashMap<u64, MemoryScopeStats>>,
    global_memory: RwSignal<MemoryScopeStats>,
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
            workspace_memory: RwSignal::new(HashMap::new()),
            global_memory: RwSignal::new(MemoryScopeStats::default()),
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
    pub fn global_memory(&self) -> RwSignal<MemoryScopeStats> {
        self.global_memory
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
    pub fn active_editor_status(&self, wb: WorkbenchService) -> Option<ActiveEditorStatus> {
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
        let position = self
            .editor_cursors
            .with(|cursors| cursors.get(&(active, rel_path.clone())).copied())?;
        Some(ActiveEditorStatus { rel_path, position })
    }

    #[must_use]
    pub fn active_workspace_memory(&self, wb: WorkbenchService) -> Option<MemoryScopeStats> {
        let active = wb.active_id().get()?;
        Some(
            self.workspace_memory
                .with(|stats| stats.get(&active).copied().unwrap_or_default()),
        )
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
        let workspace_id = wb.active_id().get_untracked();
        let Some(cwd) = wb.default_workspace_cwd() else {
            self.branch.set(None);
            self.loaded.set(false);
            return;
        };
        let connection_id = wb.active_remote_connection_id();
        let rules = self.rules;
        let skills = self.skills;
        let plans = self.plans;
        let workspace_memory = self.workspace_memory;
        let global_memory = self.global_memory;
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
                let ws_stats = memory_stats_for_scope(
                    &resp.notes,
                    &resp.memory_subcategories.workspace,
                    MemoryScope::Workspace,
                );
                let global_stats = memory_stats_for_scope(
                    &resp.notes,
                    &resp.memory_subcategories.global,
                    MemoryScope::Global,
                );
                if let Some(workspace_id) = workspace_id {
                    workspace_memory.update(|stats| {
                        stats.insert(workspace_id, ws_stats);
                    });
                }
                global_memory.set(global_stats);
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
    let global_memory = status.global_memory();
    let branch = status.branch();
    let editor_status = Memo::new(move |_| status.active_editor_status(wb));
    let workspace_memory = Memo::new(move |_| status.active_workspace_memory(wb));
    let refresh_key = Memo::new(move |_| active_workspace_refresh_key(wb));

    // Re-count whenever the active workspace changes.
    Effect::new(move |_| {
        let _ = refresh_key.get();
        status.refresh(wb);
    });

    view! {
        <Show when=move || loaded.get() || editor_status.get().is_some()>
            <div class="app-statusline__item core-status-item app-statusline__item--quiet">
                <Show when=move || editor_status.get().is_some()>
                    <span
                        class="core-status-item__seg core-status-item__seg--file"
                        title=move || editor_status.get().map(|s| s.rel_path).unwrap_or_default()
                    >
                        <LxIcon icon=icondata::LuFileText width="0.78rem" height="0.78rem" />
                        <span>{move || editor_status.get().map(|s| s.rel_path).unwrap_or_default()}</span>
                    </span>
                    <span class="core-status-item__divider" aria-hidden="true"></span>
                    <span
                        class="core-status-item__seg core-status-item__seg--cursor"
                        title=move || editor_status
                            .get()
                            .map(|s| format_cursor_title(s.position))
                            .unwrap_or_default()
                    >
                        <LxIcon icon=icondata::LuTextCursor width="0.78rem" height="0.78rem" />
                        <span>{move || editor_status
                            .get()
                            .map(|s| format_cursor_label(s.position))
                            .unwrap_or_default()}</span>
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
                        <span class="core-status-item__memory-scope" title="Workspace memory">
                            <span class="core-status-item__memory-label">
                                <LxIcon icon=icondata::LuFolderRoot width="0.78rem" height="0.78rem" />
                                <span>"Workspace"</span>
                            </span>
                            <span class="core-status-item__seg" title=move || memory_scope_title("Workspace", workspace_memory.get().unwrap_or_default())>
                                <LxIcon icon=icondata::LuFolderTree width="0.78rem" height="0.78rem" />
                                <span>{move || workspace_memory.get().unwrap_or_default().categories.to_string()}</span>
                            </span>
                            <span class="core-status-item__seg" title=move || memory_scope_title("Workspace", workspace_memory.get().unwrap_or_default())>
                                <LxIcon icon=icondata::LuFileText width="0.78rem" height="0.78rem" />
                                <span>{move || workspace_memory.get().unwrap_or_default().files.to_string()}</span>
                            </span>
                            <span class="core-status-item__seg" title=move || memory_scope_title("Workspace", workspace_memory.get().unwrap_or_default())>
                                <LxIcon icon=icondata::LuHardDrive width="0.78rem" height="0.78rem" />
                                <span>{move || format_memory_size(workspace_memory.get().unwrap_or_default().size)}</span>
                            </span>
                        </span>
                        <span class="core-status-item__divider core-status-item__divider--memory" aria-hidden="true"></span>
                        <span class="core-status-item__memory-scope" title="Global memory">
                            <span class="core-status-item__memory-label">
                                <LxIcon icon=icondata::LuGlobe width="0.78rem" height="0.78rem" />
                                <span>"Global"</span>
                            </span>
                            <span class="core-status-item__seg" title=move || memory_scope_title("Global", global_memory.get())>
                                <LxIcon icon=icondata::LuFolderTree width="0.78rem" height="0.78rem" />
                                <span>{move || global_memory.get().categories.to_string()}</span>
                            </span>
                            <span class="core-status-item__seg" title=move || memory_scope_title("Global", global_memory.get())>
                                <LxIcon icon=icondata::LuFileText width="0.78rem" height="0.78rem" />
                                <span>{move || global_memory.get().files.to_string()}</span>
                            </span>
                            <span class="core-status-item__seg" title=move || memory_scope_title("Global", global_memory.get())>
                                <LxIcon icon=icondata::LuHardDrive width="0.78rem" height="0.78rem" />
                                <span>{move || format_memory_size(global_memory.get().size)}</span>
                            </span>
                        </span>
                    </span>
                </Show>
            </div>
        </Show>
    }
}

/// Status-bar (left slot) indicator that surfaces the active Vim mode. Only
/// shown when Vim is enabled **and** the user is actively in a file
/// editor/preview tab (reusing `active_editor_status`). Lucide is the only
/// compiled icon set, so a keyboard glyph + "VIM" label stands in for a vim
/// logo.
#[component]
pub fn VimStatusIndicator() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let status = expect_context::<CoreStatusService>();
    let editor_settings = expect_context::<crate::workbench::EditorSettingsService>();
    let vim_enabled = editor_settings.vim_enabled();
    let in_editor = Memo::new(move |_| status.active_editor_status(wb).is_some());
    let visible = move || vim_enabled.get() && in_editor.get();

    view! {
        <Show when=visible>
            <span
                class="app-statusline__item app-statusline__item--quiet vim-status-item"
                title=move || i18n.tr(I18nKey::CodeEditorVimStatusTip)()
            >
                <LxIcon icon=icondata::LuKeyboard width="0.76rem" height="0.76rem" />
                <span>{move || i18n.tr(I18nKey::CoreStatusVimMode)()}</span>
            </span>
        </Show>
    }
}

fn active_workspace_refresh_key(wb: WorkbenchService) -> Option<(u64, String, Option<String>)> {
    let active = wb.active_id().get()?;
    wb.workspaces().with(|workspaces| {
        let workspace = workspaces.iter().find(|w| w.id == active)?;
        Some((
            workspace.id,
            workspace.cwd.clone(),
            workspace.remote_connection_id.clone(),
        ))
    })
}

fn format_cursor_label(position: EditorCursorPosition) -> String {
    format!("Ln {}, Col {}", position.line, position.column)
}

fn format_cursor_title(position: EditorCursorPosition) -> String {
    format!("Line {}, Column {}", position.line, position.column)
}

fn memory_stats_for_scope(
    notes: &[NoteMeta],
    subcategories: &[String],
    scope: MemoryScope,
) -> MemoryScopeStats {
    let scoped_notes = notes
        .iter()
        .filter(|note| note.scope == scope && note.enabled && !note.is_template)
        .collect::<Vec<_>>();
    let mut categories = scoped_notes
        .iter()
        .filter_map(|note| {
            let category = note.category.trim();
            (!category.is_empty() && category != "memory").then_some(category.to_owned())
        })
        .collect::<HashSet<_>>();
    categories.extend(subcategories.iter().filter_map(|category| {
        let category = category.trim();
        (!category.is_empty() && category != "memory").then_some(category.to_owned())
    }));
    MemoryScopeStats {
        categories: categories.len(),
        files: scoped_notes.len(),
        size: scoped_notes.iter().map(|note| note.size).sum(),
    }
}

fn memory_scope_title(scope: &str, stats: MemoryScopeStats) -> String {
    format!(
        "{scope} memory: {} categories, {} files, {}",
        stats.categories,
        stats.files,
        format_memory_size(stats.size)
    )
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
