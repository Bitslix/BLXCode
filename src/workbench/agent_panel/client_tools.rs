use crate::agent_wire::{AgentContextItem, AgentContextKind, AgentEvent};
use crate::tauri_bridge::{
    agent_submit_tool_result, memory_list, pty_peek_output, pty_wait_output, pty_write,
    window_set_fullscreen, window_set_size, window_state,
};
use crate::workbench::agent_context_handoff::{
    perform_handoff, HandoffRequest, WorkspaceTerminalTarget,
};
use crate::workbench::state::normalize_hex_color;
use crate::workbench::terminal_naming::{self, TerminalNamingMode, NAME_POOL_KEY, NAMING_MODE_KEY};
use crate::workbench::{HarnessSettingsCategory, RightPanelTab, WorkbenchService};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use leptos::prelude::*;

const PTY_READY_ATTEMPTS: u32 = 40;
const PTY_READY_DELAY_MS: u32 = 50;

/// Read a localStorage value (the agent tool handlers run inside a spawned
/// task where the reactive owner — and thus `expect_context` — may be
/// unavailable, so we read the persisted naming prefs directly).
fn read_local_storage(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(key).ok().flatten())
}

fn current_naming_mode() -> TerminalNamingMode {
    TerminalNamingMode::from_storage(read_local_storage(NAMING_MODE_KEY).as_deref())
}

fn current_name_pool() -> Vec<String> {
    terminal_naming::parse_pool(read_local_storage(NAME_POOL_KEY).as_deref())
}

/// Friendly name for a slot (override > deterministic pool), independent of
/// the active display mode so the agent can address terminals by name even
/// when the user currently sees slot numbers.
fn resolved_slot_name(
    wb: &WorkbenchService,
    workspace_id: u64,
    slot_id: u64,
    pool: &[String],
    siblings: &[u64],
) -> Option<String> {
    let override_name = wb.slot_name_override(workspace_id, slot_id);
    terminal_naming::resolve_slot_name(slot_id, override_name.as_deref(), pool, siblings)
}

pub fn maybe_handle_client_tool(ev: &AgentEvent, wb: WorkbenchService) {
    let AgentEvent::ToolCall {
        tool,
        call_id: Some(call_id),
        args,
    } = ev
    else {
        return;
    };
    let call_id = call_id.clone();
    match tool.as_str() {
        "harness.create_workspace" => handle_create_workspace(call_id, args.clone(), wb),
        "harness.workspace_list" => handle_workspace_list(call_id, wb),
        "harness.workspace_switch" => handle_workspace_switch(call_id, args.clone(), wb),
        "harness.workspace_prev" => handle_workspace_step(call_id, wb, -1),
        "harness.workspace_next" => handle_workspace_step(call_id, wb, 1),
        "harness.view_show" => handle_view_show(call_id, args.clone(), wb),
        "harness.open_settings" => handle_open_settings(call_id, args.clone(), wb),
        "harness.open_memory" => handle_open_memory(call_id, args.clone(), wb),
        "harness.open_plan" => handle_open_plan(call_id, args.clone(), wb),
        "harness.open_file" => handle_open_file(call_id, args.clone(), wb),
        "harness.open_diff" => handle_open_diff(call_id, args.clone(), wb),
        "harness.window_get_state" => handle_window_get_state(call_id),
        "harness.window_set_size" => handle_window_set_size(call_id, args.clone()),
        "harness.window_set_fullscreen" => handle_window_set_fullscreen(call_id, args.clone()),
        "harness.open_terminal" => handle_open_terminal(call_id, args.clone(), wb),
        "harness.list_terminals" => handle_list_terminals(call_id, wb),
        "harness.send_terminal_keys" => handle_send_keys(call_id, args.clone(), wb),
        "harness.send_agent_context" => handle_send_agent_context(call_id, args.clone(), wb),
        "harness.read_terminal_output" => handle_read_output(call_id, args.clone(), wb),
        "harness.wait_terminal_output" => handle_wait_output(call_id, args.clone(), wb),
        "harness.terminal_interrupt" => handle_terminal_interrupt(call_id, args.clone(), wb),
        "harness.ask_user" => handle_ask_user(call_id, args.clone()),
        "memory_category_list" => handle_memory_category_list(call_id, wb),
        "memory_category_update" => handle_memory_category_update(call_id, args.clone(), wb),
        "memory_context_list" => handle_memory_context_list(call_id, wb),
        "memory_context_attach" => handle_memory_context_attach(call_id, args.clone(), wb),
        "memory_context_detach" => handle_memory_context_detach(call_id, args.clone(), wb),
        "image_context_list" => handle_image_context_list(call_id, wb),
        "image_context_detach" => handle_image_context_detach(call_id, args.clone(), wb),
        _ => {}
    }
}

const LEARNINGS_PREFIX: &str = "learnings/";

fn handle_workspace_list(call_id: String, wb: WorkbenchService) {
    let active = wb.active_id().get_untracked();
    let items = wb.workspaces().with_untracked(|workspaces| {
        workspaces
            .iter()
            .map(|ws| {
                serde_json::json!({
                    "id": ws.id,
                    "title": ws.title,
                    "cwd": ws.cwd,
                    "active": Some(ws.id) == active,
                    "terminalCount": ws.terminal_count,
                })
            })
            .collect::<Vec<_>>()
    });
    submit_async(
        call_id,
        true,
        format!("{} open workspace(s)", items.len()),
        Some(serde_json::Value::Array(items)),
    );
}

fn handle_workspace_switch(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let id_arg = args
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_u64());
    let title_arg = args
        .as_ref()
        .and_then(|v| v.get("title"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let cwd_arg = args
        .as_ref()
        .and_then(|v| v.get("cwd"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let target = wb.workspaces().with_untracked(|workspaces| {
        workspaces.iter().find_map(|ws| {
            if id_arg == Some(ws.id)
                || title_arg
                    .as_ref()
                    .map(|title| ws.title.to_lowercase() == *title)
                    .unwrap_or(false)
                || cwd_arg
                    .as_ref()
                    .map(|cwd| ws.cwd.to_lowercase() == *cwd)
                    .unwrap_or(false)
            {
                Some((ws.id, ws.title.clone(), ws.cwd.clone()))
            } else {
                None
            }
        })
    });
    let Some((id, title, cwd)) = target else {
        submit_async(call_id, false, "workspace not found".into(), None);
        return;
    };
    wb.select_workspace(id);
    submit_async(
        call_id,
        true,
        format!("switched to workspace {id}"),
        Some(serde_json::json!({ "id": id, "title": title, "cwd": cwd })),
    );
}

fn handle_workspace_step(call_id: String, wb: WorkbenchService, delta: isize) {
    let active = wb.active_id().get_untracked();
    let target = wb.workspaces().with_untracked(|workspaces| {
        if workspaces.is_empty() {
            return None;
        }
        let current = active
            .and_then(|id| workspaces.iter().position(|ws| ws.id == id))
            .unwrap_or(0);
        let len = workspaces.len() as isize;
        let next = (current as isize + delta).rem_euclid(len) as usize;
        let ws = &workspaces[next];
        Some((ws.id, ws.title.clone(), ws.cwd.clone()))
    });
    let Some((id, title, cwd)) = target else {
        submit_async(call_id, false, "no open workspaces".into(), None);
        return;
    };
    wb.select_workspace(id);
    submit_async(
        call_id,
        true,
        format!("switched to workspace {id}"),
        Some(serde_json::json!({ "id": id, "title": title, "cwd": cwd })),
    );
}

fn ensure_right_panel_visible(wb: WorkbenchService) {
    if wb.right_collapsed().get_untracked() {
        wb.toggle_right_panel();
    }
}

fn ensure_sidebar_visible(wb: WorkbenchService) {
    if wb.sidebar_collapsed().get_untracked() {
        wb.toggle_sidebar();
    }
}

fn handle_view_show(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(target) = args
        .as_ref()
        .and_then(|v| v.get("target"))
        .and_then(|v| v.as_str())
    else {
        submit_async(call_id, false, "missing target".into(), None);
        return;
    };
    match target {
        "agent" => {
            wb.set_right_tab(RightPanelTab::Agent);
            ensure_right_panel_visible(wb);
        }
        "browser" => {
            wb.set_right_tab(RightPanelTab::Browser);
            ensure_right_panel_visible(wb);
        }
        "plans" => {
            wb.set_right_tab(RightPanelTab::Plans);
            ensure_right_panel_visible(wb);
        }
        "memory" => {
            wb.set_right_tab(RightPanelTab::Memory);
            ensure_right_panel_visible(wb);
        }
        "rules" => {
            wb.set_right_tab(RightPanelTab::Rules);
            ensure_right_panel_visible(wb);
        }
        "skills" => {
            wb.set_right_tab(RightPanelTab::Skills);
            ensure_right_panel_visible(wb);
        }
        "settings" => wb.open_center_settings_tab(HarnessSettingsCategory::App),
        "terminals" => {
            if let Some(ws_id) = wb.active_id().get_untracked() {
                wb.open_center_terminals_tab(ws_id);
            }
        }
        "project_files" => {
            ensure_sidebar_visible(wb);
            wb.set_active_sidebar_explorer_open(true);
        }
        "git_diff" => {
            ensure_sidebar_visible(wb);
            wb.set_active_sidebar_diff_open(true);
        }
        "git_graph" => {
            ensure_sidebar_visible(wb);
            wb.set_active_sidebar_graph_open(true);
        }
        other => {
            submit_async(
                call_id,
                false,
                format!("unknown view target: {other}"),
                None,
            );
            return;
        }
    }
    submit_async(call_id, true, format!("showed {target}"), None);
}

fn parse_settings_category(raw: &str) -> Option<HarnessSettingsCategory> {
    Some(match raw {
        "app" => HarnessSettingsCategory::App,
        "appearance" => HarnessSettingsCategory::Appearance,
        "shortcuts" => HarnessSettingsCategory::Shortcuts,
        "api_keys" => HarnessSettingsCategory::ApiKeys,
        "workspace" => HarnessSettingsCategory::Workspace,
        "agent_provider" => HarnessSettingsCategory::AgentProvider,
        "remote" => HarnessSettingsCategory::Remote,
        "memory" => HarnessSettingsCategory::Memory,
        "voice" => HarnessSettingsCategory::Voice,
        "image" => HarnessSettingsCategory::Image,
        _ => return None,
    })
}

fn handle_open_settings(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let raw = args
        .as_ref()
        .and_then(|v| v.get("category"))
        .and_then(|v| v.as_str())
        .unwrap_or("app");
    let Some(category) = parse_settings_category(raw) else {
        submit_async(
            call_id,
            false,
            format!("unknown settings category: {raw}"),
            None,
        );
        return;
    };
    wb.open_center_settings_tab(category);
    submit_async(call_id, true, format!("opened settings {raw}"), None);
}

fn handle_open_memory(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    if let Some(path) = args
        .as_ref()
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        wb.request_open_memory_note(path.to_owned());
    } else {
        wb.set_right_tab(RightPanelTab::Memory);
        ensure_right_panel_visible(wb);
    }
    submit_async(call_id, true, "opened memory".into(), None);
}

fn handle_open_plan(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let path = args
        .as_ref()
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    wb.set_right_tab(RightPanelTab::Plans);
    ensure_right_panel_visible(wb);
    let data = (!path.trim().is_empty()).then(|| serde_json::json!({ "path": path }));
    submit_async(call_id, true, "opened plans".into(), data);
}

fn handle_open_file(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(path) = args
        .as_ref()
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().trim_start_matches(['/', '\\']).to_owned())
        .filter(|s| !s.is_empty())
    else {
        submit_async(call_id, false, "missing path".into(), None);
        return;
    };
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    wb.open_center_file_tab(ws_id, path.clone());
    submit_async(call_id, true, format!("opened file {path}"), None);
}

fn handle_open_diff(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(path) = args
        .as_ref()
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().trim_start_matches(['/', '\\']).to_owned())
        .filter(|s| !s.is_empty())
    else {
        submit_async(call_id, false, "missing path".into(), None);
        return;
    };
    let staged = args
        .as_ref()
        .and_then(|v| v.get("staged"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    wb.open_center_diff_tab(ws_id, path.clone(), staged);
    submit_async(call_id, true, format!("opened diff {path}"), None);
}

fn handle_window_get_state(call_id: String) {
    leptos::task::spawn_local(async move {
        match window_state().await {
            Ok(state) => {
                let _ = agent_submit_tool_result(
                    call_id,
                    true,
                    Some("window state".into()),
                    Some(serde_json::to_value(state).unwrap_or_default()),
                )
                .await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_window_set_size(call_id: String, args: Option<serde_json::Value>) {
    let width = args
        .as_ref()
        .and_then(|v| v.get("width"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1280)
        .clamp(480, 7680) as u32;
    let height = args
        .as_ref()
        .and_then(|v| v.get("height"))
        .and_then(|v| v.as_u64())
        .unwrap_or(900)
        .clamp(360, 4320) as u32;
    leptos::task::spawn_local(async move {
        match window_set_size(width, height).await {
            Ok(()) => {
                let _ = agent_submit_tool_result(
                    call_id,
                    true,
                    Some(format!("set window size to {width}x{height}")),
                    None,
                )
                .await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_window_set_fullscreen(call_id: String, args: Option<serde_json::Value>) {
    let enabled = args
        .as_ref()
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    leptos::task::spawn_local(async move {
        match window_set_fullscreen(enabled).await {
            Ok(()) => {
                let _ = agent_submit_tool_result(
                    call_id,
                    true,
                    Some(format!("fullscreen={enabled}")),
                    None,
                )
                .await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_memory_category_list(call_id: String, wb: WorkbenchService) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let categories: Vec<serde_json::Value> = ["memory", "learnings", "architecture"]
        .into_iter()
        .map(|key| {
            let settings = wb.memory_category_settings_for_workspace_untracked(ws_id, key);
            serde_json::json!({
                "category": key,
                "label": settings.label,
                "color": settings.color,
                "showInSidebar": settings.show_in_sidebar,
                "showInGraph": settings.show_in_graph,
            })
        })
        .collect();
    let body = serde_json::Value::Array(categories);
    submit_async(call_id, true, "listed memory categories".into(), Some(body));
}

fn handle_memory_category_update(
    call_id: String,
    args: Option<serde_json::Value>,
    wb: WorkbenchService,
) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let Some(category) = args
        .as_ref()
        .and_then(|v| v.get("category"))
        .and_then(|v| v.as_str())
    else {
        submit_async(call_id, false, "missing category".into(), None);
        return;
    };
    if category != "memory" && category != "learnings" && category != "architecture" {
        submit_async(
            call_id,
            false,
            "category must be memory, learnings, or architecture".into(),
            None,
        );
        return;
    }
    let mut settings = wb.memory_category_settings_for_workspace_untracked(ws_id, category);
    let fallback_color = settings.color.clone();
    if let Some(label) = args
        .as_ref()
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
    {
        settings.label = label.trim().to_owned();
    }
    if let Some(color) = args
        .as_ref()
        .and_then(|v| v.get("color"))
        .and_then(|v| v.as_str())
    {
        settings.color = normalize_hex_color(color, &fallback_color);
    }
    if let Some(show) = args
        .as_ref()
        .and_then(|v| v.get("showInSidebar"))
        .and_then(|v| v.as_bool())
    {
        settings.show_in_sidebar = show;
    }
    if let Some(show) = args
        .as_ref()
        .and_then(|v| v.get("showInGraph"))
        .and_then(|v| v.as_bool())
    {
        settings.show_in_graph = show;
    }
    wb.set_memory_category_settings(ws_id, category, settings.clone());
    let data = serde_json::json!({
        "category": category,
        "label": settings.label,
        "color": settings.color,
        "showInSidebar": settings.show_in_sidebar,
        "showInGraph": settings.show_in_graph,
    });
    submit_async(
        call_id,
        true,
        format!("updated category {category}"),
        Some(data),
    );
}

fn handle_memory_context_list(call_id: String, wb: WorkbenchService) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let items = wb.agent_context_for_workspace_untracked(ws_id);
    let body = serde_json::to_value(&items).unwrap_or(serde_json::Value::Array(vec![]));
    let summary = format!("{} context item(s)", items.len());
    submit_async(call_id, true, summary, Some(body));
}

fn handle_memory_context_detach(
    call_id: String,
    args: Option<serde_json::Value>,
    wb: WorkbenchService,
) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let Some(id) = args
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
    else {
        submit_async(call_id, false, "missing id".into(), None);
        return;
    };
    wb.remove_workspace_agent_context(ws_id, id);
    submit_async(call_id, true, format!("detached context {id}"), None);
}

fn handle_image_context_list(call_id: String, wb: WorkbenchService) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let items = wb.agent_images_for_workspace_untracked(ws_id);
    let body = serde_json::Value::Array(
        items
            .iter()
            .map(|image| {
                serde_json::json!({
                    "id": image.item.id,
                    "label": image.item.label,
                    "mime": image.item.mime,
                    "sizeBytes": image.item.size_bytes,
                    "addedAt": image.item.added_at,
                    "status": match &image.status {
                        crate::workbench::AgentImageContextStatus::Pending => "pending",
                        crate::workbench::AgentImageContextStatus::Read => "read",
                    },
                })
            })
            .collect(),
    );
    submit_async(
        call_id,
        true,
        format!("{} image context item(s)", items.len()),
        Some(body),
    );
}

fn handle_image_context_detach(
    call_id: String,
    args: Option<serde_json::Value>,
    wb: WorkbenchService,
) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let Some(id) = args
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
    else {
        submit_async(call_id, false, "missing id".into(), None);
        return;
    };
    wb.remove_workspace_agent_image(ws_id, id);
    submit_async(call_id, true, format!("detached image context {id}"), None);
}

fn handle_memory_context_attach(
    call_id: String,
    args: Option<serde_json::Value>,
    wb: WorkbenchService,
) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let Some(kind_raw) = args
        .as_ref()
        .and_then(|v| v.get("kind"))
        .and_then(|v| v.as_str())
    else {
        submit_async(call_id, false, "missing kind".into(), None);
        return;
    };
    let kind = match kind_raw {
        "memory_category" => AgentContextKind::MemoryCategory,
        "learning_category" => AgentContextKind::LearningCategory,
        "architecture_category" => AgentContextKind::MemoryCategory,
        "memory_note" => AgentContextKind::MemoryNote,
        "learning_note" => AgentContextKind::LearningNote,
        "terminal_session" => AgentContextKind::TerminalSession,
        other => {
            submit_async(call_id, false, format!("invalid kind: {other}"), None);
            return;
        }
    };
    let label_override = args
        .as_ref()
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let is_category = matches!(
        kind,
        AgentContextKind::MemoryCategory | AgentContextKind::LearningCategory
    );
    if is_category {
        let category = match &kind {
            AgentContextKind::LearningCategory => "learnings",
            _ if kind_raw == "architecture_category" => "architecture",
            _ => "memory",
        };
        let settings = wb.memory_category_settings_for_workspace_untracked(ws_id, category);
        let label = label_override.unwrap_or_else(|| settings.label.clone());
        let Some(cwd) = wb.default_workspace_cwd() else {
            submit_async(call_id, false, "workspace has no folder".into(), None);
            return;
        };
        leptos::task::spawn_local(async move {
            match memory_list(&cwd).await {
                Ok(resp) => {
                    let paths: Vec<String> = resp
                        .notes
                        .into_iter()
                        .filter(|n| {
                            if category == "learnings" {
                                n.path.starts_with(LEARNINGS_PREFIX)
                            } else if category == "architecture" {
                                n.category == "architecture"
                            } else {
                                !n.path.starts_with(LEARNINGS_PREFIX)
                                    && n.category != "architecture"
                            }
                        })
                        .map(|n| n.path)
                        .collect();
                    let count = paths.len();
                    let item = AgentContextItem {
                        id: format!("memory-category:{category}"),
                        kind,
                        label,
                        source: format!("{count} memory paths"),
                        paths,
                        added_at: Date::now() as i64,
                        content: None,
                    };
                    wb.upsert_workspace_agent_context(ws_id, item.clone());
                    let _ = agent_submit_tool_result(
                        call_id,
                        true,
                        Some(format!("attached {category} ({count} paths)")),
                        Some(serde_json::to_value(&item).unwrap_or_default()),
                    )
                    .await;
                }
                Err(e) => {
                    let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
                }
            }
        });
        return;
    }

    let Some(path) = args
        .as_ref()
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str())
    else {
        submit_async(call_id, false, "missing path for note context".into(), None);
        return;
    };
    let label = label_override.unwrap_or_else(|| {
        path.rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".md")
            .to_owned()
    });
    let item = AgentContextItem {
        id: format!("memory-note:{path}"),
        kind,
        label,
        source: path.to_owned(),
        paths: vec![path.to_owned()],
        added_at: Date::now() as i64,
        content: None,
    };
    wb.upsert_workspace_agent_context(ws_id, item.clone());
    submit_async(
        call_id,
        true,
        format!("attached note {path}"),
        Some(serde_json::to_value(&item).unwrap_or_default()),
    );
}

fn submit_async(call_id: String, ok: bool, message: String, data: Option<serde_json::Value>) {
    leptos::task::spawn_local(async move {
        let _ = agent_submit_tool_result(call_id, ok, Some(message), data).await;
    });
}

/// `harness.ask_user` is rendered as a chat bubble (see `ask_user_card`). This
/// handler only validates the payload — the bubble itself submits the user's
/// answer (or cancellation) via `agent_submit_tool_result`. Malformed payloads
/// short-circuit with `ok=false` so the agent loop doesn't hang.
fn handle_ask_user(call_id: String, args: Option<serde_json::Value>) {
    let Some(args) = args else {
        submit_async(call_id, false, "missing args".into(), None);
        return;
    };
    if crate::workbench::agent_panel::timeline::parse_ask_user_args(&args).is_none() {
        submit_async(
            call_id,
            false,
            "invalid ask_user args: need question + 2–4 options with labels".into(),
            None,
        );
    }
    // Valid: do nothing here — the AskUserCard owns the call_id and will
    // submit when the user answers or dismisses the card.
}

fn handle_open_terminal(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    // Resolve count (default 1, clamped 1..=16).
    let count = args
        .as_ref()
        .and_then(|v| v.get("count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 16) as usize;

    // Resolve per-slot agent slugs. `agentSlugs` (array) takes precedence;
    // otherwise `agentSlug` (string) applies to every slot; otherwise plain.
    let slugs_array = args
        .as_ref()
        .and_then(|v| v.get("agentSlugs"))
        .and_then(|v| v.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| e.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        });
    let single_slug = args
        .as_ref()
        .and_then(|v| v.get("agentSlug"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let slugs: Vec<String> = match slugs_array {
        Some(arr) => {
            if arr.len() != count {
                submit_async(
                    call_id,
                    false,
                    format!(
                        "agentSlugs length {} does not match count {}",
                        arr.len(),
                        count
                    ),
                    None,
                );
                return;
            }
            arr
        }
        None => match single_slug.as_ref() {
            Some(s) => vec![s.clone(); count],
            None => vec![String::new(); count],
        },
    };

    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };

    match wb.append_terminal_slots(workspace_id, slugs.clone()) {
        Ok(slot_ids) => {
            let summary = if count == 1 {
                let suffix = slugs
                    .first()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(" with agent={s}"))
                    .unwrap_or_default();
                format!("opened terminal slot {}{}", slot_ids[0], suffix)
            } else {
                let agent_note = match (slugs_array_is_uniform(&slugs), single_slug.as_ref()) {
                    (Some(s), _) if !s.is_empty() => format!(" with agent={s}"),
                    _ => String::new(),
                };
                format!("opened {} terminal slot(s){}", slot_ids.len(), agent_note)
            };
            let wait_slot_ids = slot_ids.clone();
            let data = serde_json::json!({ "slotIds": slot_ids });
            leptos::task::spawn_local(async move {
                let _ = wait_for_slots(wb, workspace_id, &wait_slot_ids).await;
                let _ = agent_submit_tool_result(call_id, true, Some(summary), Some(data)).await;
            });
        }
        Err(e) => submit_async(call_id, false, e, None),
    }
}

/// If every entry in `slugs` matches (and is non-empty), return that value.
fn slugs_array_is_uniform(slugs: &[String]) -> Option<&str> {
    let first = slugs.first()?;
    if first.is_empty() {
        return None;
    }
    if slugs.iter().all(|s| s == first) {
        Some(first)
    } else {
        None
    }
}

fn handle_create_workspace(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let title = args
        .as_ref()
        .and_then(|v| v.get("title"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let cwd = args
        .as_ref()
        .and_then(|v| v.get("cwd"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let terminal_count = args
        .as_ref()
        .and_then(|v| v.get("terminalCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 16) as u8;
    let agent_slugs = args
        .as_ref()
        .and_then(|v| v.get("agentSlugs"))
        .and_then(|v| v.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    match wb.create_workspace(title, cwd, terminal_count, agent_slugs) {
        Ok(workspace_id) => {
            let data = wb.workspaces().with_untracked(|workspaces| {
                workspaces
                    .iter()
                    .find(|workspace| workspace.id == workspace_id)
                    .map(|workspace| {
                        serde_json::json!({
                            "workspaceId": workspace.id,
                            "title": workspace.title,
                            "cwd": workspace.cwd,
                            "terminalCount": workspace.terminal_count,
                        })
                    })
            });
            submit_async(
                call_id,
                true,
                format!("created workspace {workspace_id} with {terminal_count} terminal(s)"),
                data,
            );
        }
        Err(err) => submit_async(call_id, false, err, None),
    }
}

fn resolve_target_session(
    wb: &WorkbenchService,
    workspace_id: u64,
    args: &Option<serde_json::Value>,
) -> Result<(u64, u64), String> {
    let slot_filter = args
        .as_ref()
        .and_then(|v| v.get("slotId"))
        .and_then(|v| v.as_u64());
    let name_filter = args
        .as_ref()
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let agent_slug = args
        .as_ref()
        .and_then(|v| v.get("agentSlug"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let entries = wb.pty_sessions_for_workspace(workspace_id);
    if entries.is_empty() {
        return Err("no running terminal sessions in this workspace".into());
    }

    let label_for_slot = |slot_id: u64| -> Option<String> {
        wb.workspaces().with_untracked(|ws| {
            ws.iter().find(|w| w.id == workspace_id).and_then(|w| {
                w.slot_ids
                    .iter()
                    .position(|id| *id == slot_id)
                    .and_then(|idx| w.slot_agent_labels.get(idx).cloned())
            })
        })
    };

    if let Some(slot) = slot_filter {
        if let Some((sid, pane)) = entries
            .iter()
            .find(|(s, _, _)| *s == slot)
            .map(|(_, p, sid)| (*sid, *p))
        {
            return Ok((sid, pane));
        }
        return Err(format!("slot {slot} not running"));
    }
    if let Some(name) = name_filter {
        let pool = current_name_pool();
        let siblings = wb.slot_ids_for_workspace(workspace_id);
        let target = name.to_lowercase();
        for (slot, pane, sid) in &entries {
            let resolved = resolved_slot_name(wb, workspace_id, *slot, &pool, &siblings);
            if resolved
                .as_deref()
                .map(|n| n.to_lowercase() == target)
                .unwrap_or(false)
            {
                return Ok((*sid, *pane));
            }
        }
        return Err(format!("no running terminal named '{name}'"));
    }
    if let Some(slug) = agent_slug {
        for (slot, pane, sid) in &entries {
            if label_for_slot(*slot).as_deref() == Some(slug.as_str()) {
                return Ok((*sid, *pane));
            }
        }
        return Err(format!("no running slot with agent={slug}"));
    }
    let (_, pane, sid) = entries[0];
    Ok((sid, pane))
}

fn slots_registered(wb: &WorkbenchService, workspace_id: u64, slot_ids: &[u64]) -> bool {
    let running = wb.pty_sessions_for_workspace(workspace_id);
    slot_ids
        .iter()
        .all(|slot_id| running.iter().any(|(slot, _, _)| slot == slot_id))
}

async fn wait_for_slots(wb: WorkbenchService, workspace_id: u64, slot_ids: &[u64]) -> bool {
    for _ in 0..PTY_READY_ATTEMPTS {
        if slots_registered(&wb, workspace_id, slot_ids) {
            return true;
        }
        TimeoutFuture::new(PTY_READY_DELAY_MS).await;
    }
    slots_registered(&wb, workspace_id, slot_ids)
}

async fn wait_for_target_session(
    wb: WorkbenchService,
    workspace_id: u64,
    args: &Option<serde_json::Value>,
) -> Result<(u64, u64), String> {
    let mut last_err = None;
    for _ in 0..PTY_READY_ATTEMPTS {
        match resolve_target_session(&wb, workspace_id, args) {
            Ok(session) => return Ok(session),
            Err(err) => last_err = Some(err),
        }
        TimeoutFuture::new(PTY_READY_DELAY_MS).await;
    }
    resolve_target_session(&wb, workspace_id, args)
        .or_else(|_| Err(last_err.unwrap_or_else(|| "terminal session not running".into())))
}

fn handle_list_terminals(call_id: String, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let running = wb.pty_sessions_for_workspace(workspace_id);
    let pool = current_name_pool();
    let mode = current_naming_mode();
    let entries = wb.workspaces().with_untracked(|ws| {
        let Some(w) = ws.iter().find(|w| w.id == workspace_id) else {
            return Vec::new();
        };
        let siblings = w.slot_ids.clone();
        w.slot_ids
            .iter()
            .enumerate()
            .map(|(idx, slot_id)| {
                let agent = w.slot_agent_labels.get(idx).cloned().unwrap_or_default();
                let running = running.iter().any(|(s, _, _)| *s == *slot_id);
                let name = resolved_slot_name(&wb, workspace_id, *slot_id, &pool, &siblings)
                    .unwrap_or_else(|| format!("#{slot_id}"));
                serde_json::json!({
                    "slotId": slot_id,
                    "name": name,
                    "namingMode": mode.storage_value(),
                    "agentSlug": agent,
                    "running": running,
                })
            })
            .collect::<Vec<_>>()
    });
    let body = serde_json::Value::Array(entries.clone());
    let summary = format!("{} slot(s) listed", entries.len());
    submit_async(call_id, true, summary, Some(body));
}

fn handle_send_keys(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let text = args
        .as_ref()
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let Some(text) = text else {
        submit_async(call_id, false, "missing text".into(), None);
        return;
    };
    let submit = args
        .as_ref()
        .and_then(|v| v.get("submit"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut payload = text.clone();
    if submit {
        payload.push('\r');
    }
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
    leptos::task::spawn_local(async move {
        let (sid, _pane) = match wait_for_target_session(wb, workspace_id, &args).await {
            Ok(t) => t,
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
                return;
            }
        };
        match pty_write(sid, b64).await {
            Ok(()) => {
                let msg = format!(
                    "wrote {} byte(s) to session {sid}{}",
                    text.len(),
                    if submit { " (submitted)" } else { "" }
                );
                let _ = agent_submit_tool_result(call_id, true, Some(msg), None).await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_read_output(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let max_bytes = args
        .as_ref()
        .and_then(|v| v.get("maxBytes"))
        .and_then(|v| v.as_u64())
        .unwrap_or(4096)
        .min(65536) as usize;
    let (sid, _pane) = match resolve_target_session(&wb, workspace_id, &args) {
        Ok(t) => t,
        Err(e) => {
            submit_async(call_id, false, e, None);
            return;
        }
    };
    leptos::task::spawn_local(async move {
        match pty_peek_output(sid, max_bytes).await {
            Ok(text) => {
                let len = text.len();
                let _ = agent_submit_tool_result(
                    call_id,
                    true,
                    Some(text),
                    Some(serde_json::json!({ "bytes": len, "sessionId": sid })),
                )
                .await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_wait_output(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let after_seq = args
        .as_ref()
        .and_then(|v| v.get("afterSeq"))
        .and_then(|v| v.as_u64());
    let timeout_ms = args
        .as_ref()
        .and_then(|v| v.get("timeoutMs"))
        .and_then(|v| v.as_u64())
        .map(|ms| ms.clamp(1, 120_000));
    let idle_ms = args
        .as_ref()
        .and_then(|v| v.get("idleMs"))
        .and_then(|v| v.as_u64())
        .map(|ms| ms.min(30_000));
    let max_bytes = args
        .as_ref()
        .and_then(|v| v.get("maxBytes"))
        .and_then(|v| v.as_u64())
        .map(|bytes| bytes.clamp(1, 65_536) as usize);
    let contains = args
        .as_ref()
        .and_then(|v| v.get("contains"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .filter(|s| !s.is_empty());

    leptos::task::spawn_local(async move {
        let (sid, _pane) = match wait_for_target_session(wb, workspace_id, &args).await {
            Ok(t) => t,
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
                return;
            }
        };
        match pty_wait_output(sid, after_seq, timeout_ms, idle_ms, max_bytes, contains).await {
            Ok(snapshot) => {
                let summary = if snapshot.timed_out {
                    format!(
                        "timed out waiting for terminal output at seq {} ({} byte(s))",
                        snapshot.seq, snapshot.bytes
                    )
                } else {
                    format!(
                        "observed terminal output at seq {} ({} byte(s))",
                        snapshot.seq, snapshot.bytes
                    )
                };
                let data = serde_json::json!({
                    "sessionId": snapshot.session_id,
                    "seq": snapshot.seq,
                    "bytes": snapshot.bytes,
                    "text": snapshot.text,
                    "timedOut": snapshot.timed_out,
                    "lastOutputMs": snapshot.last_output_ms,
                });
                let _ = agent_submit_tool_result(call_id, true, Some(summary), Some(data)).await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_terminal_interrupt(
    call_id: String,
    args: Option<serde_json::Value>,
    wb: WorkbenchService,
) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    leptos::task::spawn_local(async move {
        let (sid, _pane) = match wait_for_target_session(wb, workspace_id, &args).await {
            Ok(t) => t,
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
                return;
            }
        };
        use base64::Engine;
        let ctrl_c = base64::engine::general_purpose::STANDARD.encode([0x03]);
        match pty_write(sid, ctrl_c).await {
            Ok(()) => {
                let _ = agent_submit_tool_result(
                    call_id,
                    true,
                    Some(format!("sent Ctrl+C to terminal session {sid}")),
                    Some(serde_json::json!({ "sessionId": sid })),
                )
                .await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_send_agent_context(
    call_id: String,
    args: Option<serde_json::Value>,
    wb: WorkbenchService,
) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };

    let include_memory;
    let include_images;
    let include_plans;
    let include_tasks;
    match args
        .as_ref()
        .and_then(|v| v.get("includeKinds"))
        .and_then(|v| v.as_array())
    {
        Some(arr) => {
            let mut mem = false;
            let mut img = false;
            let mut pl = false;
            let mut tk = false;
            for entry in arr {
                if let Some(s) = entry.as_str() {
                    match s {
                        "memory" => mem = true,
                        "images" => img = true,
                        "plans" => pl = true,
                        "tasks" => tk = true,
                        _ => {}
                    }
                }
            }
            include_memory = mem;
            include_images = img;
            include_plans = pl;
            include_tasks = tk;
        }
        None => {
            include_memory = true;
            include_images = true;
            include_plans = true;
            include_tasks = true;
        }
    }

    let instruction = args
        .as_ref()
        .and_then(|v| v.get("instruction"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let submit = args
        .as_ref()
        .and_then(|v| v.get("submit"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let workspace_root = wb.default_workspace_cwd();
    let args_for_target = args.clone();

    leptos::task::spawn_local(async move {
        let (sid, pane) = match wait_for_target_session(wb, workspace_id, &args_for_target).await {
            Ok(t) => t,
            Err(e) => {
                let hint = if matches!(e.as_str(), "no running terminal sessions in this workspace")
                    || e.starts_with("no running slot")
                    || e.starts_with("slot ")
                {
                    format!("{e} — try `harness.list_terminals` to inspect slots")
                } else {
                    e
                };
                let _ = agent_submit_tool_result(call_id, false, Some(hint), None).await;
                return;
            }
        };

        let (slot_id, agent_slug_str) = resolve_slot_label(&wb, workspace_id, sid);
        let target = WorkspaceTerminalTarget {
            slot_id: slot_id.unwrap_or(0),
            pane_id: pane,
            session_id: sid,
            agent_slug: agent_slug_str,
            label: String::new(),
        };

        let req = HandoffRequest {
            workspace_id,
            workspace_root,
            target,
            context_items: None,
            include_memory,
            include_images,
            include_plans,
            include_tasks,
            instruction,
            submit,
        };

        match perform_handoff(wb, req).await {
            Ok(outcome) => {
                let summary = format!(
                    "wrote {} byte(s) of context to session {sid}{}",
                    outcome.bytes_written,
                    if outcome.submitted {
                        " (submitted)"
                    } else {
                        ""
                    }
                );
                let data = serde_json::json!({
                    "sessionId": sid,
                    "submitted": outcome.submitted,
                    "bytes": outcome.bytes_written,
                    "manifestPath": outcome.manifest_path,
                    "imagesExported": outcome.images_exported,
                });
                let _ = agent_submit_tool_result(call_id, true, Some(summary), Some(data)).await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

/// Find `(slot_id, agent_slug)` for a live PTY session id in the workspace.
fn resolve_slot_label(
    wb: &WorkbenchService,
    workspace_id: u64,
    session_id: u64,
) -> (Option<u64>, String) {
    let sessions = wb.pty_sessions_for_workspace(workspace_id);
    let slot = sessions
        .iter()
        .find(|(_, _, sid)| *sid == session_id)
        .map(|(slot, _, _)| *slot);
    let agent = wb.workspaces().with_untracked(|ws| {
        ws.iter()
            .find(|w| w.id == workspace_id)
            .and_then(|w| {
                slot.and_then(|s| w.slot_ids.iter().position(|id| *id == s))
                    .and_then(|idx| w.slot_agent_labels.get(idx).cloned())
            })
            .unwrap_or_default()
    });
    (slot, agent)
}
