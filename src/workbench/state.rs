use crate::config::{
    HARNESS_BROWSER_DEFAULT_URL, HARNESS_BROWSER_URL_KEY, HARNESS_WORKSPACE_ROOT_KEY,
};
use crate::tauri_bridge::{self, is_tauri_shell, workbench_drop_sessions};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// True while the workspace is in inline-configuration mode (the
    /// configurator UI is shown instead of the terminal grid). Newly
    /// created workspaces start in this state; committing the
    /// configuration flips it to `false`.
    #[serde(default)]
    pub configuring: bool,
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
            configuring: false,
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
            9 => (3, 3),
            12 => (3, 4),
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
    App,
    Workspace,
    AgentProvider,
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
            settings_category: RwSignal::new(HarnessSettingsCategory::App),
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

fn normalize_browser_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains("://") {
        return trimmed.to_string();
    }
    format!("https://{trimmed}")
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
    /// Drafts for workspaces currently in inline-configuration mode,
    /// keyed by workspace id. Entries are removed on commit or cancel.
    workspace_drafts: RwSignal<HashMap<u64, CreateWorkspaceDraft>>,
    /// Step indicator for the inline configurator (0 = layout, 1 = fleet).
    /// Per-workspace, keyed alongside `workspace_drafts`.
    workspace_config_steps: RwSignal<HashMap<u64, u8>>,
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

        Self {
            workspaces: RwSignal::new(Vec::new()),
            active_id: RwSignal::new(None),
            sidebar_collapsed: RwSignal::new(false),
            right_collapsed: RwSignal::new(false),
            right_width_px: RwSignal::new(420.0),
            right_tab: RwSignal::new(RightPanelTab::Agent),
            browser_url: RwSignal::new(browser_url.clone()),
            embedded_browser_tabs: RwSignal::new(vec![EmbeddedBrowserTab {
                id: first_tab_id,
                url: browser_url,
            }]),
            embedded_browser_active_id: RwSignal::new(first_tab_id),
            embedded_browser_next_id: RwSignal::new(first_tab_id + 1),
            harness_workspace_root: RwSignal::new(harness_workspace_root),
            workspace_next_id: RwSignal::new(1),
            workspace_drafts: RwSignal::new(HashMap::new()),
            workspace_config_steps: RwSignal::new(HashMap::new()),
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
        let normalized = normalize_browser_url(&url);
        if !normalized.is_empty() {
            write_local_storage(HARNESS_BROWSER_URL_KEY, &normalized);
        }
        let aid = self.embedded_browser_active_id.get_untracked();
        self.embedded_browser_tabs.update(|tabs| {
            if let Some(t) = tabs.iter_mut().find(|t| t.id == aid) {
                t.url.clone_from(&normalized);
            }
        });
        self.browser_url.set(normalized);
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

    // --- Inline workspace configuration ---

    #[must_use]
    pub fn workspace_drafts(&self) -> RwSignal<HashMap<u64, CreateWorkspaceDraft>> {
        self.workspace_drafts
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn workspace_config_step(&self, id: u64) -> u8 {
        self.workspace_config_steps
            .with(|m| m.get(&id).copied().unwrap_or(0))
    }

    #[must_use]
    pub fn workspace_config_steps(&self) -> RwSignal<HashMap<u64, u8>> {
        self.workspace_config_steps
    }

    pub fn set_workspace_config_step(&self, id: u64, step: u8) {
        self.workspace_config_steps.update(|m| {
            m.insert(id, step);
        });
    }

    #[must_use]
    pub fn workspace_draft(&self, id: u64) -> CreateWorkspaceDraft {
        self.workspace_drafts
            .with(|m| m.get(&id).cloned().unwrap_or_default())
    }

    pub fn update_workspace_draft(&self, id: u64, f: impl FnOnce(&mut CreateWorkspaceDraft)) {
        self.workspace_drafts.update(|m| {
            let entry = m.entry(id).or_default();
            f(entry);
        });
    }

    /// Creates a new workspace in inline-configuration mode and selects it.
    /// The configurator UI renders inside the workspace surface itself —
    /// no modal. Returns the new workspace id.
    pub fn start_inline_configure(&self) -> u64 {
        let id = self.workspace_next_id.get_untracked();
        self.workspace_next_id.set(id + 1);

        let root = self.harness_workspace_root.get_untracked();
        let mut draft = CreateWorkspaceDraft::default();
        draft.cwd_display = root;

        let entry = WorkspaceEntry {
            id,
            title: format!("Workspace {id}"),
            cwd: String::new(),
            terminal_count: 1,
            grid_rows: 1,
            grid_cols: 1,
            next_terminal_id: 1,
            slot_ids: Vec::new(),
            slot_agent_labels: Vec::new(),
            slot_pane_states: Vec::new(),
            configuring: true,
        };
        self.workspaces.update(|v| v.push(entry));
        self.workspace_drafts.update(|m| {
            m.insert(id, draft);
        });
        self.workspace_config_steps.update(|m| {
            m.insert(id, 0);
        });
        self.active_id.set(Some(id));
        id
    }

    pub fn cancel_inline_configure(&self, id: u64) {
        self.workspace_drafts.update(|m| {
            m.remove(&id);
        });
        self.workspace_config_steps.update(|m| {
            m.remove(&id);
        });
        self.close_workspace(id);
    }

    pub fn set_workspace_terminal_layout(&self, id: u64, count: u8) {
        let count = count.clamp(1, 16);
        let (r, c) = WorkspaceEntry::grid_dims_for_count(count);
        self.update_workspace_draft(id, |d| {
            d.terminal_count = count;
            d.grid_rows = r;
            d.grid_cols = c;
        });
    }

    pub fn workspace_go_to_fleet_step(&self, id: u64) -> Result<(), ()> {
        let d = self.workspace_draft(id);
        if d.cwd_display.trim().is_empty() {
            return Err(());
        }
        self.set_workspace_config_step(id, 1);
        Ok(())
    }

    pub fn workspace_back_to_layout(&self, id: u64) {
        self.set_workspace_config_step(id, 0);
    }

    #[must_use]
    pub fn workspace_fleet_assigned(&self, id: u64) -> u8 {
        self.workspace_draft(id).agent_counts.iter().copied().sum()
    }

    pub fn set_workspace_agent_count(&self, id: u64, idx: usize, value: u8) {
        if idx >= 5 {
            return;
        }
        let n = self.workspace_draft(id).terminal_count;
        self.update_workspace_draft(id, |d| {
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

    pub fn workspace_agent_fill_all(&self, id: u64, idx: usize) {
        if idx >= 5 {
            return;
        }
        let n = self.workspace_draft(id).terminal_count;
        self.update_workspace_draft(id, |d| {
            d.agents_skipped = false;
            d.agent_counts = [0; 5];
            d.agent_counts[idx] = n;
        });
    }

    pub fn workspace_fleet_select_all(&self, id: u64) {
        self.update_workspace_draft(id, |d| {
            d.agents_skipped = false;
            for c in &mut d.agent_counts {
                *c = 0;
            }
            d.agent_counts[0] = d.terminal_count;
        });
    }

    pub fn workspace_fleet_one_each(&self, id: u64) {
        self.update_workspace_draft(id, |d| {
            d.agents_skipped = false;
            let n = d.terminal_count as usize;
            let base = n / 5;
            let rem = n % 5;
            for (i, c) in d.agent_counts.iter_mut().enumerate() {
                *c = (base + if i < rem { 1 } else { 0 }) as u8;
            }
        });
    }

    pub fn workspace_fleet_fill_evenly(&self, id: u64) {
        self.update_workspace_draft(id, |d| {
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

    pub fn workspace_fleet_clear(&self, id: u64) {
        self.update_workspace_draft(id, |d| {
            d.agents_skipped = false;
            d.agent_counts = [0; 5];
        });
    }

    pub fn workspace_skip_agents(&self, id: u64) {
        self.update_workspace_draft(id, |d| {
            d.agents_skipped = true;
            d.agent_counts = [0; 5];
        });
    }

    /// Finalises the inline configuration: materialises slot/pane state
    /// from the draft, flips `configuring` to false, and drops the draft.
    pub fn commit_inline_configure(&self, id: u64) {
        let draft = self.workspace_draft(id);
        let cwd = draft.cwd_display.trim().to_string();
        if cwd.is_empty() {
            return;
        }
        // Idempotent backend call: writes pointer files (CLAUDE.md,
        // AGENTS.md, GEMINI.md, .cursorrules, opencode.md) only if
        // missing, so coding agents pick up `.blxcode/memory/` on first
        // session without any UI install step.
        if is_tauri_shell() {
            let ws_cwd = cwd.clone();
            spawn_local(async move {
                let agents = vec![
                    "claude".into(),
                    "codex".into(),
                    "gemini".into(),
                    "cursor".into(),
                    "opencode".into(),
                ];
                let _ = tauri_bridge::memory_install_pointers(&ws_cwd, agents).await;
            });
        }
        let n = draft.terminal_count as usize;
        let (gr, gc) = (draft.grid_rows, draft.grid_cols);

        let slot_agent_labels = if draft.agents_skipped {
            vec![String::new(); n]
        } else {
            let assigned: u8 = draft.agent_counts.iter().sum();
            if assigned != draft.terminal_count {
                return;
            }
            fleet_counts_to_slot_labels(n, &draft.agent_counts)
        };

        let title = {
            let t = draft.name_input.trim();
            if t.is_empty() {
                format!("Workspace {id}")
            } else {
                t.to_string()
            }
        };

        let slot_ids: Vec<u64> = (1..=n as u64).collect();
        let slot_pane_states: Vec<SlotPaneState> = slot_ids
            .iter()
            .copied()
            .map(SlotPaneState::default_for_slot)
            .collect();

        self.workspaces.update(|v| {
            let Some(ws) = v.iter_mut().find(|w| w.id == id) else {
                return;
            };
            ws.title = title;
            ws.cwd = cwd;
            ws.terminal_count = draft.terminal_count;
            ws.grid_rows = gr;
            ws.grid_cols = gc;
            ws.slot_ids = slot_ids;
            ws.slot_agent_labels = slot_agent_labels;
            ws.slot_pane_states = slot_pane_states;
            ws.next_terminal_id = n as u64 + 1;
            ws.configuring = false;
        });

        self.workspace_drafts.update(|m| {
            m.remove(&id);
        });
        self.workspace_config_steps.update(|m| {
            m.remove(&id);
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

    pub fn set_workspace_cwd(&self, id: u64, path: String) {
        self.update_workspace_draft(id, |d| {
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
            // Browser tabs are session-scoped — never persisted.
            embedded_browser_tabs: Vec::new(),
            embedded_browser_active_id: 0,
            embedded_browser_next_id: 1,
        }
    }

    /// Apply a previously persisted snapshot. Mismatched / future versions
    /// are rejected silently so a stale file never breaks startup.
    pub fn hydrate(&self, snap: WorkbenchSnapshot) {
        if snap.version != WORKBENCH_SNAPSHOT_VERSION {
            return;
        }
        // Workspace list + selection. Re-seed inline drafts for any
        // workspace persisted in configuring state — drafts are transient
        // and not serialised, so without this the configurator would
        // render against an empty draft.
        let root = self.harness_workspace_root.get_untracked();
        self.workspace_drafts.update(|m| {
            m.clear();
            for ws in &snap.workspaces {
                if ws.configuring {
                    let mut d = CreateWorkspaceDraft::default();
                    if !root.is_empty() {
                        d.cwd_display.clone_from(&root);
                    }
                    m.insert(ws.id, d);
                }
            }
        });
        // If the persisted state has no workspaces left, restart numbering
        // from 1 — otherwise the next "+" would say "Workspace 7" after a
        // clean app with nothing in it.
        let next_id = if snap.workspaces.is_empty() {
            1
        } else {
            snap.workspace_next_id.max(1)
        };
        // Backfill memory pointer files for any persisted workspace that
        // predates this feature. Idempotent backend call; missing pointers
        // are written, existing ones left alone.
        if is_tauri_shell() {
            for ws in &snap.workspaces {
                if ws.configuring || ws.cwd.trim().is_empty() {
                    continue;
                }
                let cwd = ws.cwd.clone();
                spawn_local(async move {
                    let agents = vec![
                        "claude".into(),
                        "codex".into(),
                        "gemini".into(),
                        "cursor".into(),
                        "opencode".into(),
                    ];
                    let _ = tauri_bridge::memory_install_pointers(&cwd, agents).await;
                });
            }
        }
        self.workspaces.set(snap.workspaces);
        self.active_id.set(snap.active_id);
        self.workspace_next_id.set(next_id);

        // Panel chrome
        self.sidebar_collapsed.set(snap.sidebar_collapsed);
        self.right_collapsed.set(snap.right_collapsed);
        if snap.right_width_px.is_finite() && snap.right_width_px > 120.0 {
            self.right_width_px.set(snap.right_width_px);
        }
        self.right_tab.set(snap.right_tab);

        // Embedded browser tabs are intentionally NOT restored across
        // restarts — each launch starts with a single default tab.
        let _ = snap.embedded_browser_tabs;
        let _ = snap.embedded_browser_active_id;
        let _ = snap.embedded_browser_next_id;
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
