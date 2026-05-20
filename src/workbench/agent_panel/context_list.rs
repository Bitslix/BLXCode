use crate::agent_wire::AgentContextItem;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn ContextSection(context_open: RwSignal<bool>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();

    view! {
        <section class="agent-section agent-section--context" aria-labelledby="agent-context-title">
            <button
                type="button"
                class="agent-section__head agent-section__head--toggle"
                aria-expanded=move || context_open.get().to_string()
                aria-controls="agent-context-list"
                on:click=move |_| context_open.update(|open| *open = !*open)
            >
                <h3 id="agent-context-title">"Context"</h3>
                <span>
                    {move || {
                        let count = active_context_items(wb).len();
                        if count == 0 {
                            "Empty".to_string()
                        } else {
                            format!("{count} attached")
                        }
                    }}
                    <span class="agent-section__chev" aria-hidden="true">
                        {move || if context_open.get() { "⌃" } else { "⌄" }}
                    </span>
                </span>
            </button>
            <Show when=move || context_open.get()>
                <ol id="agent-context-list" class="agent-task-list agent-context-list">
                    {move || {
                        let items = active_context_items(wb);
                        if items.is_empty() {
                            return view! {
                                <li class="agent-task agent-task--empty">
                                    <div>
                                        <strong>"No context attached"</strong>
                                        <small>"Send Memory categories or notes here from the Memory sidebar."</small>
                                    </div>
                                </li>
                            }
                            .into_any();
                        }
                        view! {
                            <>
                                {items
                                    .into_iter()
                                    .map(|item| view! { <ContextRow item=item wb=wb /> })
                                    .collect_view()}
                            </>
                        }
                        .into_any()
                    }}
                </ol>
            </Show>
        </section>
    }
}

fn active_context_items(wb: WorkbenchService) -> Vec<AgentContextItem> {
    let Some(ws_id) = wb.active_id().get() else {
        return Vec::new();
    };
    wb.workspaces().with(|workspaces| {
        workspaces
            .iter()
            .find(|w| w.id == ws_id)
            .map(|w| w.agent_context_items.clone())
            .unwrap_or_default()
    })
}

#[component]
fn ContextRow(item: AgentContextItem, wb: WorkbenchService) -> impl IntoView {
    let id = item.id.clone();
    let label = item.label.clone();
    let source = item.source.clone();
    let count = item.paths.len();
    view! {
        <li class="agent-task agent-context-item">
            <span class="agent-task__mark agent-context-item__mark" aria-hidden="true"></span>
            <div class="agent-task__body">
                <div class="agent-task__topline">
                    <strong>{label}</strong>
                    <button
                        type="button"
                        class="agent-context-item__remove"
                        title="Remove context"
                        aria-label="Remove context"
                        on:click=move |_| {
                            if let Some(ws_id) = wb.active_id().get_untracked() {
                                wb.remove_workspace_agent_context(ws_id, &id);
                            }
                        }
                    >
                        <LxIcon icon=icondata::LuX width="0.74rem" height="0.74rem" />
                    </button>
                </div>
                <small>{if count > 1 { format!("{count} paths · {source}") } else { source }}</small>
            </div>
        </li>
    }
}
