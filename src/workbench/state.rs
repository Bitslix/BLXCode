use crate::agent_wire::AgentContextItem;
use crate::config::{
    HARNESS_BROWSER_DEFAULT_URL, HARNESS_BROWSER_URL_KEY, HARNESS_WORKSPACE_ROOT_KEY,
    MEMORY_COLOR_PRESETS_STORAGE_KEY,
};
use crate::tauri_bridge::{
    is_tauri_shell, workbench_drop_sessions, workbench_extract_sessions_prefix,
    workbench_merge_sessions_workspace, workspace_ensure_agents,
};
use crate::workbench::agent_timeline::TimelineItem;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    /// Stable, globally-unique identifier (UUID v4 hex, no dashes) used as
    /// the prefix of every `terminal_key` and therefore as the
    /// persistence key into `notifications.json` and `sessions.json`.
    /// Unlike `id` (which is a session-scoped `u64` reused for in-memory
    /// references), this never collides across workspace creation /
    /// deletion / re-creation cycles — closing "Workspace 1" and
    /// immediately creating a new "Workspace 1" produces a fresh
    /// `storage_key`, so the new workspace cannot inherit notification
    /// counts or PTY session references from the old one.
    #[serde(default)]
    pub storage_key: String,
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
    /// Persisted agent chat timeline for this workspace folder.
    #[serde(default)]
    pub agent_timeline: Vec<TimelineItem>,
    /// Draft text in the agent compose field (same workspace binding).
    #[serde(default)]
    pub agent_compose_draft: String,
    /// Memory/Learnings context attached to the next BLXCode Agent turns.
    #[serde(default)]
    pub agent_context_items: Vec<AgentContextItem>,
    /// Display/color/visibility overrides for memory categories in this workspace.
    #[serde(default)]
    pub memory_category_settings: HashMap<String, MemoryCategorySettings>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCategorySettings {
    pub label: String,
    pub color: String,
    pub show_in_sidebar: bool,
    pub show_in_graph: bool,
}

impl MemoryCategorySettings {
    #[must_use]
    pub fn for_category(key: &str) -> Self {
        match key {
            "learnings" => Self {
                label: "Learnings".into(),
                color: "#67e8f9".into(),
                show_in_sidebar: true,
                show_in_graph: true,
            },
            _ => Self {
                label: "Memory".into(),
                color: "#7dd3fc".into(),
                show_in_sidebar: true,
                show_in_graph: true,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryColorPreset {
    pub id: String,
    pub label: String,
    pub color: String,
}

#[must_use]
pub fn default_memory_color_presets() -> Vec<MemoryColorPreset> {
    vec![
        MemoryColorPreset {
            id: "memory-blue".into(),
            label: "Memory Blue".into(),
            color: "#7dd3fc".into(),
        },
        MemoryColorPreset {
            id: "learnings-teal".into(),
            label: "Learnings Teal".into(),
            color: "#67e8f9".into(),
        },
        MemoryColorPreset {
            id: "research-violet".into(),
            label: "Research Violet".into(),
            color: "#c4b5fd".into(),
        },
        MemoryColorPreset {
            id: "tasks-amber".into(),
            label: "Tasks Amber".into(),
            color: "#fbbf24".into(),
        },
        MemoryColorPreset {
            id: "archive-slate".into(),
            label: "Archive Slate".into(),
            color: "#9ca3af".into(),
        },
    ]
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
    /// Fresh UUID v4 in compact hex (no dashes) — used for the
    /// `storage_key` field on every new workspace. Legacy snapshots
    /// deserialize this as an empty string, then the snapshot backfill step
    /// assigns a UUID and migrates matching session keys before hydration.
    #[must_use]
    pub fn new_storage_key() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }

    #[must_use]
    pub fn empty_surface(id: u64) -> Self {
        Self {
            id,
            storage_key: Self::new_storage_key(),
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
            agent_timeline: Vec::new(),
            agent_compose_draft: String::new(),
            agent_context_items: Vec::new(),
            memory_category_settings: HashMap::new(),
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

#[inline]
fn normalize_cwd_key(path: &str) -> String {
    path.trim().trim_end_matches(['/', '\\']).to_string()
}

/// True when the workspace has a non-empty working-directory path (not
/// merely a wizard shell before the user picks a folder).
#[inline]
pub(crate) fn workspace_entry_has_folder(ws: &WorkspaceEntry) -> bool {
    !normalize_cwd_key(&ws.cwd).is_empty()
}

fn spawn_ensure_agents_layout(cwd: String) {
    let trimmed = cwd.trim();
    if trimmed.is_empty() || !is_tauri_shell() {
        return;
    }
    let cwd = trimmed.to_owned();
    spawn_local(async move {
        let _ = workspace_ensure_agents(&cwd).await;
    });
}

fn normalize_workspace_agent_labels(
    terminal_count: usize,
    agent_slugs: &[String],
) -> Result<Vec<String>, String> {
    if agent_slugs.len() > terminal_count {
        return Err(format!(
            "too many agent slugs for {terminal_count} terminal(s)"
        ));
    }
    let mut out = Vec::with_capacity(terminal_count);
    for slug in agent_slugs {
        let normalized = slug.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "claude" | "codex" | "gemini" | "opencode" | "cursor" => out.push(normalized),
            _ => return Err(format!("unsupported agent slug: {slug}")),
        }
    }
    while out.len() < terminal_count {
        out.push(String::new());
    }
    Ok(out)
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaceItem {
    pub workspace: WorkspaceEntry,
    /// JSON object string: map of terminal_key → SessionStart payload.
    #[serde(default)]
    pub sessions_terminals_json: String,
}

impl Default for RecentWorkspaceItem {
    fn default() -> Self {
        Self {
            workspace: WorkspaceEntry::empty_surface(0),
            sessions_terminals_json: "{}".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyStorageMigration {
    pub old_workspace_key: String,
    pub new_workspace_key: String,
}

fn rewrite_sessions_terminals_json(raw: &str, old_key: &str, new_key: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(raw.trim()) else {
        return raw.to_string();
    };
    let Some(map) = value.as_object_mut() else {
        return raw.to_string();
    };
    let old_prefix = format!("{old_key}:");
    let new_prefix = format!("{new_key}:");
    let keys: Vec<String> = map
        .keys()
        .filter(|key| key.starts_with(&old_prefix))
        .cloned()
        .collect();
    if keys.is_empty() {
        return raw.to_string();
    }
    for key in keys {
        if let Some(entry) = map.remove(&key) {
            let suffix = key.strip_prefix(&old_prefix).unwrap_or_default();
            map.insert(format!("{new_prefix}{suffix}"), entry);
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| raw.to_string())
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
    Memory,
    Voice,
}

#[derive(Clone, Copy)]
pub struct HarnessUiService {
    palette_open: RwSignal<bool>,
    settings_open: RwSignal<bool>,
    quick_open_open: RwSignal<bool>,
    palette_query: RwSignal<String>,
    palette_selection: RwSignal<usize>,
    quick_open_query: RwSignal<String>,
    quick_open_selection: RwSignal<usize>,
    settings_category: RwSignal<HarnessSettingsCategory>,
}

impl HarnessUiService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            palette_open: RwSignal::new(false),
            settings_open: RwSignal::new(false),
            quick_open_open: RwSignal::new(false),
            palette_query: RwSignal::new(String::new()),
            palette_selection: RwSignal::new(0),
            quick_open_query: RwSignal::new(String::new()),
            quick_open_selection: RwSignal::new(0),
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
    pub fn quick_open_open(&self) -> RwSignal<bool> {
        self.quick_open_open
    }

    #[must_use]
    pub fn quick_open_query(&self) -> RwSignal<String> {
        self.quick_open_query
    }

    #[must_use]
    pub fn quick_open_selection(&self) -> RwSignal<usize> {
        self.quick_open_selection
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
        self.close_quick_open();
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

    pub fn open_quick_open(&self) {
        self.close_command_palette();
        self.close_settings();
        self.quick_open_query.set(String::new());
        self.quick_open_selection.set(0);
        self.quick_open_open.set(true);
    }

    pub fn close_quick_open(&self) {
        self.quick_open_open.set(false);
    }

    pub fn toggle_quick_open(&self) {
        let next = !self.quick_open_open.get_untracked();
        if next {
            self.open_quick_open();
        } else {
            self.close_quick_open();
        }
    }

    pub fn open_settings(&self, cat: HarnessSettingsCategory) {
        self.settings_category.set(cat);
        self.close_command_palette();
        self.close_quick_open();
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

fn read_memory_color_presets() -> Vec<MemoryColorPreset> {
    read_local_storage(MEMORY_COLOR_PRESETS_STORAGE_KEY)
        .and_then(|raw| serde_json::from_str::<Vec<MemoryColorPreset>>(&raw).ok())
        .filter(|presets| !presets.is_empty())
        .unwrap_or_else(default_memory_color_presets)
}

fn write_memory_color_presets(presets: &[MemoryColorPreset]) {
    if let Ok(raw) = serde_json::to_string(presets) {
        write_local_storage(MEMORY_COLOR_PRESETS_STORAGE_KEY, &raw);
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

/// Push a navigation entry into the tab's history stack.
///
/// Behaviour matches a browser address bar:
/// - empty URL clears the tab (no history change beyond `url`).
/// - navigating to the same URL we already point at is a no-op.
/// - any forward entries past `history_index` are truncated (classic
///   "navigate from middle of stack").
fn push_history_entry(t: &mut EmbeddedBrowserTab, url: &str) {
    t.url = url.to_string();
    if url.trim().is_empty() {
        return;
    }
    if t.history.is_empty() {
        t.history.push(url.to_string());
        t.history_index = 0;
        return;
    }
    if t.history
        .get(t.history_index)
        .map(|s| s.as_str() == url)
        .unwrap_or(false)
    {
        return;
    }
    t.history.truncate(t.history_index + 1);
    t.history.push(url.to_string());
    t.history_index = t.history.len() - 1;
}

/// Ein „Blatt“ innerhalb des eingebetteten Browsers (rechtes Panel), mit eigener URL.
///
/// `history` ist ein parent-seitiger Navigations-Stack: jeder explizite
/// `navigate` (URL-Bar, Shortlink, programmatisch) trunkiert den Stack
/// hinter `history_index` und pusht die neue URL. Back/Forward bewegen
/// `history_index`. In-iframe Link-Klicks tauchen hier NICHT auf
/// (cross-origin iframes verbieten dem Parent das Auslesen der Location).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedBrowserTab {
    pub id: u64,
    pub url: String,
    #[serde(default)]
    pub history: Vec<String>,
    #[serde(default)]
    pub history_index: usize,
}

/// Unterscheidung: natives Child-Webview vs. SPA-iframe (Linux/Browser-CSR ohne Tauri).
#[derive(Clone, Copy)]
pub struct BrowserEmbedSurface(pub RwSignal<Option<String>>);

/// Application layout + workspace selection (sidebar, center, inspector).
#[derive(Clone, Copy)]
pub struct WorkbenchService {
    workspaces: RwSignal<Vec<WorkspaceEntry>>,
    active_id: RwSignal<Option<u64>>,
    recent_workspaces: RwSignal<Vec<RecentWorkspaceItem>>,
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
    /// Live registry of PTY session ids keyed by `"{workspace_id}:{slot_id}:{pane_id}"`.
    /// Each `TerminalCell` registers on spawn and clears on close so the
    /// agent can address terminals by slot via the harness tools without
    /// reaching into per-cell local state.
    pty_sessions: RwSignal<HashMap<String, u64>>,
    /// When set, [`MemoryPanel`] opens this note (API path, e.g. `learnings/topic.md`).
    pending_memory_note: RwSignal<Option<String>>,
    /// Bumped when the terminal grid gains real layout (wizard commit, etc.)
    /// so cells refit xterm/PTY after flex/grid reflow.
    terminal_layout_tick: RwSignal<u32>,
    /// Unread counts per `"{workspace_id}:{slot_id}:{pane_id}"` from agent notify hooks.
    notifications: RwSignal<HashMap<String, u32>>,
    /// Keys recently cleared in-memory while the async backend disk-write is still in flight.
    /// The notification poller filters these out so a freshly-cleared key cannot reappear
    /// before `workbench_clear_terminal_notifications` lands on disk.
    pending_clears: RwSignal<HashSet<String>>,
    /// Last focused terminal per workspace, keyed by the workspace's
    /// `storage_key` (UUID). Survives workspace switches. The value is the
    /// full `terminal_key`.
    focused_terminal_by_workspace: RwSignal<HashMap<String, String>>,
    memory_color_presets: RwSignal<Vec<MemoryColorPreset>>,
}

/// Sidebar badge aggregate for one workspace row. A single total across
/// every terminal in the workspace — per-agent breakdown is unnecessary
/// because the user resolves notifications by clicking into the specific
/// terminal that fired one (visually identifiable via the pulsing
/// titlebar), and a typical workspace only carries one unread per agent
/// at a time anyway.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceNotificationCounts {
    pub total_unread: u32,
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
        let memory_color_presets = read_memory_color_presets();

        let first_tab_id = 1_u64;

        Self {
            workspaces: RwSignal::new(Vec::new()),
            active_id: RwSignal::new(None),
            recent_workspaces: RwSignal::new(Vec::new()),
            sidebar_collapsed: RwSignal::new(false),
            right_collapsed: RwSignal::new(false),
            right_width_px: RwSignal::new(420.0),
            right_tab: RwSignal::new(RightPanelTab::Agent),
            browser_url: RwSignal::new(browser_url.clone()),
            embedded_browser_tabs: RwSignal::new(vec![{
                let history = if browser_url.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![browser_url.clone()]
                };
                EmbeddedBrowserTab {
                    id: first_tab_id,
                    url: browser_url,
                    history,
                    history_index: 0,
                }
            }]),
            embedded_browser_active_id: RwSignal::new(first_tab_id),
            embedded_browser_next_id: RwSignal::new(first_tab_id + 1),
            harness_workspace_root: RwSignal::new(harness_workspace_root),
            workspace_next_id: RwSignal::new(1),
            workspace_drafts: RwSignal::new(HashMap::new()),
            workspace_config_steps: RwSignal::new(HashMap::new()),
            pty_sessions: RwSignal::new(HashMap::new()),
            pending_memory_note: RwSignal::new(None),
            terminal_layout_tick: RwSignal::new(0),
            notifications: RwSignal::new(HashMap::new()),
            pending_clears: RwSignal::new(HashSet::new()),
            focused_terminal_by_workspace: RwSignal::new(HashMap::new()),
            memory_color_presets: RwSignal::new(memory_color_presets),
        }
    }

    pub fn notifications(&self) -> RwSignal<HashMap<String, u32>> {
        self.notifications
    }

    /// All currently-live terminal keys (`"{ws}:{slot}:{pane}"`) across
    /// every open workspace. Used to prune stale entries from
    /// `notifications.json` after slots or panes are closed.
    #[must_use]
    pub fn live_terminal_keys(&self) -> Vec<String> {
        self.workspaces.with_untracked(|list| {
            let mut out = Vec::new();
            for ws in list {
                for (slot_idx, &slot_id) in ws.slot_ids.iter().enumerate() {
                    let panes = ws
                        .slot_pane_states
                        .get(slot_idx)
                        .map(|p| p.pane_ids.clone())
                        .unwrap_or_else(|| SlotPaneState::default_for_slot(slot_id).pane_ids);
                    for pane_id in panes {
                        out.push(format!("{}:{}:{}", ws.storage_key, slot_id, pane_id));
                    }
                }
            }
            out
        })
    }

    pub fn focused_terminal_by_workspace(&self) -> RwSignal<HashMap<String, String>> {
        self.focused_terminal_by_workspace
    }

    /// Storage-key (UUID) of the currently active workspace, if any.
    /// Used by the notification poller to compare against a notification's
    /// terminal-key prefix without going through the `u64` `id` indirection.
    #[must_use]
    pub fn active_workspace_storage_key(&self) -> Option<String> {
        let active_id = self.active_id.get_untracked()?;
        self.workspaces.with_untracked(|list| {
            list.iter()
                .find(|w| w.id == active_id)
                .map(|w| w.storage_key.clone())
        })
    }

    pub fn set_notifications(&self, mut map: HashMap<String, u32>) {
        let pending = self.pending_clears.get_untracked();
        if !pending.is_empty() {
            for key in pending.iter() {
                map.remove(key);
            }
        }
        self.notifications.set(map);
    }

    #[must_use]
    pub fn terminal_unread_count(&self, terminal_key: &str) -> u32 {
        self.notifications
            .with(|m| m.get(terminal_key).copied().unwrap_or(0))
    }

    fn clear_terminal_notifications(&self, terminal_key: &str) {
        let key = terminal_key.to_string();
        self.notifications.update(|m| {
            m.remove(&key);
        });
        self.pending_clears.update(|p| {
            p.insert(key.clone());
        });
        if crate::tauri_bridge::is_tauri_shell() {
            let pending_clears = self.pending_clears;
            leptos::task::spawn_local(async move {
                let _ =
                    crate::tauri_bridge::workbench_clear_terminal_notifications(key.clone()).await;
                // Backend finished the disk clear. Any future
                // notify hook firing for this terminal must be allowed
                // through the next poll — drop the suppression entry now,
                // otherwise the poller keeps stripping fresh notifications
                // (the original bug: unread re-armed by the agent never
                // surfaced because the key never escaped pending_clears).
                pending_clears.update(|p| {
                    p.remove(&key);
                });
            });
        } else {
            self.pending_clears.update(|p| {
                p.remove(&key);
            });
        }
    }

    fn agent_slug_for_terminal_key(&self, key: &str) -> Option<String> {
        let storage_key = super::agent_accent::terminal_key_storage_key(key)?;
        let slot_id = key.split(':').nth(1)?.parse::<u64>().ok()?;
        self.workspaces.with_untracked(|list| {
            let ws = list.iter().find(|w| w.storage_key == storage_key)?;
            let idx = ws.slot_ids.iter().position(|&id| id == slot_id)?;
            ws.slot_agent_labels
                .get(idx)
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
        })
    }

    fn notification_ack_keys_for_terminal(&self, terminal_key: &str) -> Vec<String> {
        let mut keys = vec![terminal_key.to_string()];
        let Some(storage_key) = super::agent_accent::terminal_key_storage_key(terminal_key) else {
            return keys;
        };
        let Some(agent_slug) = self.agent_slug_for_terminal_key(terminal_key) else {
            return keys;
        };
        self.workspaces.with_untracked(|list| {
            let Some(ws) = list.iter().find(|w| w.storage_key == storage_key) else {
                return;
            };
            for (idx, &slot_id) in ws.slot_ids.iter().enumerate() {
                let slot_agent = ws
                    .slot_agent_labels
                    .get(idx)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_default();
                if slot_agent != agent_slug {
                    continue;
                }
                let panes = ws
                    .slot_pane_states
                    .get(idx)
                    .map(|state| state.pane_ids.clone())
                    .unwrap_or_else(|| SlotPaneState::default_for_slot(slot_id).pane_ids);
                for pane_id in panes {
                    keys.push(format!("{}:{}:{}", ws.storage_key, slot_id, pane_id));
                }
            }
        });
        keys.sort();
        keys.dedup();
        keys
    }

    #[must_use]
    pub fn is_terminal_focused(&self, terminal_key: &str) -> bool {
        let Some(storage_key) = super::agent_accent::terminal_key_storage_key(terminal_key) else {
            return false;
        };
        self.focused_terminal_by_workspace.with_untracked(|m| {
            m.get(&storage_key)
                .map(|k| k == terminal_key)
                .unwrap_or(false)
        })
    }

    /// Focus a terminal: remember per workspace and acknowledge unread on
    /// that terminal's agent slot. With a workspace-level badge, clicking a
    /// Claude/Codex/Cursor terminal should clear that agent's contribution
    /// instead of leaving the badge looking stuck.
    pub fn focus_terminal(&self, terminal_key: String) {
        let Some(storage_key) = super::agent_accent::terminal_key_storage_key(&terminal_key) else {
            return;
        };
        for key in self.notification_ack_keys_for_terminal(&terminal_key) {
            self.clear_terminal_notifications(&key);
        }
        self.focused_terminal_by_workspace.update(|m| {
            m.insert(storage_key, terminal_key);
        });
    }

    /// True when a notification's terminal key still maps to an agent-attached
    /// slot in the open workspace. Filters out ghosts from previous sessions
    /// (closed slots and slots without an agent label).
    fn notification_key_is_live(&self, key: &str) -> bool {
        let Some(storage_key) = super::agent_accent::terminal_key_storage_key(key) else {
            return false;
        };
        let Some(slot_id) = key.split(':').nth(1).and_then(|s| s.parse::<u64>().ok()) else {
            return false;
        };
        self.workspaces.with_untracked(|list| {
            let Some(ws) = list.iter().find(|w| w.storage_key == storage_key) else {
                return false;
            };
            let Some(idx) = ws.slot_ids.iter().position(|&id| id == slot_id) else {
                return false;
            };
            ws.slot_agent_labels
                .get(idx)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
        })
    }

    #[must_use]
    pub fn workspace_notification_counts(&self, workspace_id: u64) -> WorkspaceNotificationCounts {
        let Some(storage_key) = self.workspaces.with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.storage_key.clone())
        }) else {
            return WorkspaceNotificationCounts::default();
        };
        let prefix = format!("{storage_key}:");
        let mut total_unread = 0u32;
        for (key, &count) in self.notifications.get_untracked().iter() {
            if count == 0 || !key.starts_with(&prefix) {
                continue;
            }
            // Skip entries for slots that no longer exist or have no agent
            // attached — they would otherwise inflate the badge with
            // ghost counts from a previous session.
            if !self.notification_key_is_live(key) {
                continue;
            }
            total_unread = total_unread.saturating_add(count);
        }
        WorkspaceNotificationCounts { total_unread }
    }

    pub fn terminal_layout_tick(&self) -> RwSignal<u32> {
        self.terminal_layout_tick
    }

    pub fn bump_terminal_layout(&self) {
        self.terminal_layout_tick.update(|t| *t = t.wrapping_add(1));
    }

    /// Allocate the next visible workspace id. When every workspace has
    /// been closed, restart at `1` so default titles become "Workspace 1"
    /// again. Session/notification persistence stays safe because terminal
    /// keys use `WorkspaceEntry::storage_key`, not this display-oriented id.
    fn allocate_workspace_id(&self) -> u64 {
        if self.workspaces.with_untracked(|w| w.is_empty()) {
            self.workspace_next_id.set(2);
            return 1;
        }
        let max_open = self
            .workspaces
            .with_untracked(|w| w.iter().map(|x| x.id).max().unwrap_or(0));
        let id = self
            .workspace_next_id
            .get_untracked()
            .max(1)
            .max(max_open.saturating_add(1));
        self.workspace_next_id.set(id.saturating_add(1));
        id
    }

    /// Reset the visible workspace numbering after the last workspace is
    /// closed. Storage keys remain UUID-backed, so this no longer revives
    /// stale terminal sessions or unread notification entries.
    fn reset_workspace_id_counter_if_empty(&self) {
        if self.workspaces.with_untracked(|w| w.is_empty()) {
            self.workspace_next_id.set(1);
        }
    }

    /// Register a live PTY session for a terminal cell. `terminal_key` is
    /// the same `"{ws}:{slot}:{pane}"` shape used by the cell.
    pub fn register_pty_session(&self, terminal_key: String, session_id: u64) {
        self.pty_sessions.update(|m| {
            m.insert(terminal_key, session_id);
        });
    }

    pub fn unregister_pty_session(&self, terminal_key: &str) {
        self.pty_sessions.update(|m| {
            m.remove(terminal_key);
        });
    }

    /// Snapshot of all PTY sessions belonging to one workspace, keyed by
    /// `(slot_id, pane_id)`. The pane component is parsed from the key.
    #[must_use]
    pub fn pty_sessions_for_workspace(&self, workspace_id: u64) -> Vec<(u64, u64, u64)> {
        let Some(storage_key) = self.workspaces.with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.storage_key.clone())
        }) else {
            return Vec::new();
        };
        self.pty_sessions.with_untracked(|m| {
            m.iter()
                .filter_map(|(key, sid)| {
                    let mut it = key.split(':');
                    let ws = it.next()?;
                    let slot: u64 = it.next()?.parse().ok()?;
                    let pane: u64 = it.next()?.parse().ok()?;
                    if ws == storage_key {
                        Some((slot, pane, *sid))
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    #[must_use]
    pub fn workspaces(&self) -> RwSignal<Vec<WorkspaceEntry>> {
        self.workspaces
    }

    #[must_use]
    pub fn active_id(&self) -> RwSignal<Option<u64>> {
        self.active_id
    }

    #[must_use]
    pub fn recent_workspaces(&self) -> RwSignal<Vec<RecentWorkspaceItem>> {
        self.recent_workspaces
    }

    pub fn select_workspace(&self, id: u64) {
        self.active_id.set(Some(id));
        if let Some(cwd) = self.workspace_cwd_for(id) {
            spawn_ensure_agents_layout(cwd);
        }
    }

    fn workspace_cwd_for(&self, id: u64) -> Option<String> {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == id)
                .filter(|w| workspace_entry_has_folder(w))
                .map(|w| w.cwd.clone())
        })
    }

    #[must_use]
    pub fn default_workspace_cwd(&self) -> Option<String> {
        if let Some(active) = self.active_id.get_untracked() {
            let active_cwd = self.workspaces.with_untracked(|workspaces| {
                workspaces
                    .iter()
                    .find(|workspace| workspace.id == active)
                    .map(|workspace| workspace.cwd.trim().to_string())
            });
            if let Some(cwd) = active_cwd.filter(|cwd| !cwd.is_empty()) {
                return Some(cwd);
            }
        }
        let root = self.harness_workspace_root.get_untracked();
        let root = root.trim();
        (!root.is_empty()).then(|| root.to_string())
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

    /// Moves the workspace at `from_index` to `to_index` (indices in the
    /// list **before** the move). Order is persisted with the workbench snapshot.
    pub fn reorder_workspaces(&self, from_index: usize, to_index: usize) {
        if from_index == to_index {
            return;
        }
        self.workspaces.update(|ws| {
            let n = ws.len();
            if from_index >= n || to_index >= n {
                return;
            }
            let item = ws.remove(from_index);
            let insert_at = to_index.min(ws.len());
            ws.insert(insert_at, item);
        });
    }

    pub fn create_workspace(
        &self,
        title: Option<String>,
        cwd: Option<String>,
        terminal_count: u8,
        agent_slugs: Vec<String>,
    ) -> Result<u64, String> {
        let cwd = cwd
            .map(|cwd| cwd.trim().to_string())
            .filter(|cwd| !cwd.is_empty())
            .or_else(|| self.default_workspace_cwd())
            .ok_or_else(|| "no workspace cwd available".to_string())?;

        let terminal_count = terminal_count.clamp(1, 16);
        let slot_ids: Vec<u64> = (1..=terminal_count as u64).collect();
        let slot_pane_states: Vec<SlotPaneState> = slot_ids
            .iter()
            .copied()
            .map(SlotPaneState::default_for_slot)
            .collect();
        let slot_agent_labels =
            normalize_workspace_agent_labels(terminal_count as usize, &agent_slugs)?;
        let (grid_rows, grid_cols) = WorkspaceEntry::grid_dims_for_count(terminal_count);

        let id = self.allocate_workspace_id();

        let title = title
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| format!("Workspace {id}"));

        self.active_id.set(Some(id));
        self.workspaces.update(|workspaces| {
            workspaces.push(WorkspaceEntry {
                id,
                storage_key: WorkspaceEntry::new_storage_key(),
                title,
                cwd,
                terminal_count,
                grid_rows,
                grid_cols,
                next_terminal_id: terminal_count as u64 + 1,
                slot_ids,
                slot_agent_labels,
                slot_pane_states,
                configuring: false,
                agent_timeline: Vec::new(),
                agent_compose_draft: String::new(),
                agent_context_items: Vec::new(),
                memory_category_settings: HashMap::new(),
            });
        });
        Ok(id)
    }

    /// Opens a workspace from an absolute directory path (Quick Open).
    pub fn open_workspace_from_path_quick(&self, cwd: String) -> Result<u64, String> {
        let cwd = normalize_cwd_key(&cwd);
        if cwd.is_empty() {
            return Err("path empty".into());
        }
        let title = derive_workspace_name(&cwd).unwrap_or_else(|| "Workspace".into());
        self.create_workspace(Some(title), Some(cwd), 1, vec![])
    }

    fn push_recent_workspace_internal(
        &self,
        workspace: WorkspaceEntry,
        sessions_terminals_json: String,
    ) {
        if !workspace_entry_has_folder(&workspace) {
            return;
        }
        let key = normalize_cwd_key(&workspace.cwd);
        self.recent_workspaces.update(|list| {
            list.retain(|item| normalize_cwd_key(&item.workspace.cwd) != key);
            list.insert(
                0,
                RecentWorkspaceItem {
                    workspace,
                    sessions_terminals_json,
                },
            );
            const MAX: usize = 10;
            if list.len() > MAX {
                list.truncate(MAX);
            }
        });
    }

    fn finalize_workspace_close(&self, id: u64, entry: WorkspaceEntry, sessions_json: String) {
        self.push_recent_workspace_internal(entry, sessions_json);
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
        // Must run after `workspaces.update` — re-entering the same signal inside
        // the closure can deadlock Leptos and freeze close / add workspace.
        self.reset_workspace_id_counter_if_empty();
    }

    pub fn close_workspace(&self, id: u64) {
        let entry = self
            .workspaces
            .with_untracked(|w| w.iter().find(|x| x.id == id).cloned());
        let Some(entry) = entry else {
            return;
        };
        if !is_tauri_shell() {
            self.finalize_workspace_close(id, entry, "{}".into());
            return;
        }
        let storage_key = entry.storage_key.clone();
        let me = *self;
        spawn_local(async move {
            let blob = match workbench_extract_sessions_prefix(format!("{storage_key}:")).await {
                Ok(s) => s,
                Err(e) => {
                    leptos::logging::warn!("workbench_extract_sessions_prefix: {e}");
                    "{}".into()
                }
            };
            me.finalize_workspace_close(id, entry, blob);
        });
    }

    /// Restores a workspace from the recent list. The recent entry already
    /// carries its original `storage_key` (UUID) — we keep it so the
    /// extracted session entries (keyed by `{storage_key}:slot:pane`) are
    /// reinjected without any key rewriting. Only the in-memory `id: u64`
    /// is fresh.
    pub fn reopen_recent_workspace(&self, index: usize) {
        let item = self
            .recent_workspaces
            .with_untracked(|r| r.get(index).cloned());
        let Some(mut item) = item else {
            return;
        };
        self.recent_workspaces.update(|r| {
            if index < r.len() {
                r.remove(index);
            }
        });
        let new_id = self.allocate_workspace_id();
        item.workspace.id = new_id;
        // Backfill a storage_key on legacy recent entries that predate UUIDs.
        if item.workspace.storage_key.trim().is_empty() {
            item.workspace.storage_key = WorkspaceEntry::new_storage_key();
        }
        let storage_key = item.workspace.storage_key.clone();
        let sessions_json = item.sessions_terminals_json.clone();
        self.active_id.set(Some(new_id));
        self.workspaces.update(|v| v.push(item.workspace));
        if !is_tauri_shell() {
            return;
        }
        let trimmed = sessions_json.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            return;
        }
        spawn_local(async move {
            if let Err(e) =
                workbench_merge_sessions_workspace(storage_key.clone(), storage_key, sessions_json)
                    .await
            {
                leptos::logging::warn!("workbench_merge_sessions_workspace: {e}");
            }
        });
    }

    /// Drops one entry from the recent-workspaces ring buffer by list index.
    pub fn remove_recent_workspace(&self, index: usize) {
        self.recent_workspaces.update(|list| {
            if index < list.len() {
                list.remove(index);
            }
        });
    }

    /// Appends a new terminal slot to a workspace, optionally pre-labelled
    /// with a CLI-agent slug. Snaps `terminal_count` to the next supported
    /// preset (`[1,2,4,6,8,9,12,16]`) so `grid_dims_for_count` stays valid;
    /// the gap between the previous count and the new preset is filled
    /// with empty placeholder slots to keep the slot arrays aligned.
    /// Returns the new slot id, or an error string if the workspace is
    /// full / not found.
    #[allow(dead_code)]
    pub fn append_terminal_slot(
        &self,
        workspace_id: u64,
        agent_slug: Option<String>,
    ) -> Result<u64, String> {
        let ids = self.append_terminal_slots(workspace_id, vec![agent_slug.unwrap_or_default()])?;
        ids.into_iter()
            .next()
            .ok_or_else(|| "failed to append slot".into())
    }

    /// Append `slugs.len()` terminal slots in a single state update. Empty
    /// strings in `slugs` map to plain-shell slots; non-empty strings are
    /// stored as the slot's agent label. Returns the newly minted slot ids.
    ///
    /// Unlike workspace creation, this does NOT pad up to the next wizard
    /// preset — the grid heuristic handles odd counts (3, 5, 7, …) so the
    /// agent can add exactly the number of slots it asked for, without
    /// surprise empties appearing alongside.
    pub fn append_terminal_slots(
        &self,
        workspace_id: u64,
        slugs: Vec<String>,
    ) -> Result<Vec<u64>, String> {
        if slugs.is_empty() {
            return Err("no slots requested".into());
        }
        let mut new_ids: Vec<u64> = Vec::with_capacity(slugs.len());
        let mut err: Option<String> = None;
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                err = Some("workspace not found".into());
                return;
            };
            let remaining = 16usize.saturating_sub(workspace.slot_ids.len());
            if remaining == 0 {
                err = Some("workspace already at maximum slot count (16)".into());
                return;
            }
            if slugs.len() > remaining {
                err = Some(format!(
                    "requested {} slot(s) but only {} remain (max 16)",
                    slugs.len(),
                    remaining
                ));
                return;
            }
            for slug in &slugs {
                let mut new_slot_id = workspace.next_terminal_id.max(1);
                while workspace.slot_ids.iter().any(|id| *id == new_slot_id) {
                    new_slot_id += 1;
                }
                workspace.slot_ids.push(new_slot_id);
                workspace.slot_agent_labels.push(slug.clone());
                workspace
                    .slot_pane_states
                    .push(SlotPaneState::default_for_slot(new_slot_id));
                workspace.next_terminal_id = new_slot_id + 1;
                new_ids.push(new_slot_id);
            }
            let total = workspace.slot_ids.len() as u8;
            workspace.set_count_and_dims(total);
        });
        if let Some(e) = err {
            return Err(e);
        }
        Ok(new_ids)
    }

    pub fn close_terminal(&self, workspace_id: u64, terminal_id: u64) {
        let storage_key = self.workspaces.with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.storage_key.clone())
        });
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
        if let Some(storage_key) = storage_key {
            drop_sessions_for_prefix(format!("{storage_key}:{terminal_id}:"));
        }
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
    pub fn pending_memory_note(&self) -> RwSignal<Option<String>> {
        self.pending_memory_note
    }

    /// Focuses the memory panel and opens `path` (memory API path, after sanitise).
    pub fn request_open_memory_note(&self, path: String) {
        let t = path.trim().replace('\\', "/");
        let rel = crate::memory_paths::sanitize_memory_relative_path(&t).or_else(|| {
            let slug = crate::memory_paths::slug_to_filename(&t);
            crate::memory_paths::sanitize_memory_relative_path(&slug)
        });
        let Some(rel) = rel else {
            return;
        };
        self.pending_memory_note.set(Some(rel));
        self.set_right_tab(RightPanelTab::Memory);
        if self.right_collapsed.get_untracked() {
            self.toggle_right_panel();
        }
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
                push_history_entry(t, &normalized);
            }
        });
        self.browser_url.set(normalized);
    }

    /// Move back one entry in the active tab's history stack. Returns the
    /// URL that should now be loaded (caller must trigger iframe reload /
    /// native navigate). No-op + `None` if already at the head.
    pub fn tab_navigate_back(&self) -> Option<String> {
        let aid = self.embedded_browser_active_id.get_untracked();
        let mut next_url: Option<String> = None;
        self.embedded_browser_tabs.update(|tabs| {
            if let Some(t) = tabs.iter_mut().find(|t| t.id == aid) {
                if t.history_index > 0 && !t.history.is_empty() {
                    t.history_index -= 1;
                    if let Some(u) = t.history.get(t.history_index).cloned() {
                        t.url = u.clone();
                        next_url = Some(u);
                    }
                }
            }
        });
        if let Some(ref u) = next_url {
            self.browser_url.set(u.clone());
        }
        next_url
    }

    /// Move forward one entry in the active tab's history stack.
    pub fn tab_navigate_forward(&self) -> Option<String> {
        let aid = self.embedded_browser_active_id.get_untracked();
        let mut next_url: Option<String> = None;
        self.embedded_browser_tabs.update(|tabs| {
            if let Some(t) = tabs.iter_mut().find(|t| t.id == aid) {
                if !t.history.is_empty() && t.history_index + 1 < t.history.len() {
                    t.history_index += 1;
                    if let Some(u) = t.history.get(t.history_index).cloned() {
                        t.url = u.clone();
                        next_url = Some(u);
                    }
                }
            }
        });
        if let Some(ref u) = next_url {
            self.browser_url.set(u.clone());
        }
        next_url
    }

    /// True iff the active tab has a previous entry in its history stack.
    #[must_use]
    pub fn tab_can_go_back(&self) -> bool {
        let aid = self.embedded_browser_active_id.get();
        self.embedded_browser_tabs.with(|tabs| {
            tabs.iter()
                .find(|t| t.id == aid)
                .map(|t| t.history_index > 0)
                .unwrap_or(false)
        })
    }

    /// True iff the active tab has a forward entry in its history stack.
    #[must_use]
    pub fn tab_can_go_forward(&self) -> bool {
        let aid = self.embedded_browser_active_id.get();
        self.embedded_browser_tabs.with(|tabs| {
            tabs.iter()
                .find(|t| t.id == aid)
                .map(|t| !t.history.is_empty() && t.history_index + 1 < t.history.len())
                .unwrap_or(false)
        })
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
            history: Vec::new(),
            history_index: 0,
        };
        self.embedded_browser_tabs.update(|tabs| tabs.push(tab));
        self.embedded_browser_active_id.set(nid);
        self.browser_url.set(String::new());
        nid
    }

    /// Opens `http`/`https` in a **new** embedded-browser tab and focuses the browser panel.
    ///
    /// Host-only input (no `://`) is normalized with [`normalize_browser_url`]. Other schemes
    /// (`mailto:`, `ftp:`, …) are rejected. Returns `false` if nothing was opened.
    pub fn open_http_in_new_embedded_tab(&self, href: &str) -> bool {
        let t = href.trim();
        if t.is_empty() {
            return false;
        }
        let normalized = if t.starts_with("http://") || t.starts_with("https://") {
            t.to_string()
        } else if t.contains("://") {
            return false;
        } else {
            normalize_browser_url(t)
        };
        let n = normalized.trim();
        if !(n.starts_with("http://") || n.starts_with("https://")) {
            return false;
        }
        self.add_embedded_browser_tab();
        self.persist_browser_url_from_input(n.to_string());
        self.set_right_tab(RightPanelTab::Browser);
        if self.right_collapsed.get_untracked() {
            self.toggle_right_panel();
        }
        true
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
            if let Some(pick) = tabs.first().map(|t| t.id) {
                self.select_embedded_browser_tab(pick);
            }
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
        let id = self.allocate_workspace_id();

        let root = self.harness_workspace_root.get_untracked();
        let mut draft = CreateWorkspaceDraft::default();
        draft.cwd_display = root;

        let entry = WorkspaceEntry {
            id,
            storage_key: WorkspaceEntry::new_storage_key(),
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
            agent_timeline: Vec::new(),
            agent_compose_draft: String::new(),
            agent_context_items: Vec::new(),
            memory_category_settings: HashMap::new(),
        };
        self.active_id.set(Some(id));
        self.workspaces.update(|v| v.push(entry));
        self.workspace_drafts.update(|m| {
            m.insert(id, draft);
        });
        self.workspace_config_steps.update(|m| {
            m.insert(id, 0);
        });
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
        let cwd_for_agents = cwd.clone();
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
        self.bump_terminal_layout();
        spawn_ensure_agents_layout(cwd_for_agents);
        // Grid cells mount on the next frame; delayed ticks retry agent
        // launch once xterm has real dimensions (plain shells don't need this).
        let wb = *self;
        spawn_local(async move {
            for delay_ms in [50_u32, 150, 300, 600, 1000, 1500] {
                TimeoutFuture::new(delay_ms).await;
                wb.bump_terminal_layout();
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

    pub fn set_workspace_agent_timeline(&self, workspace_id: u64, items: Vec<TimelineItem>) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.agent_timeline = items;
            }
        });
    }

    #[must_use]
    pub fn agent_timeline_for_workspace_untracked(&self, workspace_id: u64) -> Vec<TimelineItem> {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.agent_timeline.clone())
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn agent_compose_draft_for_workspace_untracked(&self, workspace_id: u64) -> String {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.agent_compose_draft.clone())
                .unwrap_or_default()
        })
    }

    pub fn set_workspace_agent_compose_draft(&self, workspace_id: u64, draft: String) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.agent_compose_draft = draft;
            }
        });
    }

    #[must_use]
    pub fn agent_context_for_workspace_untracked(
        &self,
        workspace_id: u64,
    ) -> Vec<AgentContextItem> {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.agent_context_items.clone())
                .unwrap_or_default()
        })
    }

    pub fn upsert_workspace_agent_context(&self, workspace_id: u64, item: AgentContextItem) {
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(existing) = ws
                .agent_context_items
                .iter_mut()
                .find(|it| it.id == item.id)
            {
                *existing = item;
            } else {
                ws.agent_context_items.push(item);
            }
        });
    }

    pub fn remove_workspace_agent_context(&self, workspace_id: u64, item_id: &str) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.agent_context_items.retain(|item| item.id != item_id);
            }
        });
    }

    #[must_use]
    pub fn memory_color_presets(&self) -> RwSignal<Vec<MemoryColorPreset>> {
        self.memory_color_presets
    }

    pub fn set_memory_color_presets(&self, presets: Vec<MemoryColorPreset>) {
        let presets = if presets.is_empty() {
            default_memory_color_presets()
        } else {
            presets
        };
        write_memory_color_presets(&presets);
        self.memory_color_presets.set(presets);
    }

    pub fn reset_memory_color_presets(&self) {
        self.set_memory_color_presets(default_memory_color_presets());
    }

    #[must_use]
    pub fn memory_category_settings_for_workspace_untracked(
        &self,
        workspace_id: u64,
        category: &str,
    ) -> MemoryCategorySettings {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.memory_category_settings.get(category).cloned())
                .unwrap_or_else(|| MemoryCategorySettings::for_category(category))
        })
    }

    #[must_use]
    pub fn memory_category_settings_for_workspace(
        &self,
        workspace_id: u64,
        category: &str,
    ) -> MemoryCategorySettings {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.memory_category_settings.get(category).cloned())
                .unwrap_or_else(|| MemoryCategorySettings::for_category(category))
        })
    }

    pub fn set_memory_category_settings(
        &self,
        workspace_id: u64,
        category: &str,
        settings: MemoryCategorySettings,
    ) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.memory_category_settings
                    .insert(category.to_string(), settings);
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
        let workspaces: Vec<WorkspaceEntry> = self
            .workspaces
            .get_untracked()
            .into_iter()
            .filter(|w| workspace_entry_has_folder(w))
            .collect();
        let active_id = self
            .active_id
            .get_untracked()
            .filter(|id| workspaces.iter().any(|w| w.id == *id))
            .or_else(|| workspaces.first().map(|w| w.id));
        let recent_workspaces: Vec<RecentWorkspaceItem> = self
            .recent_workspaces
            .get_untracked()
            .into_iter()
            .filter(|r| workspace_entry_has_folder(&r.workspace))
            .collect();
        WorkbenchSnapshot {
            version: WORKBENCH_SNAPSHOT_VERSION,
            workspaces,
            active_id,
            workspace_next_id: self.workspace_next_id.get_untracked(),
            sidebar_collapsed: self.sidebar_collapsed.get_untracked(),
            right_collapsed: self.right_collapsed.get_untracked(),
            right_width_px: self.right_width_px.get_untracked(),
            right_tab: self.right_tab.get_untracked(),
            recent_workspaces,
            // Browser tabs are session-scoped — never persisted.
            embedded_browser_tabs: Vec::new(),
            embedded_browser_active_id: 0,
            embedded_browser_next_id: 1,
        }
    }

    /// Apply a previously persisted snapshot. Mismatched / future versions
    /// are rejected silently so a stale file never breaks startup.
    ///
    /// Returns `true` when the snapshot version matched and state was
    /// applied. Callers must not enable disk persistence when this is false.
    pub fn hydrate(&self, snap: WorkbenchSnapshot) -> bool {
        if snap.version != WORKBENCH_SNAPSHOT_VERSION {
            return false;
        }
        let max_snap_workspace_id = snap.workspaces.iter().map(|w| w.id).max().unwrap_or(0);
        let workspaces: Vec<WorkspaceEntry> = snap
            .workspaces
            .into_iter()
            .filter(|w| workspace_entry_has_folder(w))
            .map(|mut w| {
                // Safety net for callers that did not run
                // `WorkbenchSnapshot::backfill_storage_keys` before hydrate.
                // The normal Tauri startup path backfills first so matching
                // sessions can be migrated before terminal cells mount.
                if w.storage_key.trim().is_empty() {
                    w.storage_key = WorkspaceEntry::new_storage_key();
                }
                w
            })
            .collect();
        let recent_workspaces: Vec<RecentWorkspaceItem> = snap
            .recent_workspaces
            .into_iter()
            .filter(|r| workspace_entry_has_folder(&r.workspace))
            .map(|mut r| {
                if r.workspace.storage_key.trim().is_empty() {
                    r.workspace.storage_key = WorkspaceEntry::new_storage_key();
                }
                r
            })
            .collect();
        // Workspace list + selection. Re-seed inline drafts for any
        // workspace persisted in configuring state — drafts are transient
        // and not serialised, so without this the configurator would
        // render against an empty draft.
        let root = self.harness_workspace_root.get_untracked();
        self.workspace_drafts.update(|m| {
            m.clear();
            for ws in &workspaces {
                if ws.configuring {
                    let mut d = CreateWorkspaceDraft::default();
                    if !root.is_empty() {
                        d.cwd_display.clone_from(&root);
                    }
                    m.insert(ws.id, d);
                }
            }
        });
        let next_id = snap
            .workspace_next_id
            .max(1)
            .max(max_snap_workspace_id.saturating_add(1));
        let active_id = snap
            .active_id
            .filter(|id| workspaces.iter().any(|w| w.id == *id))
            .or_else(|| workspaces.first().map(|w| w.id));
        let has_workspaces = !workspaces.is_empty();
        self.active_id.set(active_id);
        self.workspaces.set(workspaces);
        // Preserve the persisted counter even when the workspace list is
        // empty: a fresh workspace must NOT get id=1 if we have ever
        // allocated higher ids in this app installation. Otherwise it
        // would reuse the storage references of a previously-closed
        // workspace. (The deeper guarantee is `storage_key`, but keeping
        // the `id` monotonic too removes a footgun from any code that
        // still derives state from `id`.)
        self.workspace_next_id.set(next_id);
        self.recent_workspaces.set(recent_workspaces);

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

        if has_workspaces {
            self.bump_terminal_layout();
            if let Some(id) = active_id {
                if let Some(cwd) = self.workspace_cwd_for(id) {
                    spawn_ensure_agents_layout(cwd);
                }
            }
            let wb = *self;
            spawn_local(async move {
                for delay_ms in [0_u32, 16, 50, 150, 300, 600, 1000] {
                    TimeoutFuture::new(delay_ms).await;
                    wb.bump_terminal_layout();
                }
            });
        }
        true
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
    #[serde(default)]
    pub recent_workspaces: Vec<RecentWorkspaceItem>,
    pub embedded_browser_tabs: Vec<EmbeddedBrowserTab>,
    pub embedded_browser_active_id: u64,
    pub embedded_browser_next_id: u64,
}

impl WorkbenchSnapshot {
    /// Backfill pre-UUID snapshots before hydration so terminal cells mount
    /// with keys that already have their `sessions.json` entries migrated.
    pub fn backfill_storage_keys(&mut self) -> Vec<LegacyStorageMigration> {
        let mut migrations = Vec::new();
        for workspace in &mut self.workspaces {
            if workspace.storage_key.trim().is_empty() {
                let old_workspace_key = workspace.id.to_string();
                workspace.storage_key = WorkspaceEntry::new_storage_key();
                migrations.push(LegacyStorageMigration {
                    old_workspace_key,
                    new_workspace_key: workspace.storage_key.clone(),
                });
            }
        }
        for item in &mut self.recent_workspaces {
            if item.workspace.storage_key.trim().is_empty() {
                let old_workspace_key = item.workspace.id.to_string();
                item.workspace.storage_key = WorkspaceEntry::new_storage_key();
                item.sessions_terminals_json = rewrite_sessions_terminals_json(
                    &item.sessions_terminals_json,
                    &old_workspace_key,
                    &item.workspace.storage_key,
                );
            }
        }
        migrations
    }
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

/// Smallest preset in `[1,2,4,6,8,9,12,16]` strictly greater than `current`.
/// `grid_dims_for_count` is hardcoded for these counts, so any other value
/// would land in the fallback heuristic — we prefer to keep parity with
/// the wizard presets.
#[allow(dead_code)]
fn next_preset_above(current: u8) -> u8 {
    const PRESETS: [u8; 8] = [1, 2, 4, 6, 8, 9, 12, 16];
    for &p in &PRESETS {
        if p > current {
            return p;
        }
    }
    16
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
