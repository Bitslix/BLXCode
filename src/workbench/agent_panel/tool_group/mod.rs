//! Shared tool-call "pill" used by the main-agent model-round group **and** by
//! standalone tool rows (incl. subagent tool calls in the tree view). Pure
//! extraction + unification of the previously duplicated markup in
//! [`super::timeline`] — the existing decent/small/themed look is preserved 1:1
//! (icon · label · arg/×N · status · expandable path-list/detail). All styling
//! lives in the shared `.agent-tool-row*` rules in `styles.css`.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::workbench::agent_panel::timeline::{path_tail, tool_icon, ToolDetailContent};
use crate::workbench::agent_timeline::{ActivityStatus, ToolActivity};
use crate::workbench::WorkbenchService;

/// One tool-call pill. Renders the inner `.agent-tool-row` block (the clickable
/// head plus its optional expanded detail). Callers wrap it in whatever list
/// item / chat line they need.
///
/// Handles all variants in one place: single arg-summary, grouped `×N` count
/// (`merged_count > 1`), a clickable workspace-path list (file-reading tools),
/// and the structured/raw detail body.
#[component]
pub fn ToolPill(
    tool: ToolActivity,
    detail_key: String,
    tool_detail_open: RwSignal<HashMap<String, bool>>,
    wb: WorkbenchService,
    workspace_id: Option<u64>,
) -> impl IntoView {
    let status_class = match tool.status {
        ActivityStatus::Ok => "agent-tool-row--ok",
        ActivityStatus::Fail => "agent-tool-row--fail",
        ActivityStatus::Pending => "agent-tool-row--pending",
    };
    let status_icon = match tool.status {
        ActivityStatus::Ok => icondata::LuCheck,
        ActivityStatus::Fail => icondata::LuTriangleAlert,
        ActivityStatus::Pending => icondata::LuLoader,
    };
    let tool_icon_val = tool_icon(&tool.tool);
    let label = tool.label.clone();
    // Grouped calls show "×N" instead of the single-call arg summary.
    let merged_count = tool.merged_count;
    let summary = if merged_count > 1 {
        String::new()
    } else {
        tool.args_summary.clone()
    };
    let count_badge = if merged_count > 1 {
        format!("×{merged_count}")
    } else {
        String::new()
    };
    let tool_name = tool.tool.clone();
    let has_paths = !tool.paths.is_empty();
    let has_detail = has_paths || tool.detail.as_ref().is_some_and(|s| !s.is_empty());
    let detail_text = tool.detail.clone().unwrap_or_default();
    let paths_sv = StoredValue::new(tool.paths.clone());
    let detail_key_memo = detail_key.clone();
    let detail_open = Memo::new(move |_| {
        tool_detail_open.with(|m| m.get(&detail_key_memo).copied().unwrap_or(false))
    });

    view! {
        <div class=format!("agent-tool-row {status_class}") title=tool_name>
            <button
                type="button"
                class="agent-tool-row__head"
                aria-expanded=move || detail_open.get().to_string()
                prop:disabled=move || !has_detail
                on:click=move |_| {
                    if has_detail {
                        let key = detail_key.clone();
                        tool_detail_open.update(|m| {
                            let cur = m.get(&key).copied().unwrap_or(false);
                            m.insert(key, !cur);
                        });
                    }
                }
            >
                <span class="agent-tool-row__icon" aria-hidden="true">
                    <LxIcon icon=tool_icon_val width="0.82rem" height="0.82rem" />
                </span>
                <span class="agent-tool-row__label">{label}</span>
                <Show when={let s = summary.clone(); move || !s.is_empty()}>
                    <span class="agent-tool-row__arg">{summary.clone()}</span>
                </Show>
                <Show when={let b = count_badge.clone(); move || !b.is_empty()}>
                    <span class="agent-tool-row__count">{count_badge.clone()}</span>
                </Show>
                <span class="agent-tool-row__status" aria-hidden="true">
                    <LxIcon icon=status_icon width="0.78rem" height="0.78rem" />
                </span>
                <Show when=move || has_detail>
                    <span
                        class="agent-tool-row__chevron"
                        class:agent-tool-row__chevron--open=move || detail_open.get()
                        aria-hidden="true"
                    >
                        <LxIcon icon=icondata::LuChevronDown width="0.82rem" height="0.82rem" />
                    </span>
                </Show>
            </button>
            {move || {
                if !has_detail || !detail_open.get() {
                    return view! { <></> }.into_any();
                }
                if has_paths {
                    view! {
                        <ul class="tool-row-paths">
                            {paths_sv.get_value().into_iter().map(|p| {
                                let display = path_tail(&p);
                                let p_open = p.clone();
                                view! {
                                    <li>
                                        <button
                                            type="button"
                                            class="tool-row-path-btn"
                                            title=p.clone()
                                            on:click=move |_| {
                                                if let Some(ws_id) = workspace_id {
                                                    wb.open_center_file_tab(ws_id, p_open.clone());
                                                }
                                            }
                                        >{display}</button>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                } else {
                    view! { <ToolDetailContent detail=detail_text.clone() /> }.into_any()
                }
            }}
        </div>
    }
}
