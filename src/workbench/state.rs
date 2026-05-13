use crate::config::{
    HARNESS_BROWSER_DEFAULT_URL, HARNESS_BROWSER_URL_KEY, HARNESS_WORKSPACE_ROOT_KEY,
};
use crate::tauri_bridge::{is_tauri_shell, workbench_drop_sessions};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Bumped when the on-disk schema changes incompatibly. Snapshots with an
/// unknown version are ignored on load (we fall back to defaults rather
/// than crashing).
pub const WORKBENCH_SNAPSHOT_VERSION: u32 = 1;

/// Static agent rows in the fleet step (display order).
pub const WORKSPACE_FLEET_AGENT_SLUGS: [&str; 5] =
    ["claude", "codex", "gemini", "opencode", "cursor"];

/// One workspace open in the sidebar; shared across center and right panel via [`WorkbenchService`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub id: u64,
    pub title: String,
    pub cwd: String,
    pub terminal_count: u8,
    pub grid_rows: u8,
    pub grid_cols: u8,
    pub next_terminal_id: u64,
    pub slot_ids: Vec<u64>,
    /// One label/slug per terminal slot (e.g. `"claude"` or empty after skip).
    pub slot_agent_labels: Vec<String>,
    /// Split-pane state per slot, parallel-indexed to `slot_ids`. Missing
    /// entries (older snapshots, freshly-created slots) fall back to a
    /// single un-split pane via [`SlotPaneState::default_for_slot`].
    #[serde(default)]
    pub slot_pane_states: Vec<SlotPaneState>,
}

/// Per-slot terminal split state — survives a restart so the grid of
/// panes inside each slot is restored exactly as the user left it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotPaneState {
    pub axis: TerminalSplitAxis,
    pub pane_ids: Vec<u64>,
    pub next_pane_id: u64,
}

impl SlotPaneState {
    /// Default for a newly-created slot: one pane, vertical split axis,
    /// pane id derived from `slot_id` so it stays stable across restarts.
    #[must_use]
    pub fn default_for_slot(slot_id: u64) -> Self {
        let first = slot_id.saturating_mul(1000).saturating_add(1);
        Self {
            axis: TerminalSplitAxis::Vertical,
            pane_ids: vec![first],
            next_pane_id: first.saturating_add(1),
        }
    }
}

impl WorkspaceEntry {
    #[must_use]
    pub fn empty_surface(id: u64) -> Self {
        Self {
            id,
            title: String::new(),
            cwd: String::new(),
            terminal_count: 1,
            grid_rows: 1,
            grid_cols: 1,
            next_terminal_id: 1,
            slot_ids: Vec::new(),
            slot_agent_labels: Vec::new(),
            slot_pane_states: Vec::new(),
        }
    }

    #[must_use]
    pub fn grid_dims_for_count(n: u8) -> (u8, u8) {
        match n {
            1 => (1, 1),
            2 => (1, 2),
            4 => (2, 2),
            6 => (2, 3),
            8 => (2, 4),
            10 => (2, 5),
            12 => (3, 4),
            14 => (2, 7),
            16 => (4, 4),
            _ => Self::grid_heuristic(n),
        }
    }

    fn grid_heuristic(n: u8) -> (u8, u8) {
        let n = n.max(1) as u32;
        let cols = ((n as f64).sqrt().ceil() as u32).max(1);
        let rows = (n + cols - 1) / cols;
        (rows as u8, cols as u8)
    }

    fn set_count_and_dims(&mut self, count: u8) {
        self.terminal_count = count.max(1);
        let (rows, cols) = Self::grid_dims_for_count(self.terminal_count);
        self.grid_rows = rows;
        self.grid_cols = cols;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalSplitAxis {
    Vertical,
    Horizontal,
}

/// Wizard draft (single signal for simpler updates).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateWorkspaceDraft {
    pub name_input: String,
    pub cwd_display: String,
    pub nav_line: String,
    pub nav_log: Vec<String>,
    pub terminal_count: u8,
    pub grid_rows: u8,
    pub grid_cols: u8,
    pub agent_counts: [u8; 5],
    pub agents_skipped: bool,
}

impl Default for CreateWorkspaceDraft {
    fn default() -> Self {
        let (r, c) = WorkspaceEntry::grid_dims_for_count(1);
        Self {
            name_input: String::new(),
            cwd_display: String::new(),
            nav_line: String::new(),
            nav_log: Vec::new(),
            terminal_count: 1,
            grid_rows: r,
            grid_cols: c,
            agent_counts: [0; 5],
            agents_skipped: false,
        }
    }
}

/// Build `slot_agent_labels` length `n` from counts (order: agent 0, then 1, … by slot count).
#[must_use]
pub fn fleet_counts_to_slot_labels(n: usize, counts: &[u8; 5]) -> Vec<String> {
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    for (i, &c) in counts.iter().enumerate() {
        let slug = WORKSPACE_FLEET_AGENT_SLUGS[i];
        for _ in 0..c {
            if out.len() < n {
                out.push(slug.to_string());
            }
        }
    }
    while out.len() < n {
        out.push(String::new());
    }
    out.truncate(n);
    out
}

/// Rechtes Panel (Pi-inspirierte Harness-Ansicht): Agent-Stream vs. eingebetteter Browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RightPanelTab {
    Agent,
    Browser,
    Memory,
}

/// Kategorien in den Harness-Einstellungen (Befehlspalette).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HarnessSettingsCategory {
    General,
    Layout,
    Language,
    Agent,
}

#[derive(Clone, Copy)]
pub struct HarnessUiService {
    palette_open: RwSignal<bool>,
    settings_open: RwSignal<bool>,
    palette_query: RwSignal<String>,
    palette_selection: RwSignal<usize>,
    settings_category: RwSignal<HarnessSettingsCategory>,
}

impl HarnessUiService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            palette_open: RwSignal::new(false),
            settings_open: RwSignal::new(false),
            palette_query: RwSignal::new(String::new()),
            palette_selection: RwSignal::new(0),
            settings_category: RwSignal::new(HarnessSettingsCategory::General),
        }
    }

    #[must_use]
    pub fn palette_open(&self) -> RwSignal<bool> {
        self.palette_open
    }

    #[must_use]
    pub fn settings_open(&self) -> RwSignal<bool> {
        self.settings_open
    }

    #[must_use]
    pub fn palette_query(&self) -> RwSignal<String> {
        self.palette_query
    }

    #[must_use]
    pub fn palette_selection(&self) -> RwSignal<usize> {
        self.palette_selection
    }

    #[must_use]
    pub fn settings_category(&self) -> RwSignal<HarnessSettingsCategory> {
        self.settings_category
    }

    pub fn open_command_palette(&self) {
        self.palette_query.set(String::new());
        self.palette_selection.set(0);
        self.palette_open.set(true);
    }

    pub fn close_command_palette(&self) {
        self.palette_open.set(false);
    }

    pub fn toggle_command_palette(&self) {
        let next = !self.palette_open.get_untracked();
        if next {
            self.open_command_palette();
        } else {
            self.close_command_palette();
        }
    }

    pub fn open_settings(&self, cat: HarnessSettingsCategory) {
        self.settings_category.set(cat);
        self.close_command_palette();
        self.settings_open.set(true);
    }

    pub fn close_settings(&self) {
        self.settings_open.set(false);
    }
}

impl Default for HarnessUiService {
    fn default() -> Self {
        Self::new()
    }
}

fn read_local_storage(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}

fn write_local_storage(key: &str, value: &str) {
    let Some(w) = web_sys::window() else {
        return;
    };
    if let Ok(Some(s)) = w.local_storage() {
        let _ = s.set_item(key, value);
    }
}

/// Ein „Blatt“ innerhalb des eingebetteten Browsers (rechtes Panel), mit eigener URL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedBrowserTab {
    pub id: u64,
    pub url: String,
}

/// Unterscheidung: natives Child-Webview vs. SPA-iframe (Linux/Browser-CSR ohne Tauri).
#[derive(Clone, Copy)]
pub struct BrowserEmbedSurface(pub RwSignal<Option<String>>);

/// Application layout + workspace selection (sidebar, center, inspector).
#[derive(Clone, Copy)]
pub struct WorkbenchService {
    workspaces: RwSignal<Vec<WorkspaceEntry>>,
    active_id: RwSignal<Option<u64>>,
    sidebar_collapsed: RwSignal<bool>,
    right_collapsed: RwSignal<bool>,
    right_width_px: RwSignal<f64>,
    right_tab: RwSignal<RightPanelTab>,
    browser_url: RwSignal<String>,
    /// Mehrere Seiten im eingebetteten Browser (wie Browser-Tabs).
    embedded_browser_tabs: RwSignal<Vec<EmbeddedBrowserTab>>,
    embedded_browser_active_id: RwSignal<u64>,
    embedded_browser_next_id: RwSignal<u64>,
    harness_workspace_root: RwSignal<String>,
    workspace_next_id: RwSignal<u64>,
    create_wizard_open: RwSignal<bool>,
    create_wizard_step: RwSignal<u8>,
    create_wizard_draft: RwSignal<CreateWorkspaceDraft>,
}

impl WorkbenchService {
    /// Demo list until real workspace loading exists.
    #[must_use]
    pub fn new() -> Self {
        let browser_url = read_local_storage(HARNESS_BROWSER_URL_KEY)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| HARNESS_BROWSER_DEFAULT_URL.to_string());

        let harness_workspace_root =
            read_local_storage(HARNESS_WORKSPACE_ROOT_KEY).unwrap_or_default();

        let first_tab_id = 1_u64;

        let mut draft = CreateWorkspaceDraft::default();
        draft.cwd_display.clone_from(&harness_workspace_root);

        Self {
            workspaces: RwSignal::new(Vec::new()),
            active_id: RwSignal::new(None),
            sidebar_collapsed: RwSignal::new(false),
            right_collapsed: RwSignal::new(true),
            right_width_px: RwSignal::new(288.0),
            right_tab: RwSignal::new(RightPanelTab::Agent),
            browser_url: RwSignal::new(browser_url.clone()),
            embedded_browser_tabs: RwSignal::new(vec![EmbeddedBrowserTab {
                id: first_tab_id,
                url: browser_url,
            }]),
            embedded_browser_active_id: RwSignal::new(first_tab_id),
            embedded_browser_next_id: RwSignal::new(first_tab_id + 1),
            harness_workspace_root: RwSignal::new(harness_workspace_root.clone()),
            workspace_next_id: RwSignal::new(1),
            create_wizard_open: RwSignal::new(false),
            create_wizard_step: RwSignal::new(0),
            create_wizard_draft: RwSignal::new(draft),
        }
    }

    #[must_use]
    pub fn workspaces(&self) -> RwSignal<Vec<WorkspaceEntry>> {
        self.workspaces
    }

    #[must_use]
    pub fn active_id(&self) -> RwSignal<Option<u64>> {
        self.active_id
    }

    pub fn select_workspace(&self, id: u64) {
        self.active_id.set(Some(id));
    }

    pub fn rename_workspace(&self, id: u64, title: String) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        self.workspaces.update(|workspaces| {
            if let Some(workspace) = workspaces.iter_mut().find(|w| w.id == id) {
                workspace.title = title.to_string();
            }
        });
    }

    pub fn close_workspace(&self, id: u64) {
        self.workspaces.update(|workspaces| {
            let Some(index) = workspaces.iter().position(|w| w.id == id) else {
                return;
            };
            workspaces.remove(index);
            if self.active_id.get_untracked() == Some(id) {
                let next = workspaces
                    .get(index)
                    .or_else(|| index.checked_sub(1).and_then(|i| workspaces.get(i)))
                    .map(|workspace| workspace.id);
                self.active_id.set(next);
            }
        });
        drop_sessions_for_prefix(format!("{id}:"));
    }

    pub fn close_terminal(&self, workspace_id: u64, terminal_id: u64) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if workspace.terminal_count <= 1 {
                return;
            }
            let Some(index) = workspace.slot_ids.iter().position(|id| *id == terminal_id) else {
                return;
            };
            workspace.slot_ids.remove(index);
            workspace.slot_agent_labels.remove(index);
            if index < workspace.slot_pane_states.len() {
                workspace.slot_pane_states.remove(index);
            }
            workspace.set_count_and_dims(workspace.slot_agent_labels.len() as u8);
        });
        drop_sessions_for_prefix(format!("{workspace_id}:{terminal_id}:"));
    }

    #[must_use]
    pub fn sidebar_collapsed(&self) -> RwSignal<bool> {
        self.sidebar_collapsed
    }

    pub fn toggle_sidebar(&self) {
        self.sidebar_collapsed.update(|c| *c = !*c);
    }

    #[must_use]
    pub fn right_collapsed(&self) -> RwSignal<bool> {
        self.right_collapsed
    }

    pub fn toggle_right_panel(&self) {
        self.right_collapsed.update(|c| *c = !*c);
    }

    #[must_use]
    pub fn right_width_px(&self) -> RwSignal<f64> {
        self.right_width_px
    }

    #[must_use]
    pub fn right_active_tab(&self) -> RwSignal<RightPanelTab> {
        self.right_tab
    }

    pub fn set_right_tab(&self, tab: RightPanelTab) {
        self.right_tab.set(tab);
    }

    #[must_use]
    pub fn browser_url(&self) -> RwSignal<String> {
        self.browser_url
    }

    pub fn set_browser_url_text(&self, url: String) {
        self.browser_url.set(url);
    }

    pub fn persist_browser_url_from_input(&self, url: String) {
        let trimmed = url.trim().to_string();
        if !trimmed.is_empty() {
            write_local_storage(HARNESS_BROWSER_URL_KEY, &trimmed);
        }
        let aid = self.embedded_browser_active_id.get_untracked();
        self.embedded_browser_tabs.update(|tabs| {
            if let Some(t) = tabs.iter_mut().find(|t| t.id == aid) {
                t.url.clone_from(&trimmed);
            }
        });
        self.browser_url.set(trimmed);
    }

    #[must_use]
    pub fn embedded_browser_tabs(&self) -> RwSignal<Vec<EmbeddedBrowserTab>> {
        self.embedded_browser_tabs
    }

    #[must_use]
    pub fn embedded_browser_active_id(&self) -> RwSignal<u64> {
        self.embedded_browser_active_id
    }

    pub fn select_embedded_browser_tab(&self, tab_id: u64) {
        let url_opt = self
            .embedded_browser_tabs
            .get_untracked()
            .into_iter()
            .find(|t| t.id == tab_id)
            .map(|t| t.url);
        if let Some(url) = url_opt {
            self.embedded_browser_active_id.set(tab_id);
            self.browser_url.set(url.clone());
            if !url.trim().is_empty() {
                write_local_storage(HARNESS_BROWSER_URL_KEY, &url);
            }
        }
    }

    pub fn add_embedded_browser_tab(&self) -> u64 {
        let nid = self.embedded_browser_next_id.get_untracked();
        self.embedded_browser_next_id.set(nid + 1);
        let tab = EmbeddedBrowserTab {
            id: nid,
            url: String::new(),
        };
        self.embedded_browser_tabs.update(|tabs| tabs.push(tab));
        self.embedded_browser_active_id.set(nid);
        self.browser_url.set(String::new());
        nid
    }

    pub fn close_embedded_browser_tab(&self, tab_id: u64) {
        let mut tabs = self.embedded_browser_tabs.get_untracked();
        if tabs.len() <= 1 {
            return;
        }
        tabs.retain(|t| t.id != tab_id);
        self.embedded_browser_tabs.set(tabs.clone());

        let active = self.embedded_browser_active_id.get_untracked();
        if active == tab_id {
            let pick = tabs.first().expect("tabs non-empty").id;
            self.select_embedded_browser_tab(pick);
        }
    }

    #[must_use]
    pub fn harness_workspace_root(&self) -> RwSignal<String> {
        self.harness_workspace_root
    }

    pub fn set_harness_workspace_root_text(&self, path: String) {
        self.harness_workspace_root.set(path);
    }

    pub fn persist_harness_workspace_root(&self, path: String) {
        write_local_storage(HARNESS_WORKSPACE_ROOT_KEY, &path);
        self.harness_workspace_root.set(path);
    }

    // --- Create workspace wizard ---

    #[must_use]
    pub fn create_wizard_open(&self) -> RwSignal<bool> {
        self.create_wizard_open
    }

    #[must_use]
    pub fn create_wizard_step(&self) -> RwSignal<u8> {
        self.create_wizard_step
    }

    #[must_use]
    pub fn create_wizard_draft(&self) -> RwSignal<CreateWorkspaceDraft> {
        self.create_wizard_draft
    }

    pub fn open_create_workspace_wizard(&self) {
        let root = self.harness_workspace_root.get_untracked();
        let mut d = CreateWorkspaceDraft::default();
        d.cwd_display = root;
        d.nav_log.clear();
        d.nav_line.clear();
        d.name_input.clear();
        d.agents_skipped = false;
        d.agent_counts = [0; 5];
        self.create_wizard_draft.set(d);
        self.create_wizard_step.set(0);
        self.create_wizard_open.set(true);
    }

    pub fn close_create_workspace_wizard(&self) {
        self.create_wizard_open.set(false);
    }

    pub fn wizard_set_terminal_layout(&self, count: u8) {
        let count = count.clamp(1, 16);
        let (r, c) = WorkspaceEntry::grid_dims_for_count(count);
        self.create_wizard_draft.update(|d| {
            d.terminal_count = count;
            d.grid_rows = r;
            d.grid_cols = c;
        });
    }

    pub fn wizard_go_to_fleet_step(&self) -> Result<(), ()> {
        let d = self.create_wizard_draft.get_untracked();
        if d.cwd_display.trim().is_empty() {
            return Err(());
        }
        self.create_wizard_step.set(1);
        Ok(())
    }

    pub fn wizard_back_to_layout(&self) {
        self.create_wizard_step.set(0);
    }

    /// Sum of fleet agent counts.
    #[must_use]
    pub fn wizard_fleet_assigned(&self) -> u8 {
        self.create_wizard_draft
            .get()
            .agent_counts
            .iter()
            .copied()
            .sum()
    }

    pub fn wizard_set_agent_count(&self, idx: usize, value: u8) {
        if idx >= 5 {
            return;
        }
        let n = self.create_wizard_draft.get_untracked().terminal_count;
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = false;
            let max_for_slot = n.saturating_sub(
                d.agent_counts
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != idx)
                    .map(|(_, c)| *c)
                    .sum::<u8>(),
            );
            d.agent_counts[idx] = value.min(max_for_slot);
        });
    }

    pub fn wizard_agent_fill_all(&self, idx: usize) {
        if idx >= 5 {
            return;
        }
        let n = self.create_wizard_draft.get_untracked().terminal_count;
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = false;
            d.agent_counts = [0; 5];
            d.agent_counts[idx] = n;
        });
    }

    pub fn wizard_fleet_select_all(&self) {
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = false;
            for c in &mut d.agent_counts {
                *c = 0;
            }
            d.agent_counts[0] = d.terminal_count;
        });
    }

    pub fn wizard_fleet_one_each(&self) {
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = false;
            let n = d.terminal_count as usize;
            let base = n / 5;
            let rem = n % 5;
            for (i, c) in d.agent_counts.iter_mut().enumerate() {
                *c = (base + if i < rem { 1 } else { 0 }) as u8;
            }
        });
    }

    pub fn wizard_fleet_fill_evenly(&self) {
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = false;
            let n = d.terminal_count as usize;
            if n == 0 {
                return;
            }
            let base = n / 5;
            let rem = n % 5;
            for (i, c) in d.agent_counts.iter_mut().enumerate() {
                *c = (base + if i < rem { 1 } else { 0 }) as u8;
            }
        });
    }

    pub fn wizard_fleet_clear(&self) {
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = false;
            d.agent_counts = [0; 5];
        });
    }

    pub fn wizard_skip_agents(&self) {
        self.create_wizard_draft.update(|d| {
            d.agents_skipped = true;
            d.agent_counts = [0; 5];
        });
    }

    pub fn commit_create_workspace(&self) {
        let mut draft = self.create_wizard_draft.get_untracked();
        let cwd = draft.cwd_display.trim().to_string();
        if cwd.is_empty() {
            return;
        }
        let n = draft.terminal_count as usize;
        let (gr, gc) = (draft.grid_rows, draft.grid_cols);
        let id = self.workspace_next_id.get_untracked();
        self.workspace_next_id.set(id + 1);

        let title = {
            let t = draft.name_input.trim();
            if t.is_empty() {
                format!("Workspace {id}")
            } else {
                t.to_string()
            }
        };

        let slot_agent_labels = if draft.agents_skipped {
            vec![String::new(); n]
        } else {
            let assigned: u8 = draft.agent_counts.iter().sum();
            if assigned != draft.terminal_count {
                return;
            }
            fleet_counts_to_slot_labels(n, &draft.agent_counts)
        };

        let slot_ids: Vec<u64> = (1..=n as u64).collect();
        let slot_pane_states = slot_ids
            .iter()
            .copied()
            .map(SlotPaneState::default_for_slot)
            .collect();
        let entry = WorkspaceEntry {
            id,
            title,
            cwd,
            terminal_count: draft.terminal_count,
            grid_rows: gr,
            grid_cols: gc,
            slot_agent_labels,
            next_terminal_id: n as u64 + 1,
            slot_ids,
            slot_pane_states,
        };

        self.workspaces.update(|v| v.push(entry));
        self.active_id.set(Some(id));
        self.create_wizard_open.set(false);
        draft = CreateWorkspaceDraft::default();
        draft
            .cwd_display
            .clone_from(&self.harness_workspace_root.get_untracked());
        self.create_wizard_draft.set(draft);
        self.create_wizard_step.set(0);
    }

    pub fn append_nav_log(&self, line: String) {
        self.create_wizard_draft.update(|d| {
            d.nav_log.push(line);
            if d.nav_log.len() > 80 {
                let excess = d.nav_log.len() - 80;
                d.nav_log.drain(0..excess);
            }
        });
    }

    /// Look up the persisted pane state for a slot. Returns
    /// [`SlotPaneState::default_for_slot`] when nothing has been stored
    /// yet (fresh workspace, snapshot from before Phase 2.3).
    #[must_use]
    pub fn slot_panes(&self, workspace_id: u64, slot_id: u64) -> SlotPaneState {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| {
                    w.slot_ids
                        .iter()
                        .position(|id| *id == slot_id)
                        .and_then(|idx| w.slot_pane_states.get(idx).cloned())
                })
                .unwrap_or_else(|| SlotPaneState::default_for_slot(slot_id))
        })
    }

    /// Persist a slot's current split-pane layout back into the workspace
    /// entry. Quietly no-ops if the workspace or slot has vanished
    /// (e.g. closed mid-write).
    pub fn set_slot_panes(&self, workspace_id: u64, slot_id: u64, state: SlotPaneState) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            let Some(idx) = workspace.slot_ids.iter().position(|id| *id == slot_id) else {
                return;
            };
            // Keep parallel arrays aligned even on snapshots created before
            // this field existed.
            while workspace.slot_pane_states.len() < workspace.slot_ids.len() {
                let sid = workspace.slot_ids[workspace.slot_pane_states.len()];
                workspace
                    .slot_pane_states
                    .push(SlotPaneState::default_for_slot(sid));
            }
            if let Some(slot) = workspace.slot_pane_states.get_mut(idx) {
                if *slot != state {
                    *slot = state;
                }
            }
        });
    }

    pub fn set_wizard_cwd(&self, path: String) {
        self.create_wizard_draft.update(|d| {
            if d.name_input.trim().is_empty() {
                if let Some(name) = derive_workspace_name(&path) {
                    d.name_input = name;
                }
            }
            d.cwd_display = path;
        });
    }

    /// Serialisable snapshot of every workbench bit that should survive a
    /// restart. Transient state (wizard draft, command palette, embedded
    /// browser surface kind) is intentionally excluded.
    #[must_use]
    pub fn snapshot(&self) -> WorkbenchSnapshot {
        WorkbenchSnapshot {
            version: WORKBENCH_SNAPSHOT_VERSION,
            workspaces: self.workspaces.get_untracked(),
            active_id: self.active_id.get_untracked(),
            workspace_next_id: self.workspace_next_id.get_untracked(),
            sidebar_collapsed: self.sidebar_collapsed.get_untracked(),
            right_collapsed: self.right_collapsed.get_untracked(),
            right_width_px: self.right_width_px.get_untracked(),
            right_tab: self.right_tab.get_untracked(),
            embedded_browser_tabs: self.embedded_browser_tabs.get_untracked(),
            embedded_browser_active_id: self.embedded_browser_active_id.get_untracked(),
            embedded_browser_next_id: self.embedded_browser_next_id.get_untracked(),
        }
    }

    /// Apply a previously persisted snapshot. Mismatched / future versions
    /// are rejected silently so a stale file never breaks startup.
    pub fn hydrate(&self, snap: WorkbenchSnapshot) {
        if snap.version != WORKBENCH_SNAPSHOT_VERSION {
            return;
        }
        // Workspace list + selection
        self.workspaces.set(snap.workspaces);
        self.active_id.set(snap.active_id);
        self.workspace_next_id
            .set(snap.workspace_next_id.max(1));

        // Panel chrome
        self.sidebar_collapsed.set(snap.sidebar_collapsed);
        self.right_collapsed.set(snap.right_collapsed);
        if snap.right_width_px.is_finite() && snap.right_width_px > 120.0 {
            self.right_width_px.set(snap.right_width_px);
        }
        self.right_tab.set(snap.right_tab);

        // Embedded browser — keep at least one tab to match `new()` invariant.
        if !snap.embedded_browser_tabs.is_empty() {
            let tabs = snap.embedded_browser_tabs;
            let aid = if tabs.iter().any(|t| t.id == snap.embedded_browser_active_id) {
                snap.embedded_browser_active_id
            } else {
                tabs[0].id
            };
            let next = snap
                .embedded_browser_next_id
                .max(tabs.iter().map(|t| t.id).max().unwrap_or(0) + 1);
            let active_url = tabs
                .iter()
                .find(|t| t.id == aid)
                .map(|t| t.url.clone())
                .unwrap_or_default();
            self.embedded_browser_tabs.set(tabs);
            self.embedded_browser_active_id.set(aid);
            self.embedded_browser_next_id.set(next);
            if !active_url.is_empty() {
                self.browser_url.set(active_url);
            }
        }
    }
}

/// On-disk schema for the workbench layout. Versioned via
/// [`WORKBENCH_SNAPSHOT_VERSION`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkbenchSnapshot {
    pub version: u32,
    pub workspaces: Vec<WorkspaceEntry>,
    pub active_id: Option<u64>,
    pub workspace_next_id: u64,
    pub sidebar_collapsed: bool,
    pub right_collapsed: bool,
    pub right_width_px: f64,
    pub right_tab: RightPanelTab,
    pub embedded_browser_tabs: Vec<EmbeddedBrowserTab>,
    pub embedded_browser_active_id: u64,
    pub embedded_browser_next_id: u64,
}

/// Fire-and-forget cleanup of `sessions.json` entries whose key starts
/// with `prefix`. Called from close handlers; failures are swallowed so
/// a missing or transient IPC error never blocks the UI.
fn drop_sessions_for_prefix(prefix: String) {
    if prefix.is_empty() || !is_tauri_shell() {
        return;
    }
    spawn_local(async move {
        let _ = workbench_drop_sessions(prefix).await;
    });
}

/// Last meaningful path segment, used to auto-name a workspace from its
/// working directory. Handles both `/` and `\` separators, skips trailing
/// slashes, and rejects pathological inputs (root, empty, dots).
#[must_use]
pub fn derive_workspace_name(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return None;
    }
    let last = trimmed
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or("")
        .trim();
    if last.is_empty() || last == "." || last == ".." {
        return None;
    }
    Some(last.to_string())
}
