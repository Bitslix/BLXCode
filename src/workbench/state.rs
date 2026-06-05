use crate::agent_wire::{AgentChatMode, AgentContextItem, AgentImageContextItem};
use crate::config::{
    DEFAULT_PROJECT_DIR_KEY, HARNESS_BROWSER_DEFAULT_URL, HARNESS_BROWSER_URL_KEY,
    HARNESS_WORKSPACE_ROOT_KEY, MEMORY_COLOR_PRESETS_STORAGE_KEY, SIDEBAR_WIDTH_PX_DEFAULT,
    SIDEBAR_WIDTH_PX_KEY,
};
use crate::tauri_bridge::{
    agent_environment_invalidate, is_tauri_shell, workbench_drop_sessions,
    workbench_extract_sessions_prefix, workbench_merge_sessions_workspace, AgentNotification,
    TimelineDiagram,
};
use crate::workbench::agent_timeline::TimelineDoc;
use crate::workbench::terminal_agent_profiles::{
    is_supported_terminal_agent_slug, supported_terminal_agent_slugs,
};
use crate::workbench::terminal_slot_dnd::TerminalSlotDropAction;
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

pub const DEFAULT_AGENT_CHAT_SESSION_ID: &str = "default";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentChatSessionStatus {
    #[default]
    Idle,
    Running,
    NeedsInput,
    Error,
    Restored,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatSession {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub timeline: TimelineDoc,
    #[serde(default)]
    pub draft: String,
    #[serde(default)]
    pub image_mode: bool,
    #[serde(default)]
    pub chat_mode: AgentChatMode,
    #[serde(default)]
    pub enhance_prompt_before_send: bool,
    #[serde(default)]
    pub usage: ChatUsageStats,
    #[serde(default)]
    pub status: AgentChatSessionStatus,
    #[serde(default)]
    pub unread_count: u32,
    #[serde(default)]
    pub pending_context_items: Vec<AgentContextItem>,
    #[serde(default)]
    pub pending_image_context_items: Vec<AgentImageContextItem>,
    #[serde(default)]
    pub created_at: f64,
    #[serde(default)]
    pub updated_at: f64,
}

impl AgentChatSession {
    #[must_use]
    pub fn legacy_default(
        timeline: TimelineDoc,
        draft: String,
        image_mode: bool,
        chat_mode: AgentChatMode,
        enhance_prompt_before_send: bool,
        usage: ChatUsageStats,
        context_items: Vec<AgentContextItem>,
    ) -> Self {
        Self {
            id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
            title: "Chat 1".to_string(),
            timeline,
            draft,
            image_mode,
            chat_mode,
            enhance_prompt_before_send,
            usage,
            status: AgentChatSessionStatus::Idle,
            unread_count: 0,
            pending_context_items: context_items,
            pending_image_context_items: Vec::new(),
            created_at: 0.0,
            updated_at: 0.0,
        }
    }

    #[must_use]
    pub fn fresh(index: usize, now: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            title: format!("Chat {}", index.max(1)),
            timeline: TimelineDoc::default(),
            draft: String::new(),
            image_mode: false,
            chat_mode: AgentChatMode::AskEdits,
            enhance_prompt_before_send: false,
            usage: ChatUsageStats::default(),
            status: AgentChatSessionStatus::Idle,
            unread_count: 0,
            pending_context_items: Vec::new(),
            pending_image_context_items: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// One workspace open in the sidebar; shared across center and right panel via [`WorkbenchService`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Sidebar accent color. Empty values come from older snapshots and are
    /// backfilled from the Workspace settings category-color presets.
    #[serde(default)]
    pub color: String,
    pub cwd: String,
    pub terminal_count: u8,
    pub grid_rows: u8,
    pub grid_cols: u8,
    pub next_terminal_id: u64,
    pub slot_ids: Vec<u64>,
    /// One label/slug per terminal slot (e.g. `"claude"` or empty after skip).
    pub slot_agent_labels: Vec<String>,
    /// Optional CLI model id per terminal slot, parallel to `slot_agent_labels`.
    /// Empty entries use the agent's own default model. Chosen in the
    /// Create-Workspace fleet step from the per-CLI built-in model catalog.
    #[serde(default)]
    pub slot_agent_models: Vec<String>,
    /// Optional reasoning effort per terminal slot, parallel to
    /// `slot_agent_labels` / `slot_agent_models`. Empty entries use the CLI's
    /// own default effort.
    #[serde(default)]
    pub slot_agent_efforts: Vec<String>,
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
    pub agent_timeline: TimelineDoc,
    /// Draft text in the agent compose field (same workspace binding).
    #[serde(default)]
    pub agent_compose_draft: String,
    /// Image-generation toggle for the agent chat (per workspace).
    #[serde(default)]
    pub agent_image_mode: bool,
    /// Execution/approval mode for the current Agent Chat session.
    #[serde(default)]
    pub agent_chat_mode: AgentChatMode,
    /// When true, the composer runs a separate one-shot prompt enhancement
    /// before submitting the user turn. Per workspace/session, default off.
    #[serde(default)]
    pub agent_enhance_prompt_before_send: bool,
    /// Default-off setting for future optional LLM prose synthesis into the
    /// architecture map. Current rebuilds remain deterministic and non-LLM.
    #[serde(default)]
    pub architecture_llm_prose: bool,
    /// Memory/Learnings context attached to the next BLXCode Agent turns.
    #[serde(default)]
    pub agent_context_items: Vec<AgentContextItem>,
    /// Parallel BLXCode Agent chat sessions for this workspace. Legacy
    /// single-chat fields above are migrated into the first session and then
    /// kept as compatibility mirrors of the active session.
    #[serde(default)]
    pub agent_chat_sessions: Vec<AgentChatSession>,
    #[serde(default)]
    pub active_agent_chat_session_id: String,
    /// Display/color/visibility overrides for memory categories in this workspace.
    #[serde(default)]
    pub memory_category_settings: HashMap<String, MemoryCategorySettings>,
    /// Aggregated token / latency stats for this workspace's agent chat.
    /// Accumulates across turns; reset on chat clear.
    #[serde(default)]
    pub agent_chat_usage: ChatUsageStats,
    /// Sidebar explorer section expanded (bottom panel).
    #[serde(default = "default_sidebar_section_open")]
    pub sidebar_explorer_open: bool,
    /// Sidebar git graph section expanded. Defaults to **closed** so the
    /// section starts collapsed and only opens when the user explicitly
    /// expands it; the per-workspace state then persists across sessions.
    #[serde(default = "default_sidebar_graph_open")]
    pub sidebar_graph_open: bool,
    /// Sidebar `File Diff` section expanded. Defaults open so changes are
    /// visible the moment the workspace mounts. Persisted across sessions.
    #[serde(default = "default_sidebar_section_open")]
    pub sidebar_diff_open: bool,
    /// Relative paths (from `cwd`) expanded in the project explorer tree.
    #[serde(default)]
    pub sidebar_explorer_expanded_paths: Vec<String>,
    /// Center editor tabs for this workspace. The terminal grid is a pinned
    /// default tab; other tabs are opened dynamically from the UI.
    #[serde(default = "default_center_tabs")]
    pub center_tabs: Vec<CenterTab>,
    /// Currently selected center tab.
    #[serde(default = "default_center_active_tab_id")]
    pub center_active_tab_id: u64,
    /// Next id for dynamically-created center tabs.
    #[serde(default = "default_center_next_tab_id")]
    pub center_next_tab_id: u64,
    /// `Some(connection_id)` when this is an SSH remote workspace — its
    /// terminals spawn `ssh` to the saved [`RemoteConnection`] preset instead
    /// of a local shell. `None` (default, back-compat) means a local workspace.
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    /// Git worktree metadata when this workspace was opened from, or created
    /// as, a Git worktree. `None` means a normal local/remote workspace.
    #[serde(default)]
    pub worktree: Option<WorkspaceWorktreeMeta>,
    /// Per-slot friendly-name overrides, keyed by `slot_id`. Empty by
    /// default; an entry takes precedence over the deterministic name pool
    /// when the terminal naming mode is `names`. Keyed by `slot_id` (not a
    /// parallel vector) so it survives slot insertion/removal without
    /// index maintenance.
    #[serde(default)]
    pub slot_name_overrides: HashMap<u64, String>,
    /// Active BLXCode harness session-role slug (specialized skill) for this
    /// workspace, e.g. `"coordinator"`. `None` = default agent. Persisted with
    /// the workspace snapshot so the role is restored on reload and handed to
    /// the agent each turn via `UserTurn.session_role`.
    #[serde(default)]
    pub agent_session_role: Option<String>,
    /// Active workspace view mode. Legacy snapshots default to Grid.
    #[serde(default)]
    pub view_mode: WorkspaceViewMode,
    /// Canvas viewport, terminal positions, and display filters.
    #[serde(default)]
    pub canvas_view_state: CanvasViewState,
    /// User-created Canvas routing edges.
    #[serde(default)]
    pub canvas_edges: Vec<CanvasEdge>,
    /// Default transfer behavior for newly created Canvas edges.
    #[serde(default)]
    pub canvas_default_transfer_mode: CanvasTransferMode,
    /// Swarm graph display state.
    #[serde(default)]
    pub swarm_view_state: SwarmViewState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWorktreeMeta {
    pub base_cwd: String,
    pub worktree_cwd: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub git_common_dir: Option<String>,
    pub main_worktree_cwd: Option<String>,
    pub created_by_blxcode: bool,
}

fn default_sidebar_section_open() -> bool {
    true
}

fn default_sidebar_graph_open() -> bool {
    false
}

pub const CENTER_KANBAN_TAB_ID: u64 = 0;
pub const CENTER_TERMINALS_TAB_ID: u64 = 1;

fn default_center_active_tab_id() -> u64 {
    CENTER_TERMINALS_TAB_ID
}

fn default_center_next_tab_id() -> u64 {
    CENTER_TERMINALS_TAB_ID + 1
}

fn default_center_tabs() -> Vec<CenterTab> {
    vec![CenterTab::kanban(), CenterTab::terminals()]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CenterTab {
    pub id: u64,
    pub title: String,
    pub kind: CenterTabKind,
}

impl CenterTab {
    #[must_use]
    pub fn kanban() -> Self {
        Self {
            id: CENTER_KANBAN_TAB_ID,
            title: "Kanban".into(),
            kind: CenterTabKind::Kanban,
        }
    }

    #[must_use]
    pub fn terminals() -> Self {
        Self {
            id: CENTER_TERMINALS_TAB_ID,
            title: "Terminals".into(),
            kind: CenterTabKind::Terminals,
        }
    }

    #[must_use]
    pub fn mode(mode: WorkspaceViewMode) -> Self {
        Self {
            id: CENTER_TERMINALS_TAB_ID,
            title: mode.title().into(),
            kind: mode.tab_kind(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CenterTabKind {
    Kanban,
    Terminals,
    Canvas,
    Swarm,
    Settings,
    Memory,
    FilePreview {
        rel_path: String,
    },
    /// Side-by-side or inline diff view for one changed file.
    /// `staged` selects between `git diff [--cached]`.
    FileDiff {
        rel_path: String,
        staged: bool,
    },
    /// Centered Mermaid diagram gallery for a plan's diagram set.
    DiagramGallery {
        /// Plan slug whose `diagrams/` folder is shown.
        slug: String,
    },
    /// Centered gallery for an ephemeral (non-persisted) diagram group opened
    /// from the agent timeline. The diagrams are embedded directly since they
    /// live only in the chat, not in any plan's `diagrams/` folder.
    DiagramGroup {
        title: String,
        diagrams: Vec<TimelineDiagram>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceViewMode {
    #[default]
    Grid,
    Canvas,
    Swarm,
}

impl WorkspaceViewMode {
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Grid => "Terminals",
            Self::Canvas => "Canvas",
            Self::Swarm => "Swarm",
        }
    }

    #[must_use]
    pub fn tab_kind(self) -> CenterTabKind {
        match self {
            Self::Grid => CenterTabKind::Terminals,
            Self::Canvas => CenterTabKind::Canvas,
            Self::Swarm => CenterTabKind::Swarm,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CanvasTransferMode {
    Raw,
    #[default]
    Structured,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasNodeLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CanvasNodeLayout {
    #[must_use]
    pub fn for_index(index: usize) -> Self {
        let col = index % 2;
        let row = index / 2;
        Self {
            x: 48.0 + col as f64 * 460.0,
            y: 52.0 + row as f64 * 330.0,
            width: 420.0,
            height: 260.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasViewState {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
    #[serde(default)]
    pub terminal_nodes: HashMap<u64, CanvasNodeLayout>,
    #[serde(default)]
    pub selected_node_ids: Vec<String>,
    #[serde(default)]
    pub selected_edge_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub show_agent_links: bool,
}

impl Default for CanvasViewState {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            terminal_nodes: HashMap::new(),
            selected_node_ids: Vec::new(),
            selected_edge_ids: Vec::new(),
            show_agent_links: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPortRef {
    pub node_kind: CanvasNodeKind,
    #[serde(default)]
    pub slot_id: Option<u64>,
    #[serde(default)]
    pub pane_id: Option<u64>,
    pub direction: CanvasPortDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CanvasNodeKind {
    Terminal,
    AgentHub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CanvasPortDirection {
    Stdin,
    Stdout,
    AgentCommand,
    AgentObserve,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasEdge {
    pub id: String,
    pub source: CanvasPortRef,
    pub target: CanvasPortRef,
    #[serde(default)]
    pub transfer_mode: CanvasTransferMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmViewState {
    #[serde(default)]
    pub selected_node_id: Option<String>,
    #[serde(default)]
    pub node_positions: HashMap<String, SwarmNodeLayout>,
    #[serde(default = "default_true")]
    pub show_agent_links: bool,
    #[serde(default = "default_true")]
    pub show_idle: bool,
}

impl Default for SwarmViewState {
    fn default() -> Self {
        Self {
            selected_node_id: None,
            node_positions: HashMap::new(),
            show_agent_links: true,
            show_idle: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmNodeLayout {
    pub x: f64,
    pub y: f64,
}

fn default_true() -> bool {
    true
}

/// Aggregated token / cost stats for a workspace's agent chat. Each
/// `AgentEvent::TurnUsage` (ModelRound or ToolExec) updates these running
/// totals. Per-row metrics live on the timeline items themselves; this
/// struct only carries what the chat header needs.
///
/// Legacy fields `ttft_sum_ms` / `ttft_sample_count` were removed when
/// TTFT moved to per-row metrics. Older `workbench.json` snapshots ignore
/// the removed fields on deserialize (Serde default behaviour) and rewrite
/// without them on the next save.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUsageStats {
    /// Sum of input/prompt tokens across all rounds and subagents.
    #[serde(default)]
    pub total_input_tokens: u64,
    /// Sum of output/completion tokens across all rounds and subagents.
    #[serde(default)]
    pub total_output_tokens: u64,
    /// Sum of wall-clock ms across all rounds and tool executions.
    #[serde(default)]
    pub total_elapsed_ms: u64,
    /// Total resolved USD cost across all rounds and tool executions.
    /// Missing pricing data is treated as zero contribution (no flag).
    #[serde(default)]
    pub total_cost_usd: f64,
    /// Total number of accounted events (`ModelRound` + `ToolExec` across
    /// main agent and subagents). Rendered as `N turns` in the chat header.
    #[serde(default)]
    pub turn_count: u32,
    /// Input tokens reported by the **most recent main-agent `ModelRound`**.
    /// Unlike `total_input_tokens` (a cumulative sum used for cost), this is
    /// the live context-window occupancy: each provider round re-sends the
    /// whole conversation, so the latest round's prompt size is "tokens
    /// currently in the window". Drives the chat-header context meter; reset
    /// on clear and overwritten after a compaction. Subagent rounds are
    /// excluded (they don't sit in the main conversation window).
    #[serde(default)]
    pub last_round_input_tokens: u64,
    /// Epoch-ms timestamp of the **first user turn** in the current session.
    /// Mirrored from the timeline on submit; cleared by `clear_chat_usage`
    /// (alongside `agent_clear_conversation`). Drives the "Session start" row
    /// in the agent stats panel. Persisted so it survives reloads.
    #[serde(default)]
    pub session_started_at: Option<f64>,
    /// Highest `turn_generation` observed in a `TurnUsage` event for this
    /// workspace. Events stamped with a lower generation are dropped — they
    /// belong to a turn that was cancelled by `agent_clear_conversation`.
    /// Bumped locally on chat reset as well so we don't credit anything
    /// emitted before the reset to the fresh chat.
    ///
    /// Not persisted: the backend resets its own generation to 0 on every
    /// app launch, so persisting this value would cause all first-session
    /// events to be dropped after any prior-session reset.
    #[serde(skip)]
    pub current_turn_generation: u64,
}

fn default_sidebar_width_px() -> f64 {
    SIDEBAR_WIDTH_PX_DEFAULT
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
            "memory" => Self {
                label: "Memory".into(),
                color: "#7dd3fc".into(),
                show_in_sidebar: true,
                show_in_graph: true,
            },
            "learnings" => Self {
                label: "Learnings".into(),
                color: "#67e8f9".into(),
                show_in_sidebar: true,
                show_in_graph: true,
            },
            other => Self {
                label: other.to_string(),
                color: stable_category_color(other),
                show_in_sidebar: true,
                show_in_graph: true,
            },
        }
    }
}

/// Deterministic `#rrggbb` color for an arbitrary category name — same hash
/// used for graph cluster colors so the sidebar accent matches the node fill.
#[must_use]
pub fn stable_category_color(key: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in key.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    let hue = (h % 360) as f32;
    hsl_to_hex(hue, 70.0, 64.0)
}

#[must_use]
pub fn normalize_hex_color(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
    {
        trimmed.to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

fn hsl_to_hex(h: f32, s_pct: f32, l_pct: f32) -> String {
    let s = s_pct / 100.0;
    let l = l_pct / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to_byte = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", to_byte(r1), to_byte(g1), to_byte(b1))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryColorPreset {
    pub id: String,
    pub label: String,
    pub color: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentImageContextStatus {
    Pending,
    Read,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceAgentImage {
    pub item: AgentImageContextItem,
    pub status: AgentImageContextStatus,
}

impl WorkspaceAgentImage {
    #[must_use]
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.item.mime, self.item.bytes_b64)
    }
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

#[must_use]
pub fn workspace_color_from_presets(presets: &[MemoryColorPreset], index: usize) -> String {
    let valid: Vec<String> = presets
        .iter()
        .map(|preset| normalize_hex_color(&preset.color, ""))
        .filter(|color| !color.is_empty())
        .collect();
    if valid.is_empty() {
        let defaults = default_memory_color_presets();
        return defaults
            .get(index % defaults.len())
            .map(|preset| normalize_hex_color(&preset.color, "#7dd3fc"))
            .unwrap_or_else(|| "#7dd3fc".into());
    }
    valid[index % valid.len()].clone()
}

/// Per-slot terminal split state — survives a restart so the grid of
/// panes inside each slot is restored exactly as the user left it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotPaneAgentState {
    #[serde(default)]
    pub agent_label: String,
    #[serde(default)]
    pub agent_model: String,
    #[serde(default)]
    pub agent_effort: String,
}

impl SlotPaneAgentState {
    #[must_use]
    pub fn with_fallback(&self, fallback: &Self) -> Self {
        Self {
            agent_label: field_or_fallback(&self.agent_label, &fallback.agent_label),
            agent_model: field_or_fallback(&self.agent_model, &fallback.agent_model),
            agent_effort: field_or_fallback(&self.agent_effort, &fallback.agent_effort),
        }
    }
}

fn field_or_fallback(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn terminal_key_slot_pane(key: &str) -> Option<(u64, u64)> {
    let mut parts = key.split(':');
    let _storage = parts.next()?;
    let slot_id = parts.next()?.parse::<u64>().ok()?;
    let pane_id = parts.next()?.parse::<u64>().ok()?;
    Some((slot_id, pane_id))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotPaneState {
    pub axis: TerminalSplitAxis,
    pub pane_ids: Vec<u64>,
    pub next_pane_id: u64,
    #[serde(default)]
    pub pane_agents: Vec<SlotPaneAgentState>,
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
            pane_agents: Vec::new(),
        }
    }

    pub fn normalize_pane_agents(&mut self, fallback: &SlotPaneAgentState) {
        self.pane_agents.truncate(self.pane_ids.len());
        while self.pane_agents.len() < self.pane_ids.len() {
            self.pane_agents.push(fallback.clone());
        }
        for agent in &mut self.pane_agents {
            if !agent.agent_label.trim().is_empty() {
                *agent = agent.with_fallback(fallback);
            }
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

    fn slot_agent_state_at(&self, slot_idx: usize) -> SlotPaneAgentState {
        SlotPaneAgentState {
            agent_label: self
                .slot_agent_labels
                .get(slot_idx)
                .cloned()
                .unwrap_or_default(),
            agent_model: self
                .slot_agent_models
                .get(slot_idx)
                .cloned()
                .unwrap_or_default(),
            agent_effort: self
                .slot_agent_efforts
                .get(slot_idx)
                .cloned()
                .unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn pane_agent_state(&self, slot_id: u64, pane_id: u64) -> Option<SlotPaneAgentState> {
        let slot_idx = self.slot_ids.iter().position(|id| *id == slot_id)?;
        let fallback = self.slot_agent_state_at(slot_idx);
        let mut pane_state = self
            .slot_pane_states
            .get(slot_idx)
            .cloned()
            .unwrap_or_else(|| SlotPaneState::default_for_slot(slot_id));
        pane_state.normalize_pane_agents(&fallback);
        let pane_idx = pane_state.pane_ids.iter().position(|id| *id == pane_id)?;
        Some(
            pane_state
                .pane_agents
                .get(pane_idx)
                .map(|agent| {
                    if agent.agent_label.trim().is_empty() {
                        agent.clone()
                    } else {
                        agent.with_fallback(&fallback)
                    }
                })
                .unwrap_or(fallback),
        )
    }

    #[must_use]
    pub fn empty_surface(id: u64) -> Self {
        Self {
            id,
            storage_key: Self::new_storage_key(),
            title: String::new(),
            color: "#7dd3fc".into(),
            cwd: String::new(),
            terminal_count: 1,
            grid_rows: 1,
            grid_cols: 1,
            next_terminal_id: 1,
            slot_ids: Vec::new(),
            slot_agent_labels: Vec::new(),
            slot_agent_models: Vec::new(),
            slot_agent_efforts: Vec::new(),
            slot_pane_states: Vec::new(),
            configuring: false,
            agent_timeline: TimelineDoc::default(),
            agent_compose_draft: String::new(),
            agent_image_mode: false,
            agent_chat_mode: AgentChatMode::AskEdits,
            agent_enhance_prompt_before_send: false,
            architecture_llm_prose: false,
            agent_context_items: Vec::new(),
            agent_chat_sessions: Vec::new(),
            active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
            memory_category_settings: HashMap::new(),
            agent_chat_usage: ChatUsageStats::default(),
            sidebar_explorer_open: true,
            sidebar_graph_open: false,
            sidebar_diff_open: true,
            sidebar_explorer_expanded_paths: Vec::new(),
            center_tabs: default_center_tabs(),
            center_active_tab_id: default_center_active_tab_id(),
            center_next_tab_id: default_center_next_tab_id(),
            remote_connection_id: None,
            worktree: None,
            slot_name_overrides: HashMap::new(),
            agent_session_role: None,
            view_mode: WorkspaceViewMode::Grid,
            canvas_view_state: CanvasViewState::default(),
            canvas_edges: Vec::new(),
            canvas_default_transfer_mode: CanvasTransferMode::Structured,
            swarm_view_state: SwarmViewState::default(),
        }
    }

    pub fn ensure_agent_chat_sessions(&mut self) {
        if self.agent_chat_sessions.is_empty() {
            self.agent_chat_sessions
                .push(AgentChatSession::legacy_default(
                    self.agent_timeline.clone(),
                    self.agent_compose_draft.clone(),
                    self.agent_image_mode,
                    self.agent_chat_mode,
                    self.agent_enhance_prompt_before_send,
                    self.agent_chat_usage.clone(),
                    self.agent_context_items.clone(),
                ));
        }

        for (idx, session) in self.agent_chat_sessions.iter_mut().enumerate() {
            if session.id.trim().is_empty() {
                session.id = if idx == 0 {
                    DEFAULT_AGENT_CHAT_SESSION_ID.to_string()
                } else {
                    uuid::Uuid::new_v4().simple().to_string()
                };
            }
            if session.title.trim().is_empty() {
                session.title = format!("Chat {}", idx + 1);
            }
            if matches!(
                session.status,
                AgentChatSessionStatus::Running | AgentChatSessionStatus::NeedsInput
            ) {
                session.status = AgentChatSessionStatus::Restored;
            }
        }

        if self.active_agent_chat_session_id.trim().is_empty()
            || !self
                .agent_chat_sessions
                .iter()
                .any(|session| session.id == self.active_agent_chat_session_id)
        {
            self.active_agent_chat_session_id = self
                .agent_chat_sessions
                .first()
                .map(|session| session.id.clone())
                .unwrap_or_else(|| DEFAULT_AGENT_CHAT_SESSION_ID.to_string());
        }
        self.sync_legacy_agent_chat_fields_from_active();
    }

    fn active_agent_chat_session(&self) -> Option<&AgentChatSession> {
        self.agent_chat_sessions
            .iter()
            .find(|session| session.id == self.active_agent_chat_session_id)
            .or_else(|| self.agent_chat_sessions.first())
    }

    fn active_agent_chat_session_mut(&mut self) -> Option<&mut AgentChatSession> {
        let active = self.active_agent_chat_session_id.clone();
        let idx = self
            .agent_chat_sessions
            .iter()
            .position(|session| session.id == active)
            .unwrap_or(0);
        self.agent_chat_sessions.get_mut(idx)
    }

    fn agent_chat_session(&self, session_id: &str) -> Option<&AgentChatSession> {
        self.agent_chat_sessions
            .iter()
            .find(|session| session.id == session_id)
    }

    fn agent_chat_session_mut(&mut self, session_id: &str) -> Option<&mut AgentChatSession> {
        self.agent_chat_sessions
            .iter_mut()
            .find(|session| session.id == session_id)
    }

    fn sync_legacy_agent_chat_fields_from_active(&mut self) {
        if let Some(session) = self.active_agent_chat_session().cloned() {
            self.agent_timeline = session.timeline;
            self.agent_compose_draft = session.draft;
            self.agent_image_mode = session.image_mode;
            self.agent_chat_mode = session.chat_mode;
            self.agent_enhance_prompt_before_send = session.enhance_prompt_before_send;
            self.agent_chat_usage = session.usage;
            self.agent_context_items = session.pending_context_items;
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
        let rows = n.div_ceil(cols);
        (rows as u8, cols as u8)
    }

    fn set_count_and_dims(&mut self, count: u8) {
        self.terminal_count = count.max(1);
        let (rows, cols) = Self::grid_dims_for_count(self.terminal_count);
        self.grid_rows = rows;
        self.grid_cols = cols;
    }
}

fn should_have_workspace_pinned_tabs(workspace: &WorkspaceEntry) -> bool {
    workspace_entry_has_folder(workspace) || workspace.configuring
}

fn ensure_workspace_pinned_tabs(workspace: &mut WorkspaceEntry) {
    if !should_have_workspace_pinned_tabs(workspace) {
        return;
    }
    if !workspace
        .center_tabs
        .iter()
        .any(|tab| matches!(tab.kind, CenterTabKind::Kanban))
    {
        workspace.center_tabs.insert(0, CenterTab::kanban());
    }
    // Collapse every view-mode tab into a single canonical one. The terminal
    // grid / Canvas / Swarm views are *one* mutable tab; older or corrupted
    // snapshots could carry more than one mode tab, which then rendered as two
    // centered tabs (e.g. Terminals + Swarm) sharing `CENTER_TERMINALS_TAB_ID`
    // — making the duplicate appear unclickable. Keep the first, drop the rest.
    let mut seen_mode_tab = false;
    workspace.center_tabs.retain(|tab| {
        let is_mode = is_workspace_mode_tab_kind(&tab.kind) || tab.id == CENTER_TERMINALS_TAB_ID;
        if is_mode {
            if seen_mode_tab {
                return false;
            }
            seen_mode_tab = true;
        }
        true
    });
    if let Some(tab) = workspace
        .center_tabs
        .iter_mut()
        .find(|tab| is_workspace_mode_tab_kind(&tab.kind) || tab.id == CENTER_TERMINALS_TAB_ID)
    {
        tab.id = CENTER_TERMINALS_TAB_ID;
        tab.title = workspace.view_mode.title().into();
        tab.kind = workspace.view_mode.tab_kind();
    } else {
        let insert_at = workspace
            .center_tabs
            .iter()
            .position(|tab| matches!(tab.kind, CenterTabKind::Kanban))
            .map(|idx| idx.saturating_add(1))
            .unwrap_or(0)
            .min(workspace.center_tabs.len());
        workspace
            .center_tabs
            .insert(insert_at, CenterTab::mode(workspace.view_mode));
    }
    workspace.center_tabs.sort_by_key(|tab| match tab.kind {
        CenterTabKind::Kanban => (0_u8, tab.id),
        CenterTabKind::Terminals | CenterTabKind::Canvas | CenterTabKind::Swarm => (1_u8, tab.id),
        _ => (2_u8, tab.id),
    });
}

fn is_workspace_mode_tab_kind(kind: &CenterTabKind) -> bool {
    matches!(
        kind,
        CenterTabKind::Terminals | CenterTabKind::Canvas | CenterTabKind::Swarm
    )
}

fn valid_canvas_edge(source: &CanvasPortRef, target: &CanvasPortRef) -> bool {
    use CanvasNodeKind::{AgentHub, Terminal};
    use CanvasPortDirection::{AgentCommand, AgentObserve, Stdin, Stdout};

    if source == target {
        return false;
    }
    matches!(
        (
            source.node_kind,
            source.direction,
            target.node_kind,
            target.direction
        ),
        (Terminal, Stdout, Terminal, Stdin)
            | (Terminal, Stdout, AgentHub, AgentObserve)
            | (AgentHub, AgentCommand, Terminal, Stdin)
    )
}

/// Repair `center_active_tab_id` / `center_next_tab_id` so they stay
/// consistent with `center_tabs`. Real/configuring workspaces always get the
/// pinned Kanban and current view-mode tabs; ephemeral shell workspaces keep
/// only the tabs the caller explicitly opens.
fn repair_center_tab_state(workspace: &mut WorkspaceEntry) {
    ensure_workspace_pinned_tabs(workspace);
    if workspace.center_tabs.is_empty() {
        workspace.center_active_tab_id = 0;
    } else if !workspace
        .center_tabs
        .iter()
        .any(|tab| tab.id == workspace.center_active_tab_id)
    {
        workspace.center_active_tab_id = workspace
            .center_tabs
            .iter()
            .find(|tab| tab.id == CENTER_TERMINALS_TAB_ID)
            .or_else(|| workspace.center_tabs.first())
            .map(|tab| tab.id)
            .unwrap_or(0);
    }
    let max_id = workspace
        .center_tabs
        .iter()
        .map(|tab| tab.id)
        .max()
        .unwrap_or(CENTER_TERMINALS_TAB_ID);
    workspace.center_next_tab_id = workspace
        .center_next_tab_id
        .max(max_id.saturating_add(1))
        .max(default_center_next_tab_id());
}

/// True when a workspace is an ephemeral "shell" — no folder, not in the
/// configurator. Created by `open_center_settings_tab` when no real
/// workspace is open. These are hidden from the sidebar and from the
/// persisted snapshot.
#[inline]
pub(crate) fn is_shell_workspace(ws: &WorkspaceEntry) -> bool {
    !workspace_entry_has_folder(ws) && !ws.configuring
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
        if normalized.is_empty() || is_supported_terminal_agent_slug(&normalized) {
            out.push(normalized);
        } else {
            let supported = supported_terminal_agent_slugs()
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "unsupported agent slug: {slug} (supported: {supported})"
            ));
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
    /// `Some(connection_id)` selects an SSH remote for this workspace. `None`
    /// (default) keeps it local. For remote, `cwd_display` becomes the optional
    /// remote start directory rather than a validated local path.
    pub remote_connection_id: Option<String>,
    pub workspace_kind: WorkspaceDraftKind,
    pub worktree_base_cwd: String,
    pub worktree_branch: String,
    pub worktree_start_point: String,
    pub worktree_path: String,
    /// Selected BLXCode harness session-role slug (specialized skill), or `None`
    /// for the default agent. Carried into the created `WorkspaceEntry`.
    pub session_role: Option<String>,
    /// Optional per-slot friendly names (index = slot 0..terminal_count). Empty
    /// entries keep the deterministic terminal name. Captured by presets.
    pub slot_names: Vec<String>,
    /// Selected CLI model id per agent row, parallel to `agent_counts` /
    /// `WORKSPACE_FLEET_AGENT_SLUGS`. Empty = the agent's default model.
    pub agent_models: [String; 5],
    /// Selected reasoning effort per agent row. Empty = the CLI's default
    /// effort, or no launch override for CLIs that only support config-file
    /// effort.
    pub agent_efforts: [String; 5],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkspaceDraftKind {
    #[default]
    Normal,
    Worktree,
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
            remote_connection_id: None,
            workspace_kind: WorkspaceDraftKind::Normal,
            worktree_base_cwd: String::new(),
            worktree_branch: String::new(),
            worktree_start_point: String::new(),
            worktree_path: String::new(),
            session_role: None,
            slot_names: Vec::new(),
            agent_models: Default::default(),
            agent_efforts: Default::default(),
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

/// Build per-slot model ids parallel to `slot_agent_labels`. Each slot's label
/// (agent slug) is matched to its row in `WORKSPACE_FLEET_AGENT_SLUGS` to pull
/// the chosen `agent_models` entry; unknown/empty labels yield an empty model.
#[must_use]
pub fn fleet_slot_models_for_labels(labels: &[String], agent_models: &[String; 5]) -> Vec<String> {
    labels
        .iter()
        .map(|label| {
            let slug = label.trim();
            WORKSPACE_FLEET_AGENT_SLUGS
                .iter()
                .position(|s| *s == slug)
                .and_then(|row| agent_models.get(row))
                .map(|m| m.trim().to_string())
                .unwrap_or_default()
        })
        .collect()
}

/// Build per-slot reasoning-effort ids parallel to `slot_agent_labels`. Each
/// slot's label (agent slug) is matched to its row in
/// `WORKSPACE_FLEET_AGENT_SLUGS`; unknown/empty labels yield an empty effort.
#[must_use]
pub fn fleet_slot_efforts_for_labels(
    labels: &[String],
    agent_efforts: &[String; 5],
) -> Vec<String> {
    labels
        .iter()
        .map(|label| {
            let slug = label.trim();
            WORKSPACE_FLEET_AGENT_SLUGS
                .iter()
                .position(|s| *s == slug)
                .and_then(|row| agent_efforts.get(row))
                .map(|e| e.trim().to_string())
                .unwrap_or_default()
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    Plans,
    Memory,
    Rules,
    Skills,
}

/// Kategorien in den Harness-Einstellungen (Befehlspalette).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessSettingsCategory {
    App,
    Appearance,
    Shortcuts,
    ApiKeys,
    Workspace,
    AgentProvider,
    Heartbeat,
    Remote,
    Memory,
    Mcp,
    Plugins,
    Voice,
    Image,
    CodeEditor,
}

/// Live state of the "Close Terminals tab" confirmation overlay. The 10s
/// countdown disables the confirm button so accidental double-clicks can't
/// destroy the workspace. `generation` tags the active timer so a stale
/// async tick can't override a freshly-opened dialog (rapid-fire close).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseTerminalsConfirm {
    pub workspace_id: u64,
    pub seconds_left: u8,
    pub generation: u32,
}

/// A pending confirmation request rendered by the shared `ConfirmDialog`
/// overlay. The component is generic — every string is supplied by the
/// caller — so it can stand in for any destructive action's confirmation.
/// `on_confirm` runs when the user accepts; the dialog dismisses itself
/// afterwards either way.
#[derive(Clone)]
pub struct ConfirmRequest {
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub cancel_label: String,
    /// Style the confirm button as destructive (red).
    pub danger: bool,
    pub on_confirm: Callback<()>,
    pub on_cancel: Option<Callback<()>>,
}

#[derive(Clone, Copy)]
pub struct HarnessUiService {
    palette_open: RwSignal<bool>,
    settings_open: RwSignal<bool>,
    quick_open_open: RwSignal<bool>,
    find_file_open: RwSignal<bool>,
    find_file_query: RwSignal<String>,
    find_file_selection: RwSignal<usize>,
    palette_query: RwSignal<String>,
    palette_selection: RwSignal<usize>,
    quick_open_query: RwSignal<String>,
    quick_open_selection: RwSignal<usize>,
    settings_category: RwSignal<HarnessSettingsCategory>,
    /// Tmux-style prefix (`Ctrl+b`) armed; second key pending.
    prefix_armed: RwSignal<bool>,
    prefix_gen: RwSignal<u32>,
    close_terminals_confirm: RwSignal<Option<CloseTerminalsConfirm>>,
    close_terminals_gen: RwSignal<u32>,
    confirm_request: RwSignal<Option<ConfirmRequest>>,
}

impl HarnessUiService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            palette_open: RwSignal::new(false),
            settings_open: RwSignal::new(false),
            quick_open_open: RwSignal::new(false),
            find_file_open: RwSignal::new(false),
            find_file_query: RwSignal::new(String::new()),
            find_file_selection: RwSignal::new(0),
            palette_query: RwSignal::new(String::new()),
            palette_selection: RwSignal::new(0),
            quick_open_query: RwSignal::new(String::new()),
            quick_open_selection: RwSignal::new(0),
            settings_category: RwSignal::new(HarnessSettingsCategory::App),
            prefix_armed: RwSignal::new(false),
            prefix_gen: RwSignal::new(0),
            close_terminals_confirm: RwSignal::new(None),
            close_terminals_gen: RwSignal::new(0),
            confirm_request: RwSignal::new(None),
        }
    }

    /// Reactive accessor for the shared confirmation overlay.
    /// `None` = closed; `Some(_)` = dialog visible.
    #[must_use]
    pub fn confirm_request(&self) -> RwSignal<Option<ConfirmRequest>> {
        self.confirm_request
    }

    /// Open the shared confirmation dialog. Replaces any pending request.
    pub fn request_confirm(&self, request: ConfirmRequest) {
        self.confirm_request.set(Some(request));
    }

    /// Close the shared confirmation dialog without running its action.
    pub fn dismiss_confirm(&self) {
        self.confirm_request.set(None);
    }

    /// Reactive accessor for the Terminals-close confirmation overlay.
    /// `None` = closed; `Some(_)` = dialog visible with `seconds_left` ticks.
    #[must_use]
    pub fn close_terminals_confirm(&self) -> RwSignal<Option<CloseTerminalsConfirm>> {
        self.close_terminals_confirm
    }

    /// Open the confirmation overlay for closing the Terminals tab of
    /// `workspace_id`. Spawns a 1s tick loop that decrements
    /// `seconds_left` from 3 → 0; the confirm button stays disabled until
    /// it hits 0. A fresh `generation` lets a later call cancel an earlier
    /// in-flight countdown.
    pub fn request_close_terminals_tab(&self, workspace_id: u64) {
        const COUNTDOWN_SECS: u8 = 3;
        let gen = self.close_terminals_gen.get_untracked().wrapping_add(1);
        self.close_terminals_gen.set(gen);
        self.close_terminals_confirm
            .set(Some(CloseTerminalsConfirm {
                workspace_id,
                seconds_left: COUNTDOWN_SECS,
                generation: gen,
            }));
        let me = *self;
        spawn_local(async move {
            let mut remaining = COUNTDOWN_SECS;
            while remaining > 0 {
                TimeoutFuture::new(1000).await;
                // Bail out if a newer dialog (or close) superseded us.
                let snapshot = me.close_terminals_confirm.get_untracked();
                let Some(state) = snapshot else { return };
                if state.generation != gen {
                    return;
                }
                remaining = remaining.saturating_sub(1);
                me.close_terminals_confirm.set(Some(CloseTerminalsConfirm {
                    workspace_id: state.workspace_id,
                    seconds_left: remaining,
                    generation: gen,
                }));
            }
        });
    }

    pub fn dismiss_close_terminals_confirm(&self) {
        self.close_terminals_gen.update(|g| *g = g.wrapping_add(1));
        self.close_terminals_confirm.set(None);
    }

    pub fn prefix_armed(&self) -> RwSignal<bool> {
        self.prefix_armed
    }

    pub fn arm_prefix(&self) {
        self.prefix_armed.set(true);
        let gen = self.prefix_gen.get_untracked().wrapping_add(1);
        self.prefix_gen.set(gen);
        let ui = *self;
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(1500).await;
            if ui.prefix_gen.get_untracked() == gen {
                ui.prefix_armed.set(false);
            }
        });
    }

    pub fn clear_prefix(&self) {
        self.prefix_armed.set(false);
        self.prefix_gen.update(|g| *g = g.wrapping_add(1));
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

    /// Reactive accessors for the fuzzy file finder palette.
    #[must_use]
    pub fn find_file_open(&self) -> RwSignal<bool> {
        self.find_file_open
    }

    #[must_use]
    pub fn find_file_query(&self) -> RwSignal<String> {
        self.find_file_query
    }

    #[must_use]
    pub fn find_file_selection(&self) -> RwSignal<usize> {
        self.find_file_selection
    }

    pub fn open_find_file(&self) {
        self.close_command_palette();
        self.close_settings();
        self.close_quick_open();
        self.find_file_query.set(String::new());
        self.find_file_selection.set(0);
        self.find_file_open.set(true);
    }

    pub fn close_find_file(&self) {
        self.find_file_open.set(false);
    }

    pub fn toggle_find_file(&self) {
        let next = !self.find_file_open.get_untracked();
        if next {
            self.open_find_file();
        } else {
            self.close_find_file();
        }
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

/// Live title info a terminal cell publishes for the title-bar breadcrumb.
/// `auto` is the OSC/auto title (empty when none yet); `label` is the resolved
/// slot label (`#2` or friendly name per the naming mode); `slot` is the
/// numeric slot id used to disambiguate terminals with identical auto titles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalBreadcrumb {
    pub slot: u64,
    pub auto: String,
    pub label: String,
}

/// Application layout + workspace selection (sidebar, center, inspector).
#[derive(Clone, Copy)]
pub struct WorkbenchService {
    workspaces: RwSignal<Vec<WorkspaceEntry>>,
    active_id: RwSignal<Option<u64>>,
    recent_workspaces: RwSignal<Vec<RecentWorkspaceItem>>,
    sidebar_collapsed: RwSignal<bool>,
    sidebar_width_px: RwSignal<f64>,
    right_collapsed: RwSignal<bool>,
    right_width_px: RwSignal<f64>,
    right_tab: RwSignal<RightPanelTab>,
    browser_url: RwSignal<String>,
    /// Mehrere Seiten im eingebetteten Browser (wie Browser-Tabs).
    embedded_browser_tabs: RwSignal<Vec<EmbeddedBrowserTab>>,
    embedded_browser_active_id: RwSignal<u64>,
    embedded_browser_next_id: RwSignal<u64>,
    harness_workspace_root: RwSignal<String>,
    /// Default project / development directory used to seed new workspace
    /// cwds. Sandbox path above is reserved for BLXCode Agent sandbox
    /// actions only; this is what regular workspaces start from.
    default_project_dir: RwSignal<String>,
    /// Global default BLXCode harness session-role slug for newly-created
    /// workspaces. Persisted in agent provider settings, mirrored here so
    /// Settings panes and the workspace wizard share one reactive value.
    default_session_role: RwSignal<Option<String>>,
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
    /// Persistent BLXCode Agent notification feed shown in the titlebar bell.
    agent_notifications: RwSignal<Vec<AgentNotification>>,
    /// Keys recently cleared in-memory while the async backend disk-write is still in flight.
    /// The notification poller filters these out so a freshly-cleared key cannot reappear
    /// before `workbench_clear_terminal_notifications` lands on disk.
    pending_clears: RwSignal<HashSet<String>>,
    /// Last focused terminal per workspace, keyed by the workspace's
    /// `storage_key` (UUID). Survives workspace switches. The value is the
    /// full `terminal_key`.
    focused_terminal_by_workspace: RwSignal<HashMap<String, String>>,
    /// Live title info per terminal, keyed by the full `terminal_key`.
    /// Published by each terminal cell as the header title changes. Read by
    /// the app title bar breadcrumb. Session-only; not part of
    /// `WorkbenchSnapshot`.
    terminal_titles: RwSignal<HashMap<String, TerminalBreadcrumb>>,
    memory_color_presets: RwSignal<Vec<MemoryColorPreset>>,
    /// Session-only image context; intentionally not part of WorkbenchSnapshot.
    agent_image_context: RwSignal<HashMap<u64, Vec<WorkspaceAgentImage>>>,
    /// Bumped when the active workspace repo root changes (e.g. inline configure
    /// commit). Agent timeline/draft updates must not re-subscribe sidebar git checks.
    sidebar_repo_epoch: RwSignal<u32>,
    /// Bumped when `.agents/plans/` content changes through any workspace UI.
    /// PlansPanel and WorkspaceKanban both subscribe to this to stay in sync
    /// without directly coupling their local component state.
    plans_epoch: RwSignal<u32>,
    /// Session-only request for Kanban to reveal a specific plan after it has
    /// loaded the current board.
    kanban_plan_focus: RwSignal<Option<KanbanPlanFocusRequest>>,
    /// Old `terminal_key`s whose PTY is currently being adopted by a new
    /// cell mount (Cross-workspace transfer or extract-to-new-workspace).
    /// While present, the unmounting source cell's cleanup must NOT call
    /// `pty_kill` — the new cell owns the live PTY under its new key.
    /// Cleared automatically after the corresponding adoption completes
    /// or after a short safety timeout.
    terminal_move_guards: RwSignal<HashMap<String, String>>,
    /// Adoptable PTY sessions keyed by the **new** `terminal_key`. The
    /// freshly mounted target cell consumes this to skip `pty_spawn` and
    /// reuse the live session instead.
    terminal_adopt_pending: RwSignal<HashMap<String, u64>>,
    /// Terminal keys currently owned by a popout child window. The value is
    /// the Tauri window label so the main view can focus the child instead of
    /// mounting a duplicate xterm renderer.
    terminal_popouts: RwSignal<HashMap<String, String>>,
}

/// Cross-workspace terminal slot move; returned by
/// [`WorkbenchService::transfer_terminal_slot`] so the caller can wire up
/// any follow-up adoption (PTY skip-kill, sessions.json key rewrite).
///
/// `pane_ids` lists the per-pane suffix used to build `terminal_key`s on
/// both sides — `{old_storage_key}:{old_slot_id}:{pane_id}` →
/// `{new_storage_key}:{new_slot_id}:{pane_id}`. Pane ids are preserved as
/// they were (locally-unique inside the slot), so an existing xterm/PTY
/// remount just needs the storage/slot prefix swapped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSlotMove {
    pub from_workspace_id: u64,
    pub to_workspace_id: u64,
    pub old_storage_key: String,
    pub new_storage_key: String,
    pub old_slot_id: u64,
    pub new_slot_id: u64,
    pub pane_ids: Vec<u64>,
    pub agent_slug: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSlotSplitMove {
    pub workspace_id: u64,
    pub storage_key: String,
    pub source_slot_id: u64,
    pub target_slot_id: u64,
    pub old_pane_id: u64,
    pub new_pane_id: u64,
    pub action: TerminalSlotDropAction,
    pub agent_slug: String,
}

impl TerminalSlotSplitMove {
    #[must_use]
    pub fn terminal_key_pair(&self) -> (String, String) {
        (
            format!(
                "{}:{}:{}",
                self.storage_key, self.source_slot_id, self.old_pane_id
            ),
            format!(
                "{}:{}:{}",
                self.storage_key, self.target_slot_id, self.new_pane_id
            ),
        )
    }
}

impl TerminalSlotMove {
    /// `(old_terminal_key, new_terminal_key)` pairs derived from the move,
    /// in pane order. Both the PTY/session live registries and the on-disk
    /// `sessions.json` / `notifications.json` are keyed by `terminal_key`,
    /// so every callsite that needs to remap must walk these pairs.
    #[must_use]
    pub fn terminal_key_pairs(&self) -> Vec<(String, String)> {
        self.pane_ids
            .iter()
            .map(|&pane_id| {
                (
                    format!("{}:{}:{}", self.old_storage_key, self.old_slot_id, pane_id),
                    format!("{}:{}:{}", self.new_storage_key, self.new_slot_id, pane_id),
                )
            })
            .collect()
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KanbanPlanFocusRequest {
    pub workspace_id: u64,
    pub plan_path: String,
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
        let default_project_dir = read_local_storage(DEFAULT_PROJECT_DIR_KEY).unwrap_or_default();
        let memory_color_presets = read_memory_color_presets();

        let first_tab_id = 1_u64;

        let sidebar_width_px = read_local_storage(SIDEBAR_WIDTH_PX_KEY)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|w| w.is_finite() && *w >= 120.0)
            .unwrap_or(SIDEBAR_WIDTH_PX_DEFAULT);

        Self {
            workspaces: RwSignal::new(Vec::new()),
            active_id: RwSignal::new(None),
            recent_workspaces: RwSignal::new(Vec::new()),
            sidebar_collapsed: RwSignal::new(false),
            sidebar_width_px: RwSignal::new(sidebar_width_px),
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
            default_project_dir: RwSignal::new(default_project_dir),
            default_session_role: RwSignal::new(None),
            workspace_next_id: RwSignal::new(1),
            workspace_drafts: RwSignal::new(HashMap::new()),
            workspace_config_steps: RwSignal::new(HashMap::new()),
            pty_sessions: RwSignal::new(HashMap::new()),
            pending_memory_note: RwSignal::new(None),
            terminal_layout_tick: RwSignal::new(0),
            notifications: RwSignal::new(HashMap::new()),
            agent_notifications: RwSignal::new(Vec::new()),
            pending_clears: RwSignal::new(HashSet::new()),
            focused_terminal_by_workspace: RwSignal::new(HashMap::new()),
            terminal_titles: RwSignal::new(HashMap::new()),
            memory_color_presets: RwSignal::new(memory_color_presets),
            agent_image_context: RwSignal::new(HashMap::new()),
            sidebar_repo_epoch: RwSignal::new(0),
            plans_epoch: RwSignal::new(0),
            kanban_plan_focus: RwSignal::new(None),
            terminal_move_guards: RwSignal::new(HashMap::new()),
            terminal_adopt_pending: RwSignal::new(HashMap::new()),
            terminal_popouts: RwSignal::new(HashMap::new()),
        }
    }

    pub fn sidebar_repo_epoch(&self) -> RwSignal<u32> {
        self.sidebar_repo_epoch
    }

    pub fn bump_sidebar_repo_epoch(&self) {
        self.sidebar_repo_epoch.update(|n| *n = n.wrapping_add(1));
    }

    pub fn plans_epoch(&self) -> RwSignal<u32> {
        self.plans_epoch
    }

    pub fn bump_plans_epoch(&self) {
        self.plans_epoch.update(|n| *n = n.wrapping_add(1));
    }

    pub fn kanban_plan_focus_request(&self) -> RwSignal<Option<KanbanPlanFocusRequest>> {
        self.kanban_plan_focus
    }

    pub fn open_center_kanban_plan(&self, workspace_id: u64, plan_path: String) {
        let plan_path = plan_path.trim().to_string();
        if plan_path.is_empty() {
            return;
        }
        self.kanban_plan_focus.set(Some(KanbanPlanFocusRequest {
            workspace_id,
            plan_path,
        }));
        self.open_center_kanban_tab(workspace_id);
    }

    pub fn notifications(&self) -> RwSignal<HashMap<String, u32>> {
        self.notifications
    }

    pub fn default_session_role(&self) -> RwSignal<Option<String>> {
        self.default_session_role
    }

    pub fn set_default_session_role(&self, role: Option<String>) {
        let normalized = role.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        self.default_session_role.set(normalized);
    }

    pub fn agent_notifications(&self) -> RwSignal<Vec<AgentNotification>> {
        self.agent_notifications
    }

    pub fn set_agent_notifications(&self, mut items: Vec<AgentNotification>) {
        items.sort_by_key(|item| std::cmp::Reverse(item.created_at));
        self.agent_notifications.set(items);
    }

    pub fn upsert_agent_notification(&self, item: AgentNotification) {
        self.agent_notifications.update(|items| {
            if let Some(existing) = items.iter_mut().find(|n| n.id == item.id) {
                *existing = item;
            } else {
                items.push(item);
            }
            items.sort_by_key(|item| std::cmp::Reverse(item.created_at));
        });
    }

    pub fn remove_agent_notification(&self, id: &str) {
        self.agent_notifications
            .update(|items| items.retain(|n| n.id != id));
    }

    pub fn mark_agent_notification_read(&self, id: &str) {
        self.agent_notifications.update(|items| {
            if let Some(item) = items.iter_mut().find(|n| n.id == id) {
                item.read = true;
            }
        });
    }

    pub fn mark_all_agent_notifications_read(&self) {
        self.agent_notifications.update(|items| {
            for item in items {
                item.read = true;
            }
        });
    }

    /// Reactive accessor to the live PTY session map (`"{ws}:{slot}:{pane}" → session_id`).
    /// UI components that want to react when terminals register or close
    /// should `.get()` on this signal (cheap; the map is small).
    pub fn pty_sessions_signal(&self) -> RwSignal<HashMap<String, u64>> {
        self.pty_sessions
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
        let (slot_id, pane_id) = terminal_key_slot_pane(key)?;
        self.workspaces.with_untracked(|list| {
            let ws = list.iter().find(|w| w.storage_key == storage_key)?;
            ws.pane_agent_state(slot_id, pane_id)
                .map(|agent| agent.agent_label.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
        })
    }

    /// `Some(connection_id)` when the terminal belongs to an SSH remote
    /// workspace. Drives `pty_spawn_remote` vs the local `pty_spawn` path.
    pub fn remote_connection_for_terminal_key(&self, terminal_key: &str) -> Option<String> {
        let storage_key = super::agent_accent::terminal_key_storage_key(terminal_key)?;
        self.workspaces.with_untracked(|list| {
            list.iter()
                .find(|w| w.storage_key == storage_key)
                .and_then(|w| w.remote_connection_id.clone())
        })
    }

    /// Selected CLI-agent model id for the terminal's slot, if any. Empty/blank
    /// entries return `None` so the agent's own default model is used.
    pub fn agent_model_for_terminal_key(&self, terminal_key: &str) -> Option<String> {
        let storage_key = super::agent_accent::terminal_key_storage_key(terminal_key)?;
        let (slot_id, pane_id) = terminal_key_slot_pane(terminal_key)?;
        self.workspaces.with_untracked(|list| {
            let ws = list.iter().find(|w| w.storage_key == storage_key)?;
            ws.pane_agent_state(slot_id, pane_id)
                .map(|agent| agent.agent_model.trim().to_string())
                .filter(|s| !s.is_empty())
        })
    }

    /// Selected CLI-agent reasoning effort for the terminal's slot, if any.
    /// Empty/blank entries return `None` so the CLI's own default effort is
    /// used.
    pub fn agent_effort_for_terminal_key(&self, terminal_key: &str) -> Option<String> {
        let storage_key = super::agent_accent::terminal_key_storage_key(terminal_key)?;
        let (slot_id, pane_id) = terminal_key_slot_pane(terminal_key)?;
        self.workspaces.with_untracked(|list| {
            let ws = list.iter().find(|w| w.storage_key == storage_key)?;
            ws.pane_agent_state(slot_id, pane_id)
                .map(|agent| agent.agent_effort.trim().to_string())
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

    /// Publish the live title info for a terminal. Drives the title-bar
    /// breadcrumb. `auto` is the OSC/auto title (empty when none), `label` the
    /// resolved slot label, `slot` the numeric slot id.
    pub fn set_terminal_title(&self, terminal_key: String, slot: u64, auto: String, label: String) {
        let next = TerminalBreadcrumb { slot, auto, label };
        self.terminal_titles.update(|m| {
            match m.get(&terminal_key) {
                Some(existing) if *existing == next => {}
                _ => {
                    m.insert(terminal_key, next);
                }
            };
        });
    }

    /// Drop a terminal's published title (on cell unmount).
    pub fn clear_terminal_title(&self, terminal_key: &str) {
        self.terminal_titles.update(|m| {
            m.remove(terminal_key);
        });
    }

    /// Resolve the focused terminal's breadcrumb for the active workspace,
    /// but only while its active center tab is the Terminals grid. Returns
    /// `(slot, text)` where `slot` is `Some(n)` only when an auto title is
    /// shown (so identical auto titles can be told apart by their slot
    /// number); when falling back to the slot label, `slot` is `None` since
    /// the label already carries the number/name. Reactive — call inside a
    /// tracking scope.
    #[must_use]
    pub fn active_terminal_breadcrumb_title(&self) -> Option<(Option<u64>, String)> {
        let active = self.active_id.get()?;
        let (storage_key, is_terminals) = self.workspaces.with(|list| {
            let ws = list.iter().find(|w| w.id == active)?;
            let is_terminals = ws
                .center_tabs
                .iter()
                .find(|t| t.id == ws.center_active_tab_id)
                .map(|t| matches!(t.kind, CenterTabKind::Terminals))
                .unwrap_or(false);
            Some((ws.storage_key.clone(), is_terminals))
        })?;
        if !is_terminals {
            return None;
        }
        let focused = self
            .focused_terminal_by_workspace
            .with(|m| m.get(&storage_key).cloned())?;
        let info = self.terminal_titles.with(|m| m.get(&focused).cloned())?;
        let auto = info.auto.trim();
        if !auto.is_empty() {
            return Some((Some(info.slot), auto.to_string()));
        }
        let label = info.label.trim();
        if label.is_empty() {
            None
        } else {
            Some((None, label.to_string()))
        }
    }

    /// True when a notification's terminal key still maps to an agent-attached
    /// slot in the open workspace. Filters out ghosts from previous sessions
    /// (closed slots and slots without an agent label).
    fn notification_key_is_live(&self, key: &str) -> bool {
        let Some(storage_key) = super::agent_accent::terminal_key_storage_key(key) else {
            return false;
        };
        let Some((slot_id, pane_id)) = terminal_key_slot_pane(key) else {
            return false;
        };
        self.workspaces.with_untracked(|list| {
            let Some(ws) = list.iter().find(|w| w.storage_key == storage_key) else {
                return false;
            };
            ws.pane_agent_state(slot_id, pane_id)
                .map(|agent| !agent.agent_label.trim().is_empty())
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

    #[must_use]
    pub fn terminal_popout_label(&self, terminal_key: &str) -> Option<String> {
        self.terminal_popouts
            .with(|m| m.get(terminal_key).cloned())
    }

    pub fn prepare_terminal_popout(&self, terminal_key: &str, label: String) {
        let key = terminal_key.to_string();
        if let Some(sid) = self.pty_sessions.with_untracked(|m| m.get(&key).copied()) {
            self.terminal_move_guards.update(|m| {
                m.insert(key.clone(), key.clone());
            });
            self.terminal_adopt_pending.update(|m| {
                m.insert(key.clone(), sid);
            });
        }
        self.terminal_popouts.update(|m| {
            m.insert(key, label);
        });
    }

    pub fn prepare_terminal_popout_return(&self, terminal_key: &str) {
        let key = terminal_key.to_string();
        if let Some(sid) = self.pty_sessions.with_untracked(|m| m.get(&key).copied()) {
            self.terminal_move_guards.update(|m| {
                m.insert(key.clone(), key.clone());
            });
            self.terminal_adopt_pending.update(|m| {
                m.insert(key, sid);
            });
        }
    }

    pub fn clear_terminal_popout(&self, terminal_key: &str) {
        self.terminal_popouts.update(|m| {
            m.remove(terminal_key);
        });
        self.bump_terminal_layout();
    }

    pub fn return_terminal_popout_by_label(&self, label: &str) {
        let key = self.terminal_popouts.with_untracked(|m| {
            m.iter()
                .find_map(|(key, value)| (value == label).then(|| key.clone()))
        });
        if let Some(key) = key {
            self.prepare_terminal_popout_return(&key);
            self.clear_terminal_popout(&key);
        }
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
        Self::invalidate_agent_environment_cache();
    }

    fn invalidate_agent_environment_cache() {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async {
            let _ = agent_environment_invalidate().await;
        });
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

    /// SSH connection id of the active workspace, or `None` when it is local.
    /// Drives the remote-vs-local fs/git command routing.
    pub fn active_remote_connection_id(&self) -> Option<String> {
        self.with_active_workspace(|w| w.remote_connection_id.clone())
            .flatten()
    }

    pub fn active_sidebar_explorer_open(&self) -> bool {
        self.with_active_workspace(|w| w.sidebar_explorer_open)
            .unwrap_or(true)
    }

    pub fn set_active_sidebar_explorer_open(&self, open: bool) {
        self.update_active_workspace(|w| w.sidebar_explorer_open = open);
    }

    pub fn active_sidebar_graph_open(&self) -> bool {
        self.with_active_workspace(|w| w.sidebar_graph_open)
            .unwrap_or(false)
    }

    pub fn set_active_sidebar_graph_open(&self, open: bool) {
        self.update_active_workspace(|w| w.sidebar_graph_open = open);
    }

    pub fn active_sidebar_diff_open(&self) -> bool {
        self.with_active_workspace(|w| w.sidebar_diff_open)
            .unwrap_or(true)
    }

    pub fn set_active_sidebar_diff_open(&self, open: bool) {
        self.update_active_workspace(|w| w.sidebar_diff_open = open);
    }

    pub fn active_sidebar_explorer_expanded_paths(&self) -> Vec<String> {
        self.with_active_workspace(|w| w.sidebar_explorer_expanded_paths.clone())
            .unwrap_or_default()
    }

    pub fn set_active_sidebar_explorer_expanded_paths(&self, paths: Vec<String>) {
        self.update_active_workspace(|w| w.sidebar_explorer_expanded_paths = paths);
    }

    #[must_use]
    pub fn center_tabs_for_workspace(&self, workspace_id: u64) -> Vec<CenterTab> {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.center_tabs.clone())
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn active_center_tab_id_for_workspace(&self, workspace_id: u64) -> u64 {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.center_active_tab_id)
                .unwrap_or(0)
        })
    }

    #[must_use]
    pub fn view_mode_for_workspace(&self, workspace_id: u64) -> WorkspaceViewMode {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.view_mode)
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn active_workspace_view_mode(&self) -> Option<WorkspaceViewMode> {
        let id = self.active_id.get()?;
        Some(self.view_mode_for_workspace(id))
    }

    pub fn set_workspace_view_mode(&self, workspace_id: u64, mode: WorkspaceViewMode) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            workspace.view_mode = mode;
            if let Some(tab) = workspace.center_tabs.iter_mut().find(|tab| {
                is_workspace_mode_tab_kind(&tab.kind) || tab.id == CENTER_TERMINALS_TAB_ID
            }) {
                tab.id = CENTER_TERMINALS_TAB_ID;
                tab.title = mode.title().into();
                tab.kind = mode.tab_kind();
            }
            workspace.center_active_tab_id = CENTER_TERMINALS_TAB_ID;
            repair_center_tab_state(workspace);
        });
        self.bump_terminal_layout();
    }

    pub fn set_canvas_terminal_layout(
        &self,
        workspace_id: u64,
        slot_id: u64,
        layout: CanvasNodeLayout,
    ) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            workspace
                .canvas_view_state
                .terminal_nodes
                .insert(slot_id, layout);
        });
        self.bump_terminal_layout();
    }

    pub fn connect_canvas_ports(
        &self,
        workspace_id: u64,
        source: CanvasPortRef,
        target: CanvasPortRef,
    ) -> Option<String> {
        if !valid_canvas_edge(&source, &target) {
            return None;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            let transfer_mode = workspace.canvas_default_transfer_mode;
            workspace.canvas_edges.push(CanvasEdge {
                id: id.clone(),
                source,
                target,
                transfer_mode,
            });
        });
        Some(id)
    }

    pub fn toggle_canvas_edge_transfer_mode(&self, workspace_id: u64, edge_id: String) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            let Some(edge) = workspace
                .canvas_edges
                .iter_mut()
                .find(|edge| edge.id == edge_id)
            else {
                return;
            };
            edge.transfer_mode = match edge.transfer_mode {
                CanvasTransferMode::Raw => CanvasTransferMode::Structured,
                CanvasTransferMode::Structured => CanvasTransferMode::Raw,
            };
        });
    }

    pub fn remove_canvas_edge(&self, workspace_id: u64, edge_id: String) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            workspace.canvas_edges.retain(|edge| edge.id != edge_id);
        });
    }

    pub fn set_swarm_node_position(&self, workspace_id: u64, node_id: String, x: f64, y: f64) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            workspace
                .swarm_view_state
                .node_positions
                .insert(node_id, SwarmNodeLayout { x, y });
        });
    }

    pub fn set_active_center_tab(&self, workspace_id: u64, tab_id: u64) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if workspace.center_tabs.iter().any(|tab| tab.id == tab_id) {
                workspace.center_active_tab_id = tab_id;
            }
            repair_center_tab_state(workspace);
        });
        self.bump_terminal_layout();
    }

    /// Close a non-terminals center tab. Terminals tabs are routed through
    /// the countdown dialog and `close_center_terminals_tab` instead. When
    /// removing the last tab from a workspace, the workspace itself is
    /// closed (save + recent) — this triggers the welcome screen if no
    /// other workspace is open.
    pub fn close_center_tab(&self, workspace_id: u64, tab_id: u64) {
        let mut became_empty = false;
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            let Some(index) = workspace
                .center_tabs
                .iter()
                .position(|tab| tab.id == tab_id)
            else {
                return;
            };
            if matches!(
                workspace.center_tabs[index].kind,
                CenterTabKind::Kanban
                    | CenterTabKind::Terminals
                    | CenterTabKind::Canvas
                    | CenterTabKind::Swarm
            ) {
                return;
            }
            workspace.center_tabs.remove(index);
            if workspace.center_active_tab_id == tab_id {
                workspace.center_active_tab_id = workspace
                    .center_tabs
                    .get(index)
                    .or_else(|| {
                        index
                            .checked_sub(1)
                            .and_then(|i| workspace.center_tabs.get(i))
                    })
                    .map(|tab| tab.id)
                    .unwrap_or(0);
            }
            repair_center_tab_state(workspace);
            became_empty = workspace.center_tabs.is_empty();
        });
        if became_empty {
            // close_workspace MUST run outside workspaces.update — see
            // finalize_workspace_close for the deadlock note.
            self.close_workspace(workspace_id);
        } else {
            self.bump_terminal_layout();
        }
    }

    /// Close the Terminals center tab AND the entire workspace it belongs
    /// to. Invoked after the 10-second confirmation dialog has been
    /// accepted; behaves like a sidebar close (saves sessions, pushes
    /// to recent, switches `active_id` to the next workspace or `None`).
    pub fn close_center_terminals_tab(&self, workspace_id: u64) {
        self.close_workspace(workspace_id);
    }

    /// Ensure a Terminals tab exists at index 0 of the given workspace and
    /// make it the active tab. Used by the "new terminal" shortcut and
    /// the optional palette entry that reopens the terminal grid.
    pub fn open_center_terminals_tab(&self, workspace_id: u64) {
        self.set_workspace_view_mode(workspace_id, WorkspaceViewMode::Grid);
    }

    pub fn open_center_canvas_tab(&self, workspace_id: u64) {
        self.set_workspace_view_mode(workspace_id, WorkspaceViewMode::Canvas);
    }

    pub fn open_center_swarm_tab(&self, workspace_id: u64) {
        self.set_workspace_view_mode(workspace_id, WorkspaceViewMode::Swarm);
    }

    pub fn open_center_kanban_tab(&self, workspace_id: u64) {
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ensure_workspace_pinned_tabs(workspace);
            workspace.center_active_tab_id = CENTER_KANBAN_TAB_ID;
            repair_center_tab_state(workspace);
        });
        self.bump_terminal_layout();
    }

    pub fn open_center_settings_tab(&self, cat: HarnessSettingsCategory) {
        let workspace_id = self
            .active_id
            .get_untracked()
            .unwrap_or_else(|| self.ensure_tab_host_workspace());
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(tab) = workspace
                .center_tabs
                .iter()
                .find(|tab| matches!(tab.kind, CenterTabKind::Settings))
            {
                workspace.center_active_tab_id = tab.id;
                repair_center_tab_state(workspace);
                return;
            }
            let id = workspace
                .center_next_tab_id
                .max(default_center_next_tab_id());
            workspace.center_next_tab_id = id.saturating_add(1);
            workspace.center_tabs.push(CenterTab {
                id,
                title: settings_tab_title(cat).into(),
                kind: CenterTabKind::Settings,
            });
            workspace.center_active_tab_id = id;
            repair_center_tab_state(workspace);
        });
        self.bump_terminal_layout();
    }

    pub fn open_center_memory_tab(&self) {
        let workspace_id = self
            .active_id
            .get_untracked()
            .unwrap_or_else(|| self.ensure_tab_host_workspace());
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(tab) = workspace
                .center_tabs
                .iter()
                .find(|tab| matches!(tab.kind, CenterTabKind::Memory))
            {
                workspace.center_active_tab_id = tab.id;
                repair_center_tab_state(workspace);
                return;
            }
            let id = workspace
                .center_next_tab_id
                .max(default_center_next_tab_id());
            workspace.center_next_tab_id = id.saturating_add(1);
            workspace.center_tabs.push(CenterTab {
                id,
                title: "Memory".into(),
                kind: CenterTabKind::Memory,
            });
            workspace.center_active_tab_id = id;
            repair_center_tab_state(workspace);
        });
        self.bump_terminal_layout();
    }

    /// Open (or focus) a centered Mermaid diagram gallery tab for a plan's
    /// `diagrams/` set.
    pub fn open_center_diagram_gallery_tab(&self, workspace_id: u64, slug: String) {
        let slug = slug.trim().to_string();
        if slug.is_empty() {
            return;
        }
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(tab) = workspace.center_tabs.iter().find(|tab| {
                matches!(&tab.kind, CenterTabKind::DiagramGallery { slug: existing } if existing == &slug)
            }) {
                workspace.center_active_tab_id = tab.id;
                repair_center_tab_state(workspace);
                return;
            }
            let id = workspace.center_next_tab_id.max(default_center_next_tab_id());
            workspace.center_next_tab_id = id.saturating_add(1);
            workspace.center_tabs.push(CenterTab {
                id,
                title: format!("◇ {slug}"),
                kind: CenterTabKind::DiagramGallery { slug },
            });
            workspace.center_active_tab_id = id;
            repair_center_tab_state(workspace);
        });
    }

    /// Open (or focus) a center tab showing an ephemeral diagram group from the
    /// agent timeline. Re-opening the same group (identical title + diagrams)
    /// focuses the existing tab instead of duplicating it.
    pub fn open_center_diagram_group(
        &self,
        workspace_id: u64,
        title: String,
        diagrams: Vec<TimelineDiagram>,
    ) {
        if diagrams.is_empty() {
            return;
        }
        let group_title = title.trim().to_string();
        let tab_title = if group_title.is_empty() {
            "◇ Diagrams".to_string()
        } else {
            format!("◇ {group_title}")
        };
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(tab) = workspace.center_tabs.iter().find(|tab| {
                matches!(&tab.kind, CenterTabKind::DiagramGroup { diagrams: existing, .. } if existing == &diagrams)
            }) {
                workspace.center_active_tab_id = tab.id;
                repair_center_tab_state(workspace);
                return;
            }
            let id = workspace.center_next_tab_id.max(default_center_next_tab_id());
            workspace.center_next_tab_id = id.saturating_add(1);
            workspace.center_tabs.push(CenterTab {
                id,
                title: tab_title,
                kind: CenterTabKind::DiagramGroup { title: group_title, diagrams },
            });
            workspace.center_active_tab_id = id;
            repair_center_tab_state(workspace);
        });
    }

    pub fn open_center_file_tab(&self, workspace_id: u64, rel_path: String) {
        let rel_path = rel_path.trim().trim_start_matches(['/', '\\']).to_string();
        if rel_path.is_empty() {
            return;
        }
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(tab) = workspace.center_tabs.iter().find(|tab| {
                matches!(&tab.kind, CenterTabKind::FilePreview { rel_path: existing } if existing == &rel_path)
            }) {
                workspace.center_active_tab_id = tab.id;
                repair_center_tab_state(workspace);
                return;
            }
            let id = workspace.center_next_tab_id.max(default_center_next_tab_id());
            workspace.center_next_tab_id = id.saturating_add(1);
            workspace.center_tabs.push(CenterTab {
                id,
                title: file_tab_title(&rel_path),
                kind: CenterTabKind::FilePreview { rel_path },
            });
            workspace.center_active_tab_id = id;
            repair_center_tab_state(workspace);
        });
    }

    /// Open (or focus) a `FileDiff` center tab for the given path. Reopening
    /// an existing diff tab with a different `staged` flag updates the kind
    /// in place rather than spawning a duplicate tab — the row in the
    /// sidebar already disambiguates staged vs unstaged.
    pub fn open_center_diff_tab(&self, workspace_id: u64, rel_path: String, staged: bool) {
        let rel_path = rel_path.trim().trim_start_matches(['/', '\\']).to_string();
        if rel_path.is_empty() {
            return;
        }
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if let Some(tab) = workspace.center_tabs.iter_mut().find(|tab| {
                matches!(&tab.kind, CenterTabKind::FileDiff { rel_path: existing, .. } if existing == &rel_path)
            }) {
                tab.kind = CenterTabKind::FileDiff {
                    rel_path: rel_path.clone(),
                    staged,
                };
                tab.title = diff_tab_title(&rel_path);
                let active = tab.id;
                workspace.center_active_tab_id = active;
                repair_center_tab_state(workspace);
                return;
            }
            let id = workspace.center_next_tab_id.max(default_center_next_tab_id());
            workspace.center_next_tab_id = id.saturating_add(1);
            workspace.center_tabs.push(CenterTab {
                id,
                title: diff_tab_title(&rel_path),
                kind: CenterTabKind::FileDiff { rel_path, staged },
            });
            workspace.center_active_tab_id = id;
            repair_center_tab_state(workspace);
        });
    }

    /// Create an ephemeral "shell" workspace that hosts only the settings
    /// tab when no real workspace is open. The shell has empty `cwd`,
    /// `configuring: false`, no terminal slots, and starts with an empty
    /// `center_tabs` (the caller pushes the desired tab right after).
    /// Returns the new workspace id and selects it. Shell workspaces are
    /// filtered out of the sidebar and not persisted in the snapshot.
    fn ensure_tab_host_workspace(&self) -> u64 {
        let id = self.allocate_workspace_id();
        let color = self.workspace_color_for_new_index(self.workspaces.get_untracked().len());
        let entry = WorkspaceEntry {
            id,
            storage_key: WorkspaceEntry::new_storage_key(),
            title: format!("Workspace {id}"),
            color,
            cwd: String::new(),
            terminal_count: 0,
            grid_rows: 1,
            grid_cols: 1,
            next_terminal_id: 1,
            slot_ids: Vec::new(),
            slot_agent_labels: Vec::new(),
            slot_agent_models: Vec::new(),
            slot_agent_efforts: Vec::new(),
            slot_pane_states: Vec::new(),
            configuring: false,
            agent_timeline: TimelineDoc::default(),
            agent_compose_draft: String::new(),
            agent_image_mode: false,
            agent_chat_mode: AgentChatMode::AskEdits,
            agent_enhance_prompt_before_send: false,
            architecture_llm_prose: false,
            agent_context_items: Vec::new(),
            agent_chat_sessions: Vec::new(),
            active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
            memory_category_settings: HashMap::new(),
            agent_chat_usage: ChatUsageStats::default(),
            sidebar_explorer_open: true,
            sidebar_graph_open: false,
            sidebar_diff_open: true,
            sidebar_explorer_expanded_paths: Vec::new(),
            center_tabs: Vec::new(),
            center_active_tab_id: 0,
            center_next_tab_id: default_center_next_tab_id(),
            remote_connection_id: None,
            worktree: None,
            slot_name_overrides: std::collections::HashMap::new(),
            agent_session_role: None,
            view_mode: WorkspaceViewMode::Grid,
            canvas_view_state: CanvasViewState::default(),
            canvas_edges: Vec::new(),
            canvas_default_transfer_mode: CanvasTransferMode::Structured,
            swarm_view_state: SwarmViewState::default(),
        };
        self.workspaces.update(|v| v.push(entry));
        self.active_id.set(Some(id));
        id
    }

    #[must_use]
    pub fn with_active_workspace_entry(&self) -> Option<WorkspaceEntry> {
        self.with_active_workspace(|w| w.clone())
    }

    fn with_active_workspace<R>(&self, f: impl FnOnce(&WorkspaceEntry) -> R) -> Option<R> {
        let id = self.active_id.get_untracked()?;
        self.workspaces
            .with_untracked(|ws| ws.iter().find(|w| w.id == id).map(f))
    }

    fn update_active_workspace(&self, f: impl FnOnce(&mut WorkspaceEntry)) {
        let Some(id) = self.active_id.get_untracked() else {
            return;
        };
        self.workspaces.update(|ws| {
            if let Some(w) = ws.iter_mut().find(|w| w.id == id) {
                f(w);
            }
        });
    }

    pub fn set_workspace_display(&self, id: u64, title: String, color: String) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let fallback = self.workspace_color_for_new_index(
            self.workspaces
                .get_untracked()
                .iter()
                .position(|w| w.id == id)
                .unwrap_or(0),
        );
        let color = normalize_hex_color(&color, &fallback);
        self.workspaces.update(|workspaces| {
            if let Some(workspace) = workspaces.iter_mut().find(|w| w.id == id) {
                workspace.title = title.to_string();
                workspace.color = color;
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
        let color = self.workspace_color_for_new_index(self.workspaces.get_untracked().len());
        self.workspaces.update(|workspaces| {
            workspaces.push(WorkspaceEntry {
                id,
                storage_key: WorkspaceEntry::new_storage_key(),
                title,
                color,
                cwd,
                terminal_count,
                grid_rows,
                grid_cols,
                next_terminal_id: terminal_count as u64 + 1,
                slot_ids,
                slot_agent_labels,
                slot_agent_models: Vec::new(),
                slot_agent_efforts: Vec::new(),
                slot_pane_states,
                configuring: false,
                agent_timeline: TimelineDoc::default(),
                agent_compose_draft: String::new(),
                agent_image_mode: false,
                agent_chat_mode: AgentChatMode::AskEdits,
                agent_enhance_prompt_before_send: false,
                architecture_llm_prose: false,
                agent_context_items: Vec::new(),
                agent_chat_sessions: Vec::new(),
                active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
                memory_category_settings: HashMap::new(),
                agent_chat_usage: ChatUsageStats::default(),
                sidebar_explorer_open: true,
                sidebar_graph_open: false,
                sidebar_diff_open: true,
                sidebar_explorer_expanded_paths: Vec::new(),
                center_tabs: default_center_tabs(),
                center_active_tab_id: default_center_active_tab_id(),
                center_next_tab_id: default_center_next_tab_id(),
                remote_connection_id: None,
                worktree: None,
                slot_name_overrides: std::collections::HashMap::new(),
                agent_session_role: None,
                view_mode: WorkspaceViewMode::Grid,
                canvas_view_state: CanvasViewState::default(),
                canvas_edges: Vec::new(),
                canvas_default_transfer_mode: CanvasTransferMode::Structured,
                swarm_view_state: SwarmViewState::default(),
            });
        });
        Ok(id)
    }

    pub fn open_or_create_worktree_workspace(
        &self,
        worktree: WorkspaceWorktreeMeta,
        remote_connection_id: Option<String>,
    ) -> Result<u64, String> {
        let cwd = normalize_cwd_key(&worktree.worktree_cwd);
        if cwd.is_empty() {
            return Err("worktree path empty".into());
        }
        if let Some(existing_id) = self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|workspace| {
                    normalize_cwd_key(&workspace.cwd) == cwd
                        && workspace.remote_connection_id == remote_connection_id
                })
                .map(|workspace| workspace.id)
        }) {
            self.select_workspace(existing_id);
            return Ok(existing_id);
        }

        let terminal_count = 1;
        let slot_ids: Vec<u64> = vec![1];
        let slot_pane_states = vec![SlotPaneState::default_for_slot(1)];
        let (grid_rows, grid_cols) = WorkspaceEntry::grid_dims_for_count(terminal_count);
        let id = self.allocate_workspace_id();
        let title = worktree
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|branch| !branch.is_empty())
            .map(|branch| format!("wt: {branch}"))
            .or_else(|| derive_workspace_name(&cwd))
            .unwrap_or_else(|| format!("Workspace {id}"));
        let color = self.workspace_color_for_new_index(self.workspaces.get_untracked().len());

        self.active_id.set(Some(id));
        self.workspaces.update(|workspaces| {
            workspaces.push(WorkspaceEntry {
                id,
                storage_key: WorkspaceEntry::new_storage_key(),
                title,
                color,
                cwd,
                terminal_count,
                grid_rows,
                grid_cols,
                next_terminal_id: terminal_count as u64 + 1,
                slot_ids,
                slot_agent_labels: vec![String::new()],
                slot_agent_models: Vec::new(),
                slot_agent_efforts: Vec::new(),
                slot_pane_states,
                configuring: false,
                agent_timeline: TimelineDoc::default(),
                agent_compose_draft: String::new(),
                agent_image_mode: false,
                agent_chat_mode: AgentChatMode::AskEdits,
                agent_enhance_prompt_before_send: false,
                architecture_llm_prose: false,
                agent_context_items: Vec::new(),
                agent_chat_sessions: Vec::new(),
                active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
                memory_category_settings: HashMap::new(),
                agent_chat_usage: ChatUsageStats::default(),
                sidebar_explorer_open: true,
                sidebar_graph_open: false,
                sidebar_diff_open: true,
                sidebar_explorer_expanded_paths: Vec::new(),
                center_tabs: default_center_tabs(),
                center_active_tab_id: default_center_active_tab_id(),
                center_next_tab_id: default_center_next_tab_id(),
                remote_connection_id,
                worktree: Some(worktree),
                slot_name_overrides: std::collections::HashMap::new(),
                agent_session_role: None,
                view_mode: WorkspaceViewMode::Grid,
                canvas_view_state: CanvasViewState::default(),
                canvas_edges: Vec::new(),
                canvas_default_transfer_mode: CanvasTransferMode::Structured,
                swarm_view_state: SwarmViewState::default(),
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
        let closed_remote = entry.remote_connection_id.clone();
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
        // Always drop the in-flight wizard draft for this id (cancel_inline_configure
        // also calls us; sidebar close and the new terminals-close path rely on this
        // so a discarded configurator never leaks past the close).
        self.workspace_drafts.update(|m| {
            m.remove(&id);
        });
        self.workspace_config_steps.update(|m| {
            m.remove(&id);
        });
        // Must run after `workspaces.update` — re-entering the same signal inside
        // the closure can deadlock Leptos and freeze close / add workspace.
        self.reset_workspace_id_counter_if_empty();
        // Close the SSH exec channel when the last workspace on that connection
        // is gone (app-exit `kill_all` covers the rest).
        if let Some(cid) = closed_remote {
            let still_used = self.workspaces.with_untracked(|ws| {
                ws.iter()
                    .any(|w| w.remote_connection_id.as_deref() == Some(cid.as_str()))
            });
            if !still_used && is_tauri_shell() {
                spawn_local(async move {
                    let _ = crate::tauri_bridge::remote_exec_close(cid).await;
                });
            }
        }
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
                while workspace.slot_ids.contains(&new_slot_id) {
                    new_slot_id += 1;
                }
                workspace.slot_ids.push(new_slot_id);
                workspace.slot_agent_labels.push(slug.clone());
                workspace.slot_agent_models.push(String::new());
                workspace.slot_agent_efforts.push(String::new());
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
            if index < workspace.slot_agent_models.len() {
                workspace.slot_agent_models.remove(index);
            }
            if index < workspace.slot_agent_efforts.len() {
                workspace.slot_agent_efforts.remove(index);
            }
            workspace.slot_name_overrides.remove(&terminal_id);
            if index < workspace.slot_pane_states.len() {
                workspace.slot_pane_states.remove(index);
            }
            workspace.set_count_and_dims(workspace.slot_agent_labels.len() as u8);
        });
        if let Some(storage_key) = storage_key {
            drop_sessions_for_prefix(format!("{storage_key}:{terminal_id}:"));
        }
    }

    /// All slot ids of a workspace (reactive). Used to compute collision-free
    /// auto names for the terminal naming feature.
    #[must_use]
    pub fn slot_ids_for_workspace(&self, workspace_id: u64) -> Vec<u64> {
        self.workspaces.with(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.slot_ids.clone())
                .unwrap_or_default()
        })
    }

    /// The per-slot friendly-name override, if any (reactive).
    #[must_use]
    pub fn slot_name_override(&self, workspace_id: u64, slot_id: u64) -> Option<String> {
        self.workspaces.with(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.slot_name_overrides.get(&slot_id).cloned())
        })
    }

    /// Set (non-empty) or clear (empty/whitespace) the friendly-name override
    /// for a slot. Persistence rides the workspaces autosave.
    pub fn set_slot_name_override(&self, workspace_id: u64, slot_id: u64, name: String) {
        let trimmed = name.trim().to_string();
        self.workspaces.update(|workspaces| {
            let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            if trimmed.is_empty() {
                workspace.slot_name_overrides.remove(&slot_id);
            } else {
                workspace
                    .slot_name_overrides
                    .insert(slot_id, trimmed.clone());
            }
        });
    }

    /// Clear the friendly-name override for a slot (revert to the auto name).
    pub fn clear_slot_name_override(&self, workspace_id: u64, slot_id: u64) {
        self.set_slot_name_override(workspace_id, slot_id, String::new());
    }

    /// Swaps two terminal slots by id. Used by the EB-parity grid DnD
    /// where dropping slot A on slot B exchanges their grid positions
    /// instead of insert-reordering. Both slots must live in the same
    /// workspace; no-op for unknown ids or `slot_a == slot_b`.
    pub fn swap_terminal_slots(&self, workspace_id: u64, slot_a: u64, slot_b: u64) {
        if slot_a == slot_b {
            return;
        }
        self.workspaces.update(|workspaces| {
            if let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                swap_workspace_slots(workspace, slot_a, slot_b);
            }
        });
    }

    #[cfg_attr(
        not(test),
        expect(
        dead_code,
        reason = "wired by the follow-up terminal drop dispatch task"
        )
    )]
    pub fn move_terminal_slot_into_split(
        &self,
        workspace_id: u64,
        source_slot_id: u64,
        target_slot_id: u64,
        action: TerminalSlotDropAction,
    ) -> Result<TerminalSlotSplitMove, String> {
        let mut result: Result<TerminalSlotSplitMove, String> = Err("not run".into());
        self.workspaces.update(|workspaces| {
            result = workspaces
                .iter_mut()
                .find(|w| w.id == workspace_id)
                .ok_or_else(|| "workspace not found".to_string())
                .and_then(|workspace| {
                    move_workspace_slot_into_split(
                        workspace,
                        source_slot_id,
                        target_slot_id,
                        action,
                    )
                });
        });
        let mv = result?;
        let pair = mv.terminal_key_pair();
        self.begin_terminal_move(&[pair]);
        Ok(mv)
    }

    /// True while the source cell's `terminal_key` is mid-transfer. The
    /// cell's `on_cleanup` consults this to decide whether to `pty_kill`
    /// the underlying session — adopted PTYs must outlive the unmount.
    #[must_use]
    pub fn is_terminal_key_moving(&self, terminal_key: &str) -> bool {
        self.terminal_move_guards
            .with_untracked(|m| m.contains_key(terminal_key))
    }

    /// Consume the adoptable PTY session id registered for `new_key` by
    /// a pending cross-workspace move. Returns `Some(sid)` exactly once;
    /// the target cell's bootstrap uses this to skip a new `pty_spawn`
    /// and bind its xterm directly to the live session. Also clears the
    /// matching `terminal_move_guards` entry so an explicit close on the
    /// adopted cell behaves normally.
    pub fn take_terminal_adopt(&self, new_terminal_key: &str) -> Option<u64> {
        let mut sid: Option<u64> = None;
        self.terminal_adopt_pending.update(|m| {
            sid = m.remove(new_terminal_key);
        });
        if sid.is_some() {
            let new_owned = new_terminal_key.to_string();
            self.terminal_move_guards
                .update(|m| m.retain(|_old, new| new != &new_owned));
        }
        sid
    }

    /// Rewrite live PTY/notification/focus registries for the given
    /// `(old_key, new_key)` pairs, register guards so cleanup of the
    /// source cell does not kill the moved PTY, and queue adopt entries
    /// for the new cell's bootstrap. Also asks the Tauri layer to rewrite
    /// `sessions.json` and `notifications.json` so resume + unread state
    /// follow the slot across workspaces. A safety timeout drops the
    /// guards even if the new cell never mounts.
    fn begin_terminal_move(&self, pairs: &[(String, String)]) {
        if pairs.is_empty() {
            return;
        }
        // Move live PTY session map entries old_key -> new_key.
        let mut adopt_pairs: Vec<(String, u64)> = Vec::with_capacity(pairs.len());
        self.pty_sessions.update(|m| {
            for (old, new) in pairs {
                if let Some(sid) = m.remove(old) {
                    m.insert(new.clone(), sid);
                    adopt_pairs.push((new.clone(), sid));
                }
            }
        });
        // Move notification counts so the badge follows the slot.
        self.notifications.update(|m| {
            for (old, new) in pairs {
                if let Some(count) = m.remove(old) {
                    m.insert(new.clone(), count);
                }
            }
        });
        // Rewrite focused-terminal entries that pointed at the old keys.
        let lookup: HashMap<&String, &String> = pairs.iter().map(|(o, n)| (o, n)).collect();
        self.focused_terminal_by_workspace.update(|m| {
            for v in m.values_mut() {
                if let Some(new) = lookup.get(v) {
                    *v = (*new).clone();
                }
            }
        });
        // Mark old keys as "do not kill PTY on cleanup" + queue adopt sids.
        self.terminal_move_guards.update(|m| {
            for (old, new) in pairs {
                m.insert(old.clone(), new.clone());
            }
        });
        self.terminal_adopt_pending.update(|m| {
            for (new, sid) in &adopt_pairs {
                m.insert(new.clone(), *sid);
            }
        });
        // Tauri-side key rewrite — fire and forget; failure is non-fatal
        // (resume/unread will fall back to the next regular sync).
        #[cfg(target_arch = "wasm32")]
        {
            if is_tauri_shell() {
                let owned: Vec<(String, String)> = pairs.to_vec();
                spawn_local(async move {
                    if let Err(err) =
                        crate::tauri_bridge::workbench_rewrite_terminal_keys(owned).await
                    {
                        leptos::logging::warn!("workbench_rewrite_terminal_keys: {err}");
                    }
                });
            }
        }
        // Drop guards after a brief window so a never-mounted target
        // (e.g. workspace closed mid-transfer) doesn't keep leaking the
        // skip-kill bit forever.
        #[cfg(target_arch = "wasm32")]
        {
            let svc = *self;
            let old_keys: Vec<String> = pairs.iter().map(|(o, _)| o.clone()).collect();
            let new_keys: Vec<String> = pairs.iter().map(|(_, n)| n.clone()).collect();
            spawn_local(async move {
                TimeoutFuture::new(5_000).await;
                svc.terminal_move_guards.update(|m| {
                    for k in &old_keys {
                        m.remove(k);
                    }
                });
                svc.terminal_adopt_pending.update(|m| {
                    for k in &new_keys {
                        m.remove(k);
                    }
                });
            });
        }
    }

    /// Move a terminal slot from one workspace to another. Reuses the
    /// MoveGuard / adopt path so the slot's running PTY (and its agent
    /// CLI inside it) survives the cell remount. Returns the resulting
    /// [`TerminalSlotMove`] so the caller can react (toast, focus, …);
    /// callers don't normally need to do anything else — the state
    /// update + key rewrites are wired internally.
    pub fn transfer_terminal_slot(
        &self,
        from_workspace_id: u64,
        to_workspace_id: u64,
        slot_id: u64,
    ) -> Result<TerminalSlotMove, String> {
        let mut result: Result<TerminalSlotMove, String> = Err("not run".into());
        self.workspaces.update(|workspaces| {
            result =
                transfer_workspace_slot(workspaces, from_workspace_id, to_workspace_id, slot_id);
        });
        let mv = result?;
        // Set up adoption guards AFTER the state mutation: the source
        // cell's `on_cleanup` runs on the next render (Leptos batches),
        // which is after this synchronous block — the guard will be in
        // place by then and pty_kill will be skipped.
        let pairs = mv.terminal_key_pairs();
        self.begin_terminal_move(&pairs);
        // Activate the target workspace so the user sees the slot land
        // (mirrors EB's `extractSlotToNewWorkspace` behaviour).
        self.select_workspace(to_workspace_id);
        Ok(mv)
    }

    /// Extract a slot into a brand-new workspace. Allocates a fresh
    /// workspace entry that inherits the source workspace's cwd, then
    /// delegates to [`Self::transfer_terminal_slot`] so the PTY-adopt
    /// path is identical to a cross-workspace drop on an existing row.
    pub fn extract_terminal_slot_to_new_workspace(
        &self,
        workspace_id: u64,
        slot_id: u64,
    ) -> Result<TerminalSlotMove, String> {
        let (cwd, color_seed) =
            self.workspaces
                .with_untracked(|list| -> Result<(String, usize), String> {
                    let source = list
                        .iter()
                        .find(|w| w.id == workspace_id)
                        .ok_or_else(|| "source workspace not found".to_string())?;
                    if source.slot_ids.len() <= 1 {
                        return Err("source workspace must keep at least one slot".into());
                    }
                    if !source.slot_ids.contains(&slot_id) {
                        return Err("slot not found in source workspace".into());
                    }
                    Ok((source.cwd.clone(), list.len()))
                })?;
        let new_id = self.allocate_workspace_id();
        let title = format!("Workspace {new_id}");
        let color = self.workspace_color_for_new_index(color_seed);
        self.workspaces.update(|workspaces| {
            workspaces.push(WorkspaceEntry {
                id: new_id,
                storage_key: WorkspaceEntry::new_storage_key(),
                title,
                color,
                cwd: cwd.clone(),
                terminal_count: 0,
                grid_rows: 1,
                grid_cols: 1,
                next_terminal_id: 1,
                slot_ids: Vec::new(),
                slot_agent_labels: Vec::new(),
                slot_agent_models: Vec::new(),
                slot_agent_efforts: Vec::new(),
                slot_pane_states: Vec::new(),
                configuring: false,
                agent_timeline: TimelineDoc::default(),
                agent_compose_draft: String::new(),
                agent_image_mode: false,
                agent_chat_mode: AgentChatMode::AskEdits,
                agent_enhance_prompt_before_send: false,
                architecture_llm_prose: false,
                agent_context_items: Vec::new(),
                agent_chat_sessions: Vec::new(),
                active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
                memory_category_settings: HashMap::new(),
                agent_chat_usage: ChatUsageStats::default(),
                sidebar_explorer_open: true,
                sidebar_graph_open: false,
                sidebar_diff_open: true,
                sidebar_explorer_expanded_paths: Vec::new(),
                center_tabs: default_center_tabs(),
                center_active_tab_id: default_center_active_tab_id(),
                center_next_tab_id: default_center_next_tab_id(),
                remote_connection_id: None,
                worktree: None,
                slot_name_overrides: std::collections::HashMap::new(),
                agent_session_role: None,
                view_mode: WorkspaceViewMode::Grid,
                canvas_view_state: CanvasViewState::default(),
                canvas_edges: Vec::new(),
                canvas_default_transfer_mode: CanvasTransferMode::Structured,
                swarm_view_state: SwarmViewState::default(),
            });
        });
        match self.transfer_terminal_slot(workspace_id, new_id, slot_id) {
            Ok(mv) => Ok(mv),
            Err(err) => {
                // Roll the empty workspace back so a failed extract
                // doesn't litter the sidebar with empty placeholders.
                self.workspaces
                    .update(|list| list.retain(|w| w.id != new_id));
                self.reset_workspace_id_counter_if_empty();
                Err(err)
            }
        }
    }

    #[must_use]
    pub fn sidebar_collapsed(&self) -> RwSignal<bool> {
        self.sidebar_collapsed
    }

    pub fn sidebar_width_px(&self) -> RwSignal<f64> {
        self.sidebar_width_px
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

    #[must_use]
    pub fn default_project_dir(&self) -> RwSignal<String> {
        self.default_project_dir
    }

    pub fn set_default_project_dir_text(&self, path: String) {
        self.default_project_dir.set(path);
    }

    pub fn persist_default_project_dir(&self, path: String) {
        write_local_storage(DEFAULT_PROJECT_DIR_KEY, &path);
        self.default_project_dir.set(path);
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

    #[must_use]
    pub fn workspace_is_configuring(&self, id: u64) -> bool {
        self.workspaces
            .with_untracked(|w| w.iter().any(|ws| ws.id == id && ws.configuring))
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

        let project_root = self.default_project_dir.get_untracked();
        let draft = CreateWorkspaceDraft {
            cwd_display: if project_root.trim().is_empty() {
                self.harness_workspace_root.get_untracked()
            } else {
                project_root
            },
            session_role: self.default_session_role.get_untracked(),
            ..CreateWorkspaceDraft::default()
        };

        let color = self.workspace_color_for_new_index(self.workspaces.get_untracked().len());
        let entry = WorkspaceEntry {
            id,
            storage_key: WorkspaceEntry::new_storage_key(),
            title: format!("Workspace {id}"),
            color,
            cwd: String::new(),
            terminal_count: 1,
            grid_rows: 1,
            grid_cols: 1,
            next_terminal_id: 1,
            slot_ids: Vec::new(),
            slot_agent_labels: Vec::new(),
            slot_agent_models: Vec::new(),
            slot_agent_efforts: Vec::new(),
            slot_pane_states: Vec::new(),
            configuring: true,
            agent_timeline: TimelineDoc::default(),
            agent_compose_draft: String::new(),
            agent_image_mode: false,
            agent_chat_mode: AgentChatMode::AskEdits,
            agent_enhance_prompt_before_send: false,
            architecture_llm_prose: false,
            agent_context_items: Vec::new(),
            agent_chat_sessions: Vec::new(),
            active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
            memory_category_settings: HashMap::new(),
            agent_chat_usage: ChatUsageStats::default(),
            sidebar_explorer_open: true,
            sidebar_graph_open: false,
            sidebar_diff_open: true,
            sidebar_explorer_expanded_paths: Vec::new(),
            center_tabs: default_center_tabs(),
            center_active_tab_id: default_center_active_tab_id(),
            center_next_tab_id: default_center_next_tab_id(),
            remote_connection_id: None,
            worktree: None,
            slot_name_overrides: std::collections::HashMap::new(),
            agent_session_role: None,
            view_mode: WorkspaceViewMode::Grid,
            canvas_view_state: CanvasViewState::default(),
            canvas_edges: Vec::new(),
            canvas_default_transfer_mode: CanvasTransferMode::Structured,
            swarm_view_state: SwarmViewState::default(),
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
        // Draft + config-step cleanup is centralised in
        // `finalize_workspace_close` so every close path drops the draft.
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
        if d.workspace_kind == WorkspaceDraftKind::Worktree {
            if d.cwd_display.trim().is_empty() || d.worktree_branch.trim().is_empty() {
                return Err(());
            }
            self.set_workspace_config_step(id, 1);
            return Ok(());
        }
        // Remote workspaces may proceed without a local cwd (the remote start
        // directory is optional and comes from the connection preset).
        if d.remote_connection_id.is_none() && d.cwd_display.trim().is_empty() {
            return Err(());
        }
        self.set_workspace_config_step(id, 1);
        Ok(())
    }

    /// Select (or clear) the SSH remote connection for a workspace draft.
    pub fn set_workspace_remote_connection(&self, id: u64, connection_id: Option<String>) {
        self.update_workspace_draft(id, |d| d.remote_connection_id = connection_id);
    }

    /// Set (or clear) the harness session-role slug for a workspace draft.
    pub fn set_workspace_session_role(&self, id: u64, slug: Option<String>) {
        let slug = slug.filter(|s| !s.trim().is_empty());
        self.update_workspace_draft(id, |d| d.session_role = slug);
    }

    /// Set a per-slot friendly name in the draft (grows the vec as needed).
    pub fn set_workspace_slot_name(&self, id: u64, slot_index: usize, name: String) {
        self.update_workspace_draft(id, |d| {
            if d.slot_names.len() <= slot_index {
                d.slot_names.resize(slot_index + 1, String::new());
            }
            d.slot_names[slot_index] = name;
        });
    }

    /// Set the CLI model id for an agent row (0..5) in the draft.
    pub fn set_workspace_agent_model(&self, id: u64, idx: usize, model: String) {
        if idx >= 5 {
            return;
        }
        self.update_workspace_draft(id, |d| d.agent_models[idx] = model);
    }

    /// Set the CLI reasoning effort for an agent row (0..5) in the draft.
    pub fn set_workspace_agent_effort(&self, id: u64, idx: usize, effort: String) {
        if idx >= 5 {
            return;
        }
        self.update_workspace_draft(id, |d| d.agent_efforts[idx] = effort);
    }

    /// Apply a saved preset onto a workspace draft: terminal count + grid,
    /// per-agent counts, per-agent models, per-slot names, and session role.
    /// Clears the `agents_skipped` flag so the fleet step reflects the preset.
    pub fn apply_preset_to_draft(
        &self,
        id: u64,
        terminal_count: u8,
        agent_counts: [u8; 5],
        agent_models: [String; 5],
        agent_efforts: [String; 5],
        slot_names: Vec<String>,
        session_role: Option<String>,
    ) {
        let count = terminal_count.clamp(1, 16);
        let (r, c) = WorkspaceEntry::grid_dims_for_count(count);
        let session_role = session_role.filter(|s| !s.trim().is_empty());
        self.update_workspace_draft(id, |d| {
            d.terminal_count = count;
            d.grid_rows = r;
            d.grid_cols = c;
            d.agent_counts = agent_counts;
            d.agent_models = agent_models;
            d.agent_efforts = agent_efforts;
            d.agents_skipped = false;
            d.slot_names = slot_names;
            d.session_role = session_role;
        });
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
        let remote_connection_id = draft.remote_connection_id.clone();
        let worktree = worktree_meta_from_draft(&draft);
        // Local workspaces require a working directory; remote ones may omit it
        // (the remote start dir is resolved from the connection preset).
        if cwd.is_empty() && remote_connection_id.is_none() {
            return;
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
        let slot_agent_models =
            fleet_slot_models_for_labels(&slot_agent_labels, &draft.agent_models);
        let slot_agent_efforts =
            fleet_slot_efforts_for_labels(&slot_agent_labels, &draft.agent_efforts);

        let title = workspace_title_from_name_or_cwd(id, &draft.name_input, &cwd);

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
            // Wizard-commit upgrades a freshly-created (empty center_tabs)
            // workspace into a real one — make sure the Terminals tab is
            // present, but never overwrite a Settings/File tab the user
            // already opened in the meantime.
            if !ws
                .center_tabs
                .iter()
                .any(|tab| matches!(tab.kind, CenterTabKind::Terminals))
            {
                ws.center_tabs.insert(0, CenterTab::terminals());
            }
            ws.center_active_tab_id = CENTER_TERMINALS_TAB_ID;
            repair_center_tab_state(ws);
            ws.title = title;
            if ws.color.trim().is_empty() {
                ws.color = self.workspace_color_for_new_index(0);
            }
            ws.cwd = cwd;
            ws.terminal_count = draft.terminal_count;
            ws.grid_rows = gr;
            ws.grid_cols = gc;
            ws.slot_ids = slot_ids;
            ws.slot_agent_labels = slot_agent_labels;
            ws.slot_agent_models = slot_agent_models;
            ws.slot_agent_efforts = slot_agent_efforts;
            ws.slot_pane_states = slot_pane_states;
            ws.next_terminal_id = n as u64 + 1;
            ws.remote_connection_id = remote_connection_id.clone();
            ws.worktree = worktree.clone();
            ws.agent_session_role = draft.session_role.clone();
            // Seed per-slot name overrides from the draft (index → slot_id).
            ws.slot_name_overrides.clear();
            for (i, name) in draft.slot_names.iter().enumerate() {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(slot_id) = ws.slot_ids.get(i).copied() {
                    ws.slot_name_overrides.insert(slot_id, trimmed.to_string());
                }
            }
            ws.configuring = false;
        });

        self.workspace_drafts.update(|m| {
            m.remove(&id);
        });
        self.workspace_config_steps.update(|m| {
            m.remove(&id);
        });
        self.bump_terminal_layout();
        self.bump_sidebar_repo_epoch();
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
            let fallback = workspace.slot_agent_state_at(idx);
            if let Some(slot) = workspace.slot_pane_states.get_mut(idx) {
                let mut state = state;
                state.normalize_pane_agents(&fallback);
                if *slot != state {
                    *slot = state;
                }
            }
        });
    }

    pub fn set_workspace_agent_timeline(&self, workspace_id: u64, items: TimelineDoc) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session.timeline = items;
                    session.updated_at = js_sys::Date::now();
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    pub fn set_workspace_agent_session_timeline(
        &self,
        workspace_id: u64,
        session_id: &str,
        items: TimelineDoc,
    ) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.agent_chat_session_mut(session_id) {
                    session.timeline = items;
                    session.updated_at = js_sys::Date::now();
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    /// Read the live chat-usage aggregate for a workspace.
    #[must_use]
    pub fn chat_usage_for_workspace(&self, workspace_id: u64) -> ChatUsageStats {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.usage.clone())
                .unwrap_or_default()
        })
    }

    /// Accumulate one `TurnUsage` event (ModelRound or ToolExec, from main
    /// agent or a subagent) into the workspace's running totals.
    ///
    /// Returns `true` when the event was applied. Events whose
    /// `turn_generation` is below the workspace's `current_turn_generation`
    /// are dropped — they belong to a turn that was cancelled by a chat
    /// reset and would otherwise pollute the fresh chat's totals.
    pub fn record_chat_turn_usage(
        &self,
        workspace_id: u64,
        turn_generation: u64,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        elapsed_ms: u64,
        cost_usd: Option<f64>,
        // Some(tokens) only for a **main-agent `ModelRound`** event — the
        // caller decides this from `kind`/`agent_id`. Overwrites the live
        // context-window occupancy with the newest round's prompt size.
        round_input_tokens: Option<u64>,
    ) -> bool {
        let mut applied = false;
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                let timeline_started_at = ws
                    .active_agent_chat_session()
                    .and_then(|session| session.timeline.turns.first())
                    .and_then(|turn| turn.user.created_at);
                let single_untimed_turn = ws
                    .active_agent_chat_session()
                    .map(|session| session.timeline.turns.len() <= 1)
                    .unwrap_or(true);
                let Some(session) = ws.active_agent_chat_session_mut() else {
                    return;
                };
                let u = &mut session.usage;
                if turn_generation < u.current_turn_generation {
                    return;
                }
                if turn_generation > u.current_turn_generation {
                    u.current_turn_generation = turn_generation;
                }
                if u.turn_count == 0 && u.session_started_at.is_none() {
                    u.session_started_at = timeline_started_at.or_else(|| {
                        if single_untimed_turn {
                            Some(js_sys::Date::now())
                        } else {
                            None
                        }
                    });
                }
                u.turn_count = u.turn_count.saturating_add(1);
                if let Some(p) = input_tokens {
                    u.total_input_tokens = u.total_input_tokens.saturating_add(p);
                }
                if let Some(c) = output_tokens {
                    u.total_output_tokens = u.total_output_tokens.saturating_add(c);
                }
                u.total_elapsed_ms = u.total_elapsed_ms.saturating_add(elapsed_ms);
                if let Some(c) = cost_usd {
                    u.total_cost_usd += c;
                }
                if let Some(t) = round_input_tokens {
                    u.last_round_input_tokens = t;
                }
                applied = true;
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
        applied
    }

    pub fn record_chat_turn_usage_for_session(
        &self,
        workspace_id: u64,
        session_id: &str,
        turn_generation: u64,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        elapsed_ms: u64,
        cost_usd: Option<f64>,
        round_input_tokens: Option<u64>,
    ) -> bool {
        let mut applied = false;
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                let timeline_started_at = ws
                    .agent_chat_session(session_id)
                    .and_then(|session| session.timeline.turns.first())
                    .and_then(|turn| turn.user.created_at);
                let single_untimed_turn = ws
                    .agent_chat_session(session_id)
                    .map(|session| session.timeline.turns.len() <= 1)
                    .unwrap_or(true);
                let Some(session) = ws.agent_chat_session_mut(session_id) else {
                    return;
                };
                let u = &mut session.usage;
                if turn_generation < u.current_turn_generation {
                    return;
                }
                if turn_generation > u.current_turn_generation {
                    u.current_turn_generation = turn_generation;
                }
                if u.turn_count == 0 && u.session_started_at.is_none() {
                    u.session_started_at = timeline_started_at.or_else(|| {
                        if single_untimed_turn {
                            Some(js_sys::Date::now())
                        } else {
                            None
                        }
                    });
                }
                u.turn_count = u.turn_count.saturating_add(1);
                if let Some(p) = input_tokens {
                    u.total_input_tokens = u.total_input_tokens.saturating_add(p);
                }
                if let Some(c) = output_tokens {
                    u.total_output_tokens = u.total_output_tokens.saturating_add(c);
                }
                u.total_elapsed_ms = u.total_elapsed_ms.saturating_add(elapsed_ms);
                if let Some(c) = cost_usd {
                    u.total_cost_usd += c;
                }
                if let Some(t) = round_input_tokens {
                    u.last_round_input_tokens = t;
                }
                applied = true;
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
        applied
    }

    /// Mark the current chat session as started without crediting a usage
    /// event. Called immediately when the user submits a turn so the Agent
    /// stats header updates before the first backend `TurnUsage` event lands.
    pub fn ensure_chat_session_started_for_session(
        &self,
        workspace_id: u64,
        session_id: &str,
        started_at: f64,
    ) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.agent_chat_session_mut(session_id) {
                    if session.usage.session_started_at.is_none() {
                        session.usage.session_started_at = Some(started_at);
                    }
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    /// Overwrite the live context-window occupancy directly. Called after a
    /// compaction replaces the conversation with a much smaller summary, so
    /// the meter reflects the new (estimated) prompt size immediately rather
    /// than waiting for the next real turn's `TurnUsage`.
    pub fn set_session_last_round_input_tokens(
        &self,
        workspace_id: u64,
        session_id: &str,
        tokens: u64,
    ) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.agent_chat_session_mut(session_id) {
                    session.usage.last_round_input_tokens = tokens;
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    /// Reset the chat-usage aggregate (call alongside `agent_clear_conversation`).
    /// Bumps `current_turn_generation` past whatever it was before so any
    /// in-flight `TurnUsage` events from the cancelled turn are dropped
    /// when they arrive.
    pub fn clear_chat_usage_for_session(&self, workspace_id: u64, session_id: &str) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.agent_chat_session_mut(session_id) {
                    let next_gen = session.usage.current_turn_generation.saturating_add(1);
                    session.usage = ChatUsageStats {
                        current_turn_generation: next_gen,
                        ..ChatUsageStats::default()
                    };
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    pub fn reset_workspace_agent_chat_mode(&self, workspace_id: u64) {
        self.set_workspace_agent_chat_mode(workspace_id, AgentChatMode::AskEdits);
    }

    #[must_use]
    pub fn agent_timeline_for_workspace_untracked(&self, workspace_id: u64) -> TimelineDoc {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.timeline.clone())
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn agent_compose_draft_for_workspace_untracked(&self, workspace_id: u64) -> String {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.draft.clone())
                .unwrap_or_default()
        })
    }

    pub fn set_workspace_agent_compose_draft(&self, workspace_id: u64, draft: String) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session.draft = draft;
                    session.updated_at = js_sys::Date::now();
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    #[must_use]
    pub fn agent_image_mode_for_workspace_untracked(&self, workspace_id: u64) -> bool {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.image_mode)
                .unwrap_or(false)
        })
    }

    /// Active harness session-role slug for a workspace (committed entry), or
    /// `None`. Used to populate `UserTurn.session_role` at submit time and the
    /// agent name-badge role sub-line.
    #[must_use]
    pub fn agent_session_role_for_workspace_untracked(&self, workspace_id: u64) -> Option<String> {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.agent_session_role.clone())
        })
    }

    pub fn set_workspace_agent_image_mode(&self, workspace_id: u64, image_mode: bool) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session.image_mode = image_mode;
                    session.updated_at = js_sys::Date::now();
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    #[must_use]
    pub fn agent_chat_mode_for_workspace_untracked(&self, workspace_id: u64) -> AgentChatMode {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.chat_mode)
                .unwrap_or_default()
        })
    }

    pub fn set_workspace_agent_chat_mode(&self, workspace_id: u64, mode: AgentChatMode) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session.chat_mode = mode;
                    session.updated_at = js_sys::Date::now();
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    #[must_use]
    pub fn agent_enhance_prompt_for_workspace_untracked(&self, workspace_id: u64) -> bool {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.enhance_prompt_before_send)
                .unwrap_or(false)
        })
    }

    pub fn set_workspace_agent_enhance_prompt(&self, workspace_id: u64, enabled: bool) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session.enhance_prompt_before_send = enabled;
                    session.updated_at = js_sys::Date::now();
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    #[must_use]
    pub fn agent_chat_sessions_for_workspace(&self, workspace_id: u64) -> Vec<AgentChatSession> {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| {
                    let mut w = w.clone();
                    w.ensure_agent_chat_sessions();
                    w.agent_chat_sessions
                })
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn active_agent_chat_session_id_for_workspace(&self, workspace_id: u64) -> String {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| {
                    let mut w = w.clone();
                    w.ensure_agent_chat_sessions();
                    w.active_agent_chat_session_id
                })
                .unwrap_or_else(|| DEFAULT_AGENT_CHAT_SESSION_ID.to_string())
        })
    }

    pub fn create_agent_chat_session(&self, workspace_id: u64) -> Option<String> {
        let mut created = None;
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ws.ensure_agent_chat_sessions();
            let now = js_sys::Date::now();
            let session = AgentChatSession::fresh(ws.agent_chat_sessions.len() + 1, now);
            created = Some(session.id.clone());
            ws.active_agent_chat_session_id = session.id.clone();
            ws.agent_chat_sessions.push(session);
            ws.sync_legacy_agent_chat_fields_from_active();
        });
        created
    }

    pub fn select_agent_chat_session(&self, workspace_id: u64, session_id: &str) -> bool {
        let mut selected = false;
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ws.ensure_agent_chat_sessions();
            if ws.agent_chat_sessions.iter().any(|s| s.id == session_id) {
                ws.active_agent_chat_session_id = session_id.to_string();
                if let Some(session) = ws.agent_chat_session_mut(session_id) {
                    session.unread_count = 0;
                }
                ws.sync_legacy_agent_chat_fields_from_active();
                selected = true;
            }
        });
        selected
    }

    pub fn close_agent_chat_session(&self, workspace_id: u64, session_id: &str) -> Result<(), ()> {
        let mut closed = false;
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ws.ensure_agent_chat_sessions();
            if ws.agent_chat_sessions.len() <= 1 {
                return;
            }
            let Some(idx) = ws
                .agent_chat_sessions
                .iter()
                .position(|session| session.id == session_id)
            else {
                return;
            };
            if matches!(
                ws.agent_chat_sessions[idx].status,
                AgentChatSessionStatus::Running | AgentChatSessionStatus::NeedsInput
            ) {
                return;
            }
            ws.agent_chat_sessions.remove(idx);
            if ws.active_agent_chat_session_id == session_id {
                ws.active_agent_chat_session_id = ws
                    .agent_chat_sessions
                    .get(idx.saturating_sub(1))
                    .or_else(|| ws.agent_chat_sessions.first())
                    .map(|session| session.id.clone())
                    .unwrap_or_else(|| DEFAULT_AGENT_CHAT_SESSION_ID.to_string());
            }
            ws.sync_legacy_agent_chat_fields_from_active();
            closed = true;
        });
        if closed {
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn set_agent_chat_session_status(
        &self,
        workspace_id: u64,
        session_id: &str,
        status: AgentChatSessionStatus,
    ) {
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ws.ensure_agent_chat_sessions();
            let is_active = ws.active_agent_chat_session_id == session_id;
            if let Some(session) = ws.agent_chat_session_mut(session_id) {
                session.status = status;
                session.updated_at = js_sys::Date::now();
                if !is_active {
                    session.unread_count = session.unread_count.saturating_add(1);
                }
            }
            ws.sync_legacy_agent_chat_fields_from_active();
        });
    }

    pub fn set_agent_chat_session_title_if_auto(
        &self,
        workspace_id: u64,
        session_id: &str,
        title: String,
        previous_auto_title: Option<&str>,
    ) -> bool {
        let mut updated = false;
        let title = title.trim().to_string();
        if title.is_empty() {
            return false;
        }
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ws.ensure_agent_chat_sessions();
            if let Some(session) = ws.agent_chat_session_mut(session_id) {
                let current = session.title.trim();
                let may_replace = is_default_agent_chat_title(current)
                    || previous_auto_title
                        .map(|expected| current == expected.trim())
                        .unwrap_or(false);
                if may_replace && current != title {
                    session.title = title;
                    session.updated_at = js_sys::Date::now();
                    updated = true;
                }
            }
            ws.sync_legacy_agent_chat_fields_from_active();
        });
        updated
    }

    #[must_use]
    pub fn architecture_llm_prose_for_workspace_untracked(&self, workspace_id: u64) -> bool {
        self.workspaces.with_untracked(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.architecture_llm_prose)
                .unwrap_or(false)
        })
    }

    #[must_use]
    pub fn architecture_llm_prose_for_workspace(&self, workspace_id: u64) -> bool {
        self.workspaces.with(|workspaces| {
            workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.architecture_llm_prose)
                .unwrap_or(false)
        })
    }

    pub fn set_workspace_architecture_llm_prose(&self, workspace_id: u64, enabled: bool) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.architecture_llm_prose = enabled;
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
                .and_then(|w| w.active_agent_chat_session())
                .map(|session| session.pending_context_items.clone())
                .unwrap_or_default()
        })
    }

    pub fn upsert_workspace_agent_context(&self, workspace_id: u64, item: AgentContextItem) {
        self.workspaces.update(|workspaces| {
            let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) else {
                return;
            };
            ws.ensure_agent_chat_sessions();
            if let Some(session) = ws.active_agent_chat_session_mut() {
                if let Some(existing) = session
                    .pending_context_items
                    .iter_mut()
                    .find(|it| it.id == item.id)
                {
                    *existing = item;
                } else {
                    session.pending_context_items.push(item);
                }
            }
            ws.sync_legacy_agent_chat_fields_from_active();
        });
    }

    pub fn remove_workspace_agent_context(&self, workspace_id: u64, item_id: &str) {
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session
                        .pending_context_items
                        .retain(|item| item.id != item_id);
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    pub fn remove_workspace_agent_context_items(&self, workspace_id: u64, item_ids: &[String]) {
        if item_ids.is_empty() {
            return;
        }
        let ids: HashSet<&str> = item_ids.iter().map(String::as_str).collect();
        self.workspaces.update(|workspaces| {
            if let Some(ws) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
                ws.ensure_agent_chat_sessions();
                if let Some(session) = ws.active_agent_chat_session_mut() {
                    session
                        .pending_context_items
                        .retain(|item| !ids.contains(item.id.as_str()));
                }
                ws.sync_legacy_agent_chat_fields_from_active();
            }
        });
    }

    #[must_use]
    pub fn agent_images_for_workspace_untracked(
        &self,
        workspace_id: u64,
    ) -> Vec<WorkspaceAgentImage> {
        self.agent_image_context
            .with_untracked(|by_ws| by_ws.get(&workspace_id).cloned().unwrap_or_default())
    }

    #[must_use]
    pub fn pending_agent_images_for_workspace_untracked(
        &self,
        workspace_id: u64,
    ) -> Vec<AgentImageContextItem> {
        self.agent_image_context.with_untracked(|by_ws| {
            by_ws
                .get(&workspace_id)
                .into_iter()
                .flat_map(|items| items.iter())
                .filter(|image| image.status == AgentImageContextStatus::Pending)
                .map(|image| image.item.clone())
                .collect()
        })
    }

    #[must_use]
    pub fn active_agent_images(&self) -> Vec<WorkspaceAgentImage> {
        let Some(ws_id) = self.active_id.get() else {
            return Vec::new();
        };
        self.agent_image_context
            .with(|by_ws| by_ws.get(&ws_id).cloned().unwrap_or_default())
    }

    #[must_use]
    pub fn active_agent_image_count(&self) -> usize {
        let Some(ws_id) = self.active_id.get() else {
            return 0;
        };
        self.agent_image_context
            .with(|by_ws| by_ws.get(&ws_id).map(Vec::len).unwrap_or(0))
    }

    pub fn upsert_workspace_agent_image(&self, workspace_id: u64, item: AgentImageContextItem) {
        self.agent_image_context.update(|by_ws| {
            let images = by_ws.entry(workspace_id).or_default();
            if let Some(existing) = images.iter_mut().find(|image| image.item.id == item.id) {
                existing.item = item;
                existing.status = AgentImageContextStatus::Pending;
            } else {
                images.push(WorkspaceAgentImage {
                    item,
                    status: AgentImageContextStatus::Pending,
                });
            }
        });
    }

    pub fn remove_workspace_agent_image(&self, workspace_id: u64, image_id: &str) {
        self.agent_image_context.update(|by_ws| {
            if let Some(images) = by_ws.get_mut(&workspace_id) {
                images.retain(|image| image.item.id != image_id);
            }
        });
    }

    pub fn reactivate_workspace_agent_image(&self, workspace_id: u64, image_id: &str) {
        self.agent_image_context.update(|by_ws| {
            if let Some(images) = by_ws.get_mut(&workspace_id) {
                if let Some(image) = images.iter_mut().find(|image| image.item.id == image_id) {
                    image.status = AgentImageContextStatus::Pending;
                }
            }
        });
    }

    pub fn mark_workspace_agent_images_read(&self, workspace_id: u64, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        self.agent_image_context.update(|by_ws| {
            if let Some(images) = by_ws.get_mut(&workspace_id) {
                for image in images {
                    if ids.contains(image.item.id.as_str()) {
                        image.status = AgentImageContextStatus::Read;
                    }
                }
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
    pub fn workspace_color_for_new_index(&self, index: usize) -> String {
        workspace_color_from_presets(&self.memory_color_presets.get_untracked(), index)
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
            sidebar_width_px: self.sidebar_width_px.get_untracked(),
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
        // Snapshot hydration is a process/startup boundary. These maps are
        // live runtime coordination only; carrying them across hydration can
        // leave terminals hidden behind stale "popped out" placeholders or
        // trying to adopt PTYs that no longer exist.
        self.pty_sessions.update(|m| m.clear());
        self.terminal_move_guards.update(|m| m.clear());
        self.terminal_adopt_pending.update(|m| m.clear());
        self.terminal_popouts.update(|m| m.clear());

        let color_presets = self.memory_color_presets.get_untracked();
        let max_snap_workspace_id = snap.workspaces.iter().map(|w| w.id).max().unwrap_or(0);
        let workspaces: Vec<WorkspaceEntry> = snap
            .workspaces
            .into_iter()
            .filter(|w| workspace_entry_has_folder(w))
            .enumerate()
            .map(|(idx, mut w)| {
                // Safety net for callers that did not run
                // `WorkbenchSnapshot::backfill_storage_keys` before hydrate.
                // The normal Tauri startup path backfills first so matching
                // sessions can be migrated before terminal cells mount.
                if w.storage_key.trim().is_empty() {
                    w.storage_key = WorkspaceEntry::new_storage_key();
                }
                let fallback = workspace_color_from_presets(&color_presets, idx);
                w.color = normalize_hex_color(&w.color, &fallback);
                // Older snapshots may have lost the Terminals tab via a
                // bug; ensure persisted workspaces (filtered to those with
                // a folder) still have one so the grid renders.
                if !w
                    .center_tabs
                    .iter()
                    .any(|tab| matches!(tab.kind, CenterTabKind::Terminals))
                {
                    w.center_tabs.insert(0, CenterTab::terminals());
                }
                repair_center_tab_state(&mut w);
                w.ensure_agent_chat_sessions();
                w
            })
            .collect();
        let recent_workspaces: Vec<RecentWorkspaceItem> = snap
            .recent_workspaces
            .into_iter()
            .filter(|r| workspace_entry_has_folder(&r.workspace))
            .enumerate()
            .map(|(idx, mut r)| {
                if r.workspace.storage_key.trim().is_empty() {
                    r.workspace.storage_key = WorkspaceEntry::new_storage_key();
                }
                let fallback = workspace_color_from_presets(&color_presets, idx);
                r.workspace.color = normalize_hex_color(&r.workspace.color, &fallback);
                r.workspace.ensure_agent_chat_sessions();
                r
            })
            .collect();
        // Workspace list + selection. Re-seed inline drafts for any
        // workspace persisted in configuring state — drafts are transient
        // and not serialised, so without this the configurator would
        // render against an empty draft.
        let project_root = {
            let p = self.default_project_dir.get_untracked();
            if p.trim().is_empty() {
                self.harness_workspace_root.get_untracked()
            } else {
                p
            }
        };
        self.workspace_drafts.update(|m| {
            m.clear();
            for ws in &workspaces {
                if ws.configuring {
                    let mut d = CreateWorkspaceDraft::default();
                    if !project_root.is_empty() {
                        d.cwd_display.clone_from(&project_root);
                    }
                    d.session_role = self.default_session_role.get_untracked();
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
        if snap.sidebar_width_px.is_finite() && snap.sidebar_width_px >= 120.0 {
            self.sidebar_width_px.set(snap.sidebar_width_px);
        }
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

fn is_default_agent_chat_title(title: &str) -> bool {
    let Some(rest) = title.trim().strip_prefix("Chat ") else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit())
}

fn settings_tab_title(_cat: HarnessSettingsCategory) -> &'static str {
    "Settings"
}

fn file_tab_title(rel_path: &str) -> String {
    rel_path
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(rel_path)
        .to_string()
}

fn diff_tab_title(rel_path: &str) -> String {
    let base = rel_path
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(rel_path);
    format!("{base} (diff)")
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
    #[serde(default = "default_sidebar_width_px")]
    pub sidebar_width_px: f64,
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
    let last = trimmed.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if last.is_empty() || last == "." || last == ".." {
        return None;
    }
    Some(last.to_string())
}

fn workspace_title_from_name_or_cwd(id: u64, name_input: &str, cwd: &str) -> String {
    let explicit = name_input.trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    derive_workspace_name(cwd).unwrap_or_else(|| format!("Workspace {id}"))
}

fn worktree_meta_from_draft(draft: &CreateWorkspaceDraft) -> Option<WorkspaceWorktreeMeta> {
    if draft.workspace_kind != WorkspaceDraftKind::Worktree {
        return None;
    }
    let worktree_cwd = draft.cwd_display.trim().to_string();
    if worktree_cwd.is_empty() {
        return None;
    }
    Some(WorkspaceWorktreeMeta {
        base_cwd: draft.worktree_base_cwd.trim().to_string(),
        worktree_cwd,
        branch: draft.worktree_branch.trim().to_string().into_non_empty(),
        head: None,
        git_common_dir: None,
        main_worktree_cwd: None,
        created_by_blxcode: true,
    })
}

trait IntoNonEmptyString {
    fn into_non_empty(self) -> Option<String>;
}

impl IntoNonEmptyString for String {
    fn into_non_empty(self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

#[cfg(test)]
mod workspace_title_tests {
    use super::*;

    #[test]
    fn blank_name_uses_selected_directory_name() {
        assert_eq!(
            workspace_title_from_name_or_cwd(7, "", "/home/iptoux/Development/blxcode"),
            "blxcode"
        );
    }

    #[test]
    fn explicit_name_wins_over_directory_name() {
        assert_eq!(
            workspace_title_from_name_or_cwd(7, " backend refactor ", "/home/iptoux/blxcode"),
            "backend refactor"
        );
    }

    #[test]
    fn invalid_directory_falls_back_to_workspace_id() {
        assert_eq!(workspace_title_from_name_or_cwd(7, "", "/"), "Workspace 7");
    }
}

/// Pure swap of two slots by id within a single workspace. Returns
/// `true` when the swap actually happened. Normalises `slot_pane_states`
/// / `slot_agent_labels` lengths on the way in so callers don't have to
/// guard against snapshots written before those fields existed.
fn swap_workspace_slots(workspace: &mut WorkspaceEntry, slot_a: u64, slot_b: u64) -> bool {
    if slot_a == slot_b {
        return false;
    }
    let Some(idx_a) = workspace.slot_ids.iter().position(|id| *id == slot_a) else {
        return false;
    };
    let Some(idx_b) = workspace.slot_ids.iter().position(|id| *id == slot_b) else {
        return false;
    };
    while workspace.slot_pane_states.len() < workspace.slot_ids.len() {
        let sid = workspace.slot_ids[workspace.slot_pane_states.len()];
        workspace
            .slot_pane_states
            .push(SlotPaneState::default_for_slot(sid));
    }
    while workspace.slot_agent_labels.len() < workspace.slot_ids.len() {
        workspace.slot_agent_labels.push(String::new());
    }
    while workspace.slot_agent_models.len() < workspace.slot_ids.len() {
        workspace.slot_agent_models.push(String::new());
    }
    while workspace.slot_agent_efforts.len() < workspace.slot_ids.len() {
        workspace.slot_agent_efforts.push(String::new());
    }
    workspace.slot_ids.swap(idx_a, idx_b);
    workspace.slot_agent_labels.swap(idx_a, idx_b);
    workspace.slot_agent_models.swap(idx_a, idx_b);
    workspace.slot_agent_efforts.swap(idx_a, idx_b);
    workspace.slot_pane_states.swap(idx_a, idx_b);
    true
}

fn move_workspace_slot_into_split(
    workspace: &mut WorkspaceEntry,
    source_slot_id: u64,
    target_slot_id: u64,
    action: TerminalSlotDropAction,
) -> Result<TerminalSlotSplitMove, String> {
    if source_slot_id == target_slot_id {
        return Err("cannot split a terminal into itself".into());
    }
    let Some(source_idx) = workspace
        .slot_ids
        .iter()
        .position(|id| *id == source_slot_id)
    else {
        return Err("source slot not found".into());
    };
    let Some(target_idx) = workspace
        .slot_ids
        .iter()
        .position(|id| *id == target_slot_id)
    else {
        return Err("target slot not found".into());
    };
    if workspace.slot_ids.len() <= 1 {
        return Err("workspace must keep at least one target slot".into());
    }
    while workspace.slot_agent_labels.len() < workspace.slot_ids.len() {
        workspace.slot_agent_labels.push(String::new());
    }
    while workspace.slot_agent_models.len() < workspace.slot_ids.len() {
        workspace.slot_agent_models.push(String::new());
    }
    while workspace.slot_agent_efforts.len() < workspace.slot_ids.len() {
        workspace.slot_agent_efforts.push(String::new());
    }
    while workspace.slot_pane_states.len() < workspace.slot_ids.len() {
        let sid = workspace.slot_ids[workspace.slot_pane_states.len()];
        workspace
            .slot_pane_states
            .push(SlotPaneState::default_for_slot(sid));
    }

    let mut source_state = workspace
        .slot_pane_states
        .get(source_idx)
        .cloned()
        .unwrap_or_else(|| SlotPaneState::default_for_slot(source_slot_id));
    let source_fallback = workspace.slot_agent_state_at(source_idx);
    source_state.normalize_pane_agents(&source_fallback);
    if source_state.pane_ids.len() != 1 {
        return Err("source slot must have exactly one pane".into());
    }
    let old_pane_id = source_state.pane_ids[0];
    let source_agent = workspace
        .pane_agent_state(source_slot_id, old_pane_id)
        .unwrap_or(source_fallback);

    let mut target_state = workspace
        .slot_pane_states
        .get(target_idx)
        .cloned()
        .unwrap_or_else(|| SlotPaneState::default_for_slot(target_slot_id));
    let target_fallback = workspace.slot_agent_state_at(target_idx);
    target_state.normalize_pane_agents(&target_fallback);
    let insert_at = match action {
        TerminalSlotDropAction::SplitTop | TerminalSlotDropAction::SplitLeft => 0,
        TerminalSlotDropAction::SplitBottom | TerminalSlotDropAction::SplitRight => {
            target_state.pane_ids.len()
        }
        TerminalSlotDropAction::Swap => {
            return Err("swap is not a split action".into());
        }
    };
    target_state.axis = match action {
        TerminalSlotDropAction::SplitTop | TerminalSlotDropAction::SplitBottom => {
            TerminalSplitAxis::Horizontal
        }
        TerminalSlotDropAction::SplitLeft | TerminalSlotDropAction::SplitRight => {
            TerminalSplitAxis::Vertical
        }
        TerminalSlotDropAction::Swap => unreachable!("swap returned above"),
    };
    let mut new_pane_id = target_state.next_pane_id.max(1);
    while target_state.pane_ids.contains(&new_pane_id) {
        new_pane_id = new_pane_id.saturating_add(1);
    }
    target_state.next_pane_id = new_pane_id.saturating_add(1);
    target_state.pane_ids.insert(insert_at, new_pane_id);
    target_state.pane_agents.insert(insert_at, source_agent.clone());

    workspace.slot_ids.remove(source_idx);
    workspace.slot_agent_labels.remove(source_idx);
    workspace.slot_agent_models.remove(source_idx);
    workspace.slot_agent_efforts.remove(source_idx);
    workspace.slot_pane_states.remove(source_idx);
    workspace.slot_name_overrides.remove(&source_slot_id);

    let Some(target_idx_after_remove) = workspace
        .slot_ids
        .iter()
        .position(|id| *id == target_slot_id)
    else {
        return Err("target slot removed unexpectedly".into());
    };
    workspace.slot_pane_states[target_idx_after_remove] = target_state;
    workspace.set_count_and_dims(workspace.slot_ids.len() as u8);

    Ok(TerminalSlotSplitMove {
        workspace_id: workspace.id,
        storage_key: workspace.storage_key.clone(),
        source_slot_id,
        target_slot_id,
        old_pane_id,
        new_pane_id,
        action,
        agent_slug: source_agent.agent_label,
    })
}

/// Pure transfer of a slot between two workspaces, validating inputs and
/// returning the resulting [`TerminalSlotMove`]. Callers wire any
/// side-effects (PTY adoption, key rewrites, focus changes) themselves.
fn transfer_workspace_slot(
    workspaces: &mut [WorkspaceEntry],
    from_workspace_id: u64,
    to_workspace_id: u64,
    slot_id: u64,
) -> Result<TerminalSlotMove, String> {
    if from_workspace_id == to_workspace_id {
        return Err("cannot transfer terminal to the same workspace".into());
    }
    // Read source + target snapshots up-front so the borrow checker is
    // happy when we mutate both inside the same Vec.
    struct SrcInfo {
        index: usize,
        storage_key: String,
        agent_label: String,
        agent_model: String,
        agent_effort: String,
        pane_state: SlotPaneState,
    }
    let src = {
        let source = workspaces
            .iter()
            .find(|w| w.id == from_workspace_id)
            .ok_or_else(|| "source workspace not found".to_string())?;
        if source.slot_ids.len() <= 1 {
            return Err("source workspace must keep at least one slot".into());
        }
        let index = source
            .slot_ids
            .iter()
            .position(|id| *id == slot_id)
            .ok_or_else(|| "slot not found in source workspace".to_string())?;
        let agent_label = source
            .slot_agent_labels
            .get(index)
            .cloned()
            .unwrap_or_default();
        let agent_model = source
            .slot_agent_models
            .get(index)
            .cloned()
            .unwrap_or_default();
        let agent_effort = source
            .slot_agent_efforts
            .get(index)
            .cloned()
            .unwrap_or_default();
        let pane_state = source
            .slot_pane_states
            .get(index)
            .cloned()
            .unwrap_or_else(|| SlotPaneState::default_for_slot(slot_id));
        SrcInfo {
            index,
            storage_key: source.storage_key.clone(),
            agent_label,
            agent_model,
            agent_effort,
            pane_state,
        }
    };
    let (target_storage_key, new_slot_id) = {
        let target = workspaces
            .iter()
            .find(|w| w.id == to_workspace_id)
            .ok_or_else(|| "target workspace not found".to_string())?;
        if is_shell_workspace(target) {
            return Err("target workspace is not a valid drop target".into());
        }
        if target.slot_ids.len() >= 16 {
            return Err("target workspace already at maximum (16 slots)".into());
        }
        let mut new_slot_id = target.next_terminal_id.max(1);
        while target.slot_ids.contains(&new_slot_id) {
            new_slot_id += 1;
        }
        (target.storage_key.clone(), new_slot_id)
    };

    if let Some(source) = workspaces.iter_mut().find(|w| w.id == from_workspace_id) {
        source.slot_ids.remove(src.index);
        if src.index < source.slot_agent_labels.len() {
            source.slot_agent_labels.remove(src.index);
        }
        if src.index < source.slot_agent_models.len() {
            source.slot_agent_models.remove(src.index);
        }
        if src.index < source.slot_agent_efforts.len() {
            source.slot_agent_efforts.remove(src.index);
        }
        if src.index < source.slot_pane_states.len() {
            source.slot_pane_states.remove(src.index);
        }
        let next_count = source.slot_ids.len() as u8;
        source.set_count_and_dims(next_count.max(1));
    }
    if let Some(target) = workspaces.iter_mut().find(|w| w.id == to_workspace_id) {
        target.slot_ids.push(new_slot_id);
        target.slot_agent_labels.push(src.agent_label.clone());
        target.slot_agent_models.push(src.agent_model.clone());
        target.slot_agent_efforts.push(src.agent_effort.clone());
        target.slot_pane_states.push(src.pane_state.clone());
        target.next_terminal_id = new_slot_id.saturating_add(1);
        let next_count = target.slot_ids.len() as u8;
        target.set_count_and_dims(next_count.max(1));
    }

    Ok(TerminalSlotMove {
        from_workspace_id,
        to_workspace_id,
        old_storage_key: src.storage_key,
        new_storage_key: target_storage_key,
        old_slot_id: slot_id,
        new_slot_id,
        pane_ids: src.pane_state.pane_ids.clone(),
        agent_slug: src.agent_label,
    })
}

#[cfg(test)]
fn reorder_workspace_slots(workspace: &mut WorkspaceEntry, from_index: usize, to_index: usize) {
    if from_index == to_index {
        return;
    }
    let n = workspace.slot_ids.len();
    if from_index >= n || to_index >= n {
        return;
    }
    let id = workspace.slot_ids.remove(from_index);
    let label = workspace.slot_agent_labels.remove(from_index);
    let model = if from_index < workspace.slot_agent_models.len() {
        workspace.slot_agent_models.remove(from_index)
    } else {
        String::new()
    };
    let effort = if from_index < workspace.slot_agent_efforts.len() {
        workspace.slot_agent_efforts.remove(from_index)
    } else {
        String::new()
    };
    let pane = if from_index < workspace.slot_pane_states.len() {
        workspace.slot_pane_states.remove(from_index)
    } else {
        SlotPaneState::default_for_slot(id)
    };
    let insert_at = to_index.min(workspace.slot_ids.len());
    workspace.slot_ids.insert(insert_at, id);
    workspace.slot_agent_labels.insert(insert_at, label);
    workspace
        .slot_agent_models
        .insert(insert_at.min(workspace.slot_agent_models.len()), model);
    workspace
        .slot_agent_efforts
        .insert(insert_at.min(workspace.slot_agent_efforts.len()), effort);
    workspace
        .slot_pane_states
        .insert(insert_at.min(workspace.slot_pane_states.len()), pane);
}

#[cfg(test)]
mod center_tab_tests {
    use super::*;

    fn mk_workspace(id: u64, tabs: Vec<CenterTab>) -> WorkspaceEntry {
        let active = tabs.first().map(|t| t.id).unwrap_or(0);
        WorkspaceEntry {
            id,
            storage_key: "ws-storage".into(),
            title: format!("Workspace {id}"),
            color: "#7dd3fc".into(),
            cwd: "/tmp/blxcode-test".into(),
            terminal_count: 1,
            grid_rows: 1,
            grid_cols: 1,
            next_terminal_id: 2,
            slot_ids: vec![1],
            slot_agent_labels: vec![String::new()],
            slot_agent_models: Vec::new(),
            slot_agent_efforts: Vec::new(),
            slot_pane_states: vec![SlotPaneState::default_for_slot(1)],
            configuring: false,
            agent_timeline: TimelineDoc::default(),
            agent_compose_draft: String::new(),
            agent_image_mode: false,
            agent_chat_mode: AgentChatMode::AskEdits,
            agent_enhance_prompt_before_send: false,
            architecture_llm_prose: false,
            agent_context_items: Vec::new(),
            agent_chat_sessions: Vec::new(),
            active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
            memory_category_settings: HashMap::new(),
            agent_chat_usage: ChatUsageStats::default(),
            sidebar_explorer_open: true,
            sidebar_graph_open: false,
            sidebar_diff_open: true,
            sidebar_explorer_expanded_paths: Vec::new(),
            center_tabs: tabs,
            center_active_tab_id: active,
            center_next_tab_id: default_center_next_tab_id(),
            remote_connection_id: None,
            worktree: None,
            slot_name_overrides: std::collections::HashMap::new(),
            agent_session_role: None,
            view_mode: WorkspaceViewMode::Grid,
            canvas_view_state: CanvasViewState::default(),
            canvas_edges: Vec::new(),
            canvas_default_transfer_mode: CanvasTransferMode::Structured,
            swarm_view_state: SwarmViewState::default(),
        }
    }

    #[test]
    fn legacy_snapshot_defaults_to_grid_view_mode() {
        let mut value = serde_json::to_value(mk_workspace(1, default_center_tabs())).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("viewMode");
        object.remove("canvasViewState");
        object.remove("canvasEdges");
        object.remove("canvasDefaultTransferMode");
        object.remove("swarmViewState");

        let ws: WorkspaceEntry = serde_json::from_value(value).unwrap();

        assert_eq!(ws.view_mode, WorkspaceViewMode::Grid);
        assert_eq!(ws.canvas_view_state, CanvasViewState::default());
        assert!(ws.canvas_edges.is_empty());
        assert_eq!(
            ws.canvas_default_transfer_mode,
            CanvasTransferMode::Structured
        );
        assert_eq!(ws.swarm_view_state, SwarmViewState::default());
    }

    #[test]
    fn legacy_agent_chat_fields_migrate_to_default_session() {
        let mut value = serde_json::to_value(mk_workspace(1, default_center_tabs())).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("agent_chat_sessions");
        object.remove("active_agent_chat_session_id");
        object.insert(
            "agent_compose_draft".to_string(),
            serde_json::Value::String("legacy draft".to_string()),
        );
        object.insert(
            "agent_image_mode".to_string(),
            serde_json::Value::Bool(true),
        );

        let mut ws: WorkspaceEntry = serde_json::from_value(value).unwrap();
        ws.ensure_agent_chat_sessions();

        assert_eq!(ws.agent_chat_sessions.len(), 1);
        assert_eq!(
            ws.active_agent_chat_session_id,
            DEFAULT_AGENT_CHAT_SESSION_ID
        );
        let session = &ws.agent_chat_sessions[0];
        assert_eq!(session.draft, "legacy draft");
        assert!(session.image_mode);
        assert_eq!(session.chat_mode, AgentChatMode::AskEdits);
    }

    #[test]
    fn repair_syncs_single_mode_tab_to_canvas() {
        let mut ws = mk_workspace(
            1,
            vec![CenterTab {
                id: 42,
                title: "Settings".into(),
                kind: CenterTabKind::Settings,
            }],
        );
        ws.view_mode = WorkspaceViewMode::Canvas;

        repair_center_tab_state(&mut ws);

        let mode_tabs: Vec<_> = ws
            .center_tabs
            .iter()
            .filter(|tab| is_workspace_mode_tab_kind(&tab.kind))
            .collect();
        assert_eq!(mode_tabs.len(), 1);
        assert_eq!(mode_tabs[0].id, CENTER_TERMINALS_TAB_ID);
        assert_eq!(mode_tabs[0].title, "Canvas");
        assert!(matches!(mode_tabs[0].kind, CenterTabKind::Canvas));
        assert_eq!(ws.center_active_tab_id, 42);
    }

    #[test]
    fn repair_collapses_duplicate_mode_tabs() {
        // A corrupted snapshot carrying both a Terminals and a Swarm mode tab
        // must collapse to a single canonical view-mode tab on load — no
        // duplicate (and unclickable) centered tab.
        let mut ws = mk_workspace(
            1,
            vec![
                CenterTab::kanban(),
                CenterTab::terminals(),
                CenterTab {
                    id: 999,
                    title: "Swarm".into(),
                    kind: CenterTabKind::Swarm,
                },
            ],
        );
        ws.view_mode = WorkspaceViewMode::Swarm;

        repair_center_tab_state(&mut ws);

        let mode_tabs: Vec<_> = ws
            .center_tabs
            .iter()
            .filter(|tab| {
                is_workspace_mode_tab_kind(&tab.kind) || tab.id == CENTER_TERMINALS_TAB_ID
            })
            .collect();
        assert_eq!(mode_tabs.len(), 1, "duplicate mode tabs must be collapsed");
        assert_eq!(mode_tabs[0].id, CENTER_TERMINALS_TAB_ID);
        assert!(matches!(mode_tabs[0].kind, CenterTabKind::Swarm));
        // No two center tabs may share an id.
        let mut ids: Vec<u64> = ws.center_tabs.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped, "center tab ids must be unique");
    }

    #[test]
    fn repair_keeps_empty_shell_tabs_empty() {
        let mut ws = mk_workspace(1, vec![]);
        ws.cwd = String::new();

        repair_center_tab_state(&mut ws);

        assert_eq!(ws.center_active_tab_id, 0);
        assert!(ws.center_tabs.is_empty());
    }

    #[test]
    fn repair_fixes_dangling_active_id() {
        let mut ws = mk_workspace(1, vec![CenterTab::terminals()]);
        ws.center_active_tab_id = 999;
        repair_center_tab_state(&mut ws);
        assert_eq!(ws.center_active_tab_id, CENTER_TERMINALS_TAB_ID);
    }

    #[test]
    fn is_shell_workspace_only_for_empty_cwd_no_configure() {
        let mut ws = mk_workspace(1, vec![CenterTab::terminals()]);
        ws.cwd = String::new();
        assert!(is_shell_workspace(&ws));
        ws.configuring = true;
        assert!(!is_shell_workspace(&ws));
        ws.configuring = false;
        ws.cwd = "/tmp/x".into();
        assert!(!is_shell_workspace(&ws));
    }
}

#[cfg(test)]
mod terminal_slot_tests {
    use super::*;

    fn mk_slots(n: u8) -> WorkspaceEntry {
        let slot_ids: Vec<u64> = (1..=n as u64).collect();
        let slot_pane_states: Vec<SlotPaneState> = slot_ids
            .iter()
            .copied()
            .map(SlotPaneState::default_for_slot)
            .collect();
        let (grid_rows, grid_cols) = WorkspaceEntry::grid_dims_for_count(n);
        WorkspaceEntry {
            id: 1,
            storage_key: "ws-storage".into(),
            title: "Test".into(),
            color: "#7dd3fc".into(),
            cwd: "/tmp".into(),
            terminal_count: n,
            grid_rows,
            grid_cols,
            next_terminal_id: n as u64 + 1,
            slot_ids,
            slot_agent_labels: (0..n as usize).map(|i| format!("label{i}")).collect(),
            slot_agent_models: (0..n as usize).map(|i| format!("model{i}")).collect(),
            slot_agent_efforts: (0..n as usize).map(|i| format!("effort{i}")).collect(),
            slot_pane_states,
            configuring: false,
            agent_timeline: TimelineDoc::default(),
            agent_compose_draft: String::new(),
            agent_image_mode: false,
            agent_chat_mode: AgentChatMode::AskEdits,
            agent_enhance_prompt_before_send: false,
            architecture_llm_prose: false,
            agent_context_items: Vec::new(),
            agent_chat_sessions: Vec::new(),
            active_agent_chat_session_id: DEFAULT_AGENT_CHAT_SESSION_ID.to_string(),
            memory_category_settings: HashMap::new(),
            agent_chat_usage: ChatUsageStats::default(),
            sidebar_explorer_open: true,
            sidebar_graph_open: false,
            sidebar_diff_open: true,
            sidebar_explorer_expanded_paths: Vec::new(),
            center_tabs: default_center_tabs(),
            center_active_tab_id: default_center_active_tab_id(),
            center_next_tab_id: default_center_next_tab_id(),
            remote_connection_id: None,
            worktree: None,
            slot_name_overrides: std::collections::HashMap::new(),
            agent_session_role: None,
            view_mode: WorkspaceViewMode::Grid,
            canvas_view_state: CanvasViewState::default(),
            canvas_edges: Vec::new(),
            canvas_default_transfer_mode: CanvasTransferMode::Structured,
            swarm_view_state: SwarmViewState::default(),
        }
    }

    fn test_service() -> WorkbenchService {
        WorkbenchService {
            workspaces: RwSignal::new(Vec::new()),
            active_id: RwSignal::new(None),
            recent_workspaces: RwSignal::new(Vec::new()),
            sidebar_collapsed: RwSignal::new(false),
            sidebar_width_px: RwSignal::new(SIDEBAR_WIDTH_PX_DEFAULT),
            right_collapsed: RwSignal::new(false),
            right_width_px: RwSignal::new(420.0),
            right_tab: RwSignal::new(RightPanelTab::Agent),
            browser_url: RwSignal::new(HARNESS_BROWSER_DEFAULT_URL.to_string()),
            embedded_browser_tabs: RwSignal::new(Vec::new()),
            embedded_browser_active_id: RwSignal::new(0),
            embedded_browser_next_id: RwSignal::new(1),
            harness_workspace_root: RwSignal::new(String::new()),
            default_project_dir: RwSignal::new(String::new()),
            default_session_role: RwSignal::new(None),
            workspace_next_id: RwSignal::new(1),
            workspace_drafts: RwSignal::new(HashMap::new()),
            workspace_config_steps: RwSignal::new(HashMap::new()),
            pty_sessions: RwSignal::new(HashMap::new()),
            pending_memory_note: RwSignal::new(None),
            terminal_layout_tick: RwSignal::new(0),
            notifications: RwSignal::new(HashMap::new()),
            agent_notifications: RwSignal::new(Vec::new()),
            pending_clears: RwSignal::new(HashSet::new()),
            focused_terminal_by_workspace: RwSignal::new(HashMap::new()),
            terminal_titles: RwSignal::new(HashMap::new()),
            memory_color_presets: RwSignal::new(Vec::new()),
            agent_image_context: RwSignal::new(HashMap::new()),
            sidebar_repo_epoch: RwSignal::new(0),
            plans_epoch: RwSignal::new(0),
            kanban_plan_focus: RwSignal::new(None),
            terminal_move_guards: RwSignal::new(HashMap::new()),
            terminal_adopt_pending: RwSignal::new(HashMap::new()),
            terminal_popouts: RwSignal::new(HashMap::new()),
        }
    }

    #[test]
    fn reorder_permutes_parallel_vectors() {
        let mut ws = mk_slots(3);
        ws.slot_agent_models = vec!["m0".into(), "m1".into(), "m2".into()];
        ws.slot_agent_efforts = vec!["e0".into(), "e1".into(), "e2".into()];
        reorder_workspace_slots(&mut ws, 1, 2);
        assert_eq!(ws.slot_ids, vec![1, 3, 2]);
        assert_eq!(ws.slot_agent_labels, vec!["label0", "label2", "label1"]);
        assert_eq!(ws.slot_agent_models, vec!["m0", "m2", "m1"]);
        assert_eq!(ws.slot_agent_efforts, vec!["e0", "e2", "e1"]);
    }

    #[test]
    fn reorder_noop_on_same_index() {
        let mut ws = mk_slots(2);
        reorder_workspace_slots(&mut ws, 0, 0);
        assert_eq!(ws.slot_ids, vec![1, 2]);
    }

    #[test]
    fn reorder_moves_first_slot_to_second() {
        let mut ws = mk_slots(2);
        reorder_workspace_slots(&mut ws, 0, 1);
        assert_eq!(ws.slot_ids, vec![2, 1]);
    }

    #[test]
    fn swap_exchanges_slot_positions() {
        let mut ws = mk_slots(3);
        ws.slot_agent_models = vec!["m0".into(), "m1".into(), "m2".into()];
        ws.slot_agent_efforts = vec!["e0".into(), "e1".into(), "e2".into()];
        assert!(swap_workspace_slots(&mut ws, 1, 3));
        assert_eq!(ws.slot_ids, vec![3, 2, 1]);
        assert_eq!(
            ws.slot_agent_labels,
            vec!["label2".to_string(), "label1".into(), "label0".into()]
        );
        assert_eq!(ws.slot_agent_models, vec!["m2", "m1", "m0"]);
        assert_eq!(ws.slot_agent_efforts, vec!["e2", "e1", "e0"]);
        assert_eq!(ws.slot_pane_states[0], SlotPaneState::default_for_slot(3));
        assert_eq!(ws.slot_pane_states[2], SlotPaneState::default_for_slot(1));
    }

    #[test]
    fn swap_noop_on_same_slot() {
        let mut ws = mk_slots(3);
        assert!(!swap_workspace_slots(&mut ws, 2, 2));
        assert_eq!(ws.slot_ids, vec![1, 2, 3]);
    }

    #[test]
    fn swap_normalizes_missing_pane_states() {
        let mut ws = mk_slots(3);
        ws.slot_pane_states.clear();
        assert!(swap_workspace_slots(&mut ws, 1, 3));
        assert_eq!(ws.slot_ids, vec![3, 2, 1]);
        assert_eq!(ws.slot_pane_states.len(), 3);
        assert_eq!(ws.slot_pane_states[0], SlotPaneState::default_for_slot(3));
        assert_eq!(ws.slot_pane_states[2], SlotPaneState::default_for_slot(1));
    }

    #[test]
    fn swap_skipped_for_unknown_id() {
        let mut ws = mk_slots(2);
        assert!(!swap_workspace_slots(&mut ws, 1, 99));
        assert_eq!(ws.slot_ids, vec![1, 2]);
    }

    #[test]
    fn move_into_split_removes_source_and_inserts_target_pane() {
        let mut ws = mk_slots(3);
        let target_first_pane = ws.slot_pane_states[2].pane_ids[0];

        let mv = move_workspace_slot_into_split(
            &mut ws,
            1,
            3,
            TerminalSlotDropAction::SplitRight,
        )
        .expect("move into split");

        assert_eq!(ws.slot_ids, vec![2, 3]);
        assert_eq!(ws.terminal_count, 2);
        assert_eq!(ws.slot_agent_labels, vec!["label1", "label2"]);
        assert_eq!(mv.source_slot_id, 1);
        assert_eq!(mv.target_slot_id, 3);
        assert_eq!(mv.old_pane_id, 1001);
        assert_eq!(mv.agent_slug, "label0");
        assert_eq!(
            mv.terminal_key_pair(),
            (
                "ws-storage:1:1001".to_string(),
                format!("ws-storage:3:{}", mv.new_pane_id)
            )
        );

        let target = ws.slot_pane_states.last().expect("target pane state");
        assert_eq!(target.axis, TerminalSplitAxis::Vertical);
        assert_eq!(target.pane_ids, vec![target_first_pane, mv.new_pane_id]);
        assert_eq!(target.pane_agents[1].agent_label, "label0");
        assert_eq!(target.pane_agents[1].agent_model, "model0");
        assert_eq!(target.pane_agents[1].agent_effort, "effort0");
    }

    #[test]
    fn move_into_split_rejects_invalid_inputs() {
        let mut ws = mk_slots(2);
        assert!(move_workspace_slot_into_split(
            &mut ws,
            1,
            1,
            TerminalSlotDropAction::SplitRight,
        )
        .is_err());
        assert!(move_workspace_slot_into_split(
            &mut ws,
            99,
            2,
            TerminalSlotDropAction::SplitRight,
        )
        .is_err());
        assert!(move_workspace_slot_into_split(
            &mut ws,
            1,
            99,
            TerminalSlotDropAction::SplitRight,
        )
        .is_err());
        assert!(move_workspace_slot_into_split(&mut ws, 1, 2, TerminalSlotDropAction::Swap)
            .is_err());
    }

    #[test]
    fn move_into_split_rejects_multi_pane_source() {
        let mut ws = mk_slots(2);
        ws.slot_pane_states[0].pane_ids.push(1002);

        let result = move_workspace_slot_into_split(
            &mut ws,
            1,
            2,
            TerminalSlotDropAction::SplitBottom,
        );

        assert!(result.is_err());
        assert_eq!(ws.slot_ids, vec![1, 2]);
    }

    #[test]
    fn service_move_terminal_slot_into_split_updates_workspace() {
        Owner::new().with(|| {
            let svc = test_service();
            svc.workspaces.set(vec![mk_slots(2)]);

            let mv = svc
                .move_terminal_slot_into_split(1, 1, 2, TerminalSlotDropAction::SplitRight)
                .expect("service split move");

            assert_eq!(mv.source_slot_id, 1);
            let ws = svc.workspaces.with_untracked(|items| items[0].clone());
            assert_eq!(ws.slot_ids, vec![2]);
            assert_eq!(ws.slot_pane_states[0].pane_ids.len(), 2);
        });
    }

    #[test]
    fn service_move_terminal_slot_into_split_sets_adoption_guards() {
        Owner::new().with(|| {
            let svc = test_service();
            svc.workspaces.set(vec![mk_slots(2)]);
            let old_key = "ws-storage:1:1001".to_string();
            svc.register_pty_session(old_key.clone(), 42);
            svc.notifications.update(|m| {
                m.insert(old_key.clone(), 7);
            });
            svc.focused_terminal_by_workspace.update(|m| {
                m.insert("ws-storage".into(), old_key.clone());
            });

            let mv = svc
                .move_terminal_slot_into_split(1, 1, 2, TerminalSlotDropAction::SplitRight)
                .expect("service split move");
            let (old, new) = mv.terminal_key_pair();

            assert_eq!(old, old_key);
            assert_eq!(svc.pty_sessions.with_untracked(|m| m.get(&new).copied()), Some(42));
            assert!(!svc.pty_sessions.with_untracked(|m| m.contains_key(&old)));
            assert_eq!(
                svc.terminal_adopt_pending
                    .with_untracked(|m| m.get(&new).copied()),
                Some(42)
            );
            assert_eq!(
                svc.terminal_move_guards
                    .with_untracked(|m| m.get(&old).cloned()),
                Some(new.clone())
            );
            assert_eq!(svc.notifications.with_untracked(|m| m.get(&new).copied()), Some(7));
            assert_eq!(
                svc.focused_terminal_by_workspace
                    .with_untracked(|m| m.get("ws-storage").cloned()),
                Some(new)
            );
        });
    }

    fn mk_slots_with_id(id: u64, n: u8) -> WorkspaceEntry {
        let mut ws = mk_slots(n);
        ws.id = id;
        ws.storage_key = format!("ws-storage-{id}");
        ws
    }

    #[test]
    fn slot_pane_state_deserializes_without_pane_agents() {
        let value = serde_json::json!({
            "axis": "Vertical",
            "pane_ids": [1001],
            "next_pane_id": 1002
        });

        let state: SlotPaneState = serde_json::from_value(value).unwrap();

        assert_eq!(state.axis, TerminalSplitAxis::Vertical);
        assert_eq!(state.pane_ids, vec![1001]);
        assert_eq!(state.next_pane_id, 1002);
        assert!(state.pane_agents.is_empty());
    }

    #[test]
    fn pane_agent_state_falls_back_to_slot_agent_arrays() {
        let ws = mk_slots(2);
        let pane_id = SlotPaneState::default_for_slot(2).pane_ids[0];

        let agent = ws.pane_agent_state(2, pane_id).expect("pane agent");

        assert_eq!(agent.agent_label, "label1");
        assert_eq!(agent.agent_model, "model1");
        assert_eq!(agent.agent_effort, "effort1");
    }

    #[test]
    fn pane_agent_state_uses_pane_values_with_field_fallback() {
        let mut ws = mk_slots(1);
        let pane_id = ws.slot_pane_states[0].pane_ids[0];
        ws.slot_pane_states[0].pane_agents = vec![SlotPaneAgentState {
            agent_label: "codex".into(),
            agent_model: String::new(),
            agent_effort: "high".into(),
        }];

        let agent = ws.pane_agent_state(1, pane_id).expect("pane agent");

        assert_eq!(agent.agent_label, "codex");
        assert_eq!(agent.agent_model, "model0");
        assert_eq!(agent.agent_effort, "high");
    }

    #[test]
    fn normalize_pane_agents_aligns_metadata_to_panes() {
        let mut state = SlotPaneState {
            axis: TerminalSplitAxis::Vertical,
            pane_ids: vec![1, 2],
            next_pane_id: 3,
            pane_agents: vec![
                SlotPaneAgentState {
                    agent_label: "codex".into(),
                    agent_model: String::new(),
                    agent_effort: "high".into(),
                }
            ],
        };
        let fallback = SlotPaneAgentState {
            agent_label: "claude".into(),
            agent_model: "sonnet".into(),
            agent_effort: "medium".into(),
        };

        state.normalize_pane_agents(&fallback);

        assert_eq!(state.pane_agents.len(), 2);
        assert_eq!(state.pane_agents[0].agent_label, "codex");
        assert_eq!(state.pane_agents[0].agent_model, "sonnet");
        assert_eq!(state.pane_agents[0].agent_effort, "high");
        assert_eq!(state.pane_agents[1], fallback);
    }

    #[test]
    fn transfer_moves_slot_to_target_and_keeps_arrays_aligned() {
        let mut list = vec![mk_slots_with_id(1, 3), mk_slots_with_id(2, 1)];
        let mv = transfer_workspace_slot(&mut list, 1, 2, 2).expect("transfer ok");
        assert_eq!(mv.from_workspace_id, 1);
        assert_eq!(mv.to_workspace_id, 2);
        assert_eq!(mv.old_slot_id, 2);
        let source = list.iter().find(|w| w.id == 1).unwrap();
        let target = list.iter().find(|w| w.id == 2).unwrap();
        assert_eq!(source.slot_ids, vec![1, 3]);
        assert_eq!(source.slot_agent_labels.len(), source.slot_ids.len());
        assert_eq!(source.slot_agent_models.len(), source.slot_ids.len());
        assert_eq!(source.slot_agent_efforts.len(), source.slot_ids.len());
        assert_eq!(source.slot_pane_states.len(), source.slot_ids.len());
        assert_eq!(target.slot_ids.len(), 2);
        assert_eq!(target.slot_ids[1], mv.new_slot_id);
        assert_eq!(target.slot_agent_labels.len(), target.slot_ids.len());
        assert_eq!(target.slot_agent_models.len(), target.slot_ids.len());
        assert_eq!(target.slot_agent_efforts.len(), target.slot_ids.len());
        assert_eq!(target.slot_agent_models[1], "model1");
        assert_eq!(target.slot_agent_efforts[1], "effort1");
        assert!(target.next_terminal_id > mv.new_slot_id);
        // Pane state on the target preserves the source's pane layout.
        assert_eq!(target.slot_pane_states[1].pane_ids, mv.pane_ids);
    }

    #[test]
    fn transfer_rejects_same_workspace_and_last_slot_and_full_target() {
        let mut list = vec![mk_slots_with_id(1, 2)];
        assert!(transfer_workspace_slot(&mut list, 1, 1, 1).is_err());

        let mut list = vec![mk_slots_with_id(1, 1), mk_slots_with_id(2, 1)];
        assert!(transfer_workspace_slot(&mut list, 1, 2, 1).is_err());

        let mut target = mk_slots_with_id(2, 16);
        target.next_terminal_id = 17;
        let mut list = vec![mk_slots_with_id(1, 2), target];
        assert!(transfer_workspace_slot(&mut list, 1, 2, 1).is_err());
    }

    #[test]
    fn transfer_mints_fresh_slot_id_on_target() {
        let mut list = vec![mk_slots_with_id(1, 3), mk_slots_with_id(2, 2)];
        let mv = transfer_workspace_slot(&mut list, 1, 2, 2).expect("transfer ok");
        let target = list.iter().find(|w| w.id == 2).unwrap();
        assert_ne!(mv.new_slot_id, 2);
        assert!(target.slot_ids.contains(&mv.new_slot_id));
    }

    #[test]
    fn terminal_key_pairs_use_per_pane_suffix() {
        let mut list = vec![mk_slots_with_id(1, 3), mk_slots_with_id(2, 1)];
        // Inject a multi-pane slot so the pair count exceeds 1.
        if let Some(ws) = list.iter_mut().find(|w| w.id == 1) {
            ws.slot_pane_states[1] = SlotPaneState {
                axis: TerminalSplitAxis::Vertical,
                pane_ids: vec![7, 11],
                next_pane_id: 12,
                pane_agents: Vec::new(),
            };
        }
        let mv = transfer_workspace_slot(&mut list, 1, 2, 2).expect("transfer ok");
        let pairs = mv.terminal_key_pairs();
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].0.ends_with(&format!(":{}:7", mv.old_slot_id)));
        assert!(pairs[0].1.ends_with(&format!(":{}:7", mv.new_slot_id)));
        assert!(pairs[1].0.ends_with(&format!(":{}:11", mv.old_slot_id)));
    }

    #[test]
    fn return_popout_by_label_clears_placeholder_and_marks_adoption() {
        Owner::new().with(|| {
            let svc = test_service();
            let terminal_key = "workspace:1:1".to_string();
            svc.register_pty_session(terminal_key.clone(), 42);
            svc.prepare_terminal_popout(&terminal_key, "popout-terminal-test".into());

            svc.return_terminal_popout_by_label("popout-terminal-test");

            assert_eq!(svc.terminal_popout_label(&terminal_key), None);
            assert_eq!(
                svc.terminal_adopt_pending
                    .with_untracked(|m| m.get(&terminal_key).copied()),
                Some(42)
            );
        });
    }

    #[test]
    fn hydrate_clears_stale_terminal_runtime_state() {
        Owner::new().with(|| {
            let svc = test_service();
            let terminal_key = "workspace:1:1".to_string();
            svc.register_pty_session(terminal_key.clone(), 42);
            svc.prepare_terminal_popout(&terminal_key, "popout-terminal-test".into());

            assert!(svc.hydrate(WorkbenchSnapshot {
                version: WORKBENCH_SNAPSHOT_VERSION,
                workspaces: Vec::new(),
                active_id: None,
                workspace_next_id: 1,
                sidebar_collapsed: false,
                sidebar_width_px: SIDEBAR_WIDTH_PX_DEFAULT,
                right_collapsed: false,
                right_width_px: 420.0,
                right_tab: RightPanelTab::Agent,
                recent_workspaces: Vec::new(),
                embedded_browser_tabs: Vec::new(),
                embedded_browser_active_id: 0,
                embedded_browser_next_id: 1,
            }));

            assert!(svc.pty_sessions.with_untracked(|m| m.is_empty()));
            assert!(svc.terminal_move_guards.with_untracked(|m| m.is_empty()));
            assert!(svc.terminal_adopt_pending.with_untracked(|m| m.is_empty()));
            assert_eq!(svc.terminal_popout_label(&terminal_key), None);
        });
    }
}
