use crate::agent_wire::AgentContextItem;
use crate::workbench::{AgentImageContextStatus, WorkbenchService, WorkspaceAgentImage};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

#[component]
pub fn ContextSection(context_open: RwSignal<bool>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let preview = RwSignal::new(None::<WorkspaceAgentImage>);

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
                        let count = count + wb.active_agent_images().len();
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
                        let images = wb.active_agent_images();
                        if items.is_empty() && images.is_empty() {
                            return view! {
                                <li class="agent-task agent-task--empty">
                                    <div>
                                        <strong>"No context attached"</strong>
                                        <small>"Send Memory categories or notes here, or drop/paste images into the Agent panel."</small>
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
                                {images
                                    .into_iter()
                                    .map(|image| view! { <ImageContextRow image=image wb=wb preview=preview /> })
                                    .collect_view()}
                            </>
                        }
                        .into_any()
                    }}
                </ol>
            </Show>
        </section>
        <ImagePreviewDialog preview=preview wb=wb />
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

#[component]
fn ImageContextRow(
    image: WorkspaceAgentImage,
    wb: WorkbenchService,
    preview: RwSignal<Option<WorkspaceAgentImage>>,
) -> impl IntoView {
    let id = image.item.id.clone();
    let id_for_use = id.clone();
    let id_for_remove = id.clone();
    let label = image.item.label.clone();
    let source = format!(
        "{} · {}",
        image.item.mime,
        format_image_size(image.item.size_bytes)
    );
    let is_read = image.status == AgentImageContextStatus::Read;
    let image_for_preview = image.clone();
    let use_again = if is_read {
        view! {
            <button
                type="button"
                class="agent-context-item__remove"
                title="Use image again"
                aria-label="Use image again"
                on:click=move |_| {
                    if let Some(ws_id) = wb.active_id().get_untracked() {
                        wb.reactivate_workspace_agent_image(ws_id, &id_for_use);
                    }
                }
            >
                <LxIcon icon=icondata::LuRotateCcw width="0.74rem" height="0.74rem" />
            </button>
        }
        .into_any()
    } else {
        view! {}.into_any()
    };
    view! {
        <li class="agent-task agent-context-item agent-context-image">
            <span class="agent-task__mark agent-context-item__mark agent-context-image__mark" aria-hidden="true"></span>
            <div class="agent-task__body">
                <button
                    type="button"
                    class="agent-context-image__open"
                    on:click=move |_| preview.set(Some(image_for_preview.clone()))
                >
                    <span class="agent-task__topline">
                        <strong>{label}</strong>
                        <span class=move || {
                            if is_read {
                                "agent-task__status agent-context-image__status agent-context-image__status--read"
                            } else {
                                "agent-task__status agent-context-image__status agent-context-image__status--pending"
                            }
                        }>
                            {if is_read { "Read" } else { "Pending" }}
                        </span>
                    </span>
                    <small>{source}</small>
                </button>
                <div class="agent-context-image__actions">
                    {use_again}
                    <button
                        type="button"
                        class="agent-context-item__remove"
                        title="Remove image"
                        aria-label="Remove image"
                        on:click=move |_| {
                            if let Some(ws_id) = wb.active_id().get_untracked() {
                                wb.remove_workspace_agent_image(ws_id, &id_for_remove);
                            }
                        }
                    >
                        <LxIcon icon=icondata::LuX width="0.74rem" height="0.74rem" />
                    </button>
                </div>
            </div>
        </li>
    }
}

#[component]
fn ImagePreviewDialog(
    preview: RwSignal<Option<WorkspaceAgentImage>>,
    wb: WorkbenchService,
) -> impl IntoView {
    if let Some(window) = web_sys::window() {
        let cb: Closure<dyn FnMut(KeyboardEvent)> = Closure::new(move |ev: KeyboardEvent| {
            if ev.key() == "Escape" {
                preview.set(None);
            }
        });
        let _ = window.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    view! {
        <Show when=move || preview.get().is_some()>
            {move || {
                let Some(image) = preview.get() else {
                    return view! {}.into_any();
                };
                let id = image.item.id.clone();
                let id_for_use = id.clone();
                let id_for_remove = id.clone();
                let title = image.item.label.clone();
                let mime = image.item.mime.clone();
                let size = format_image_size(image.item.size_bytes);
                let data_url = image.data_url();
                let is_read = image.status == AgentImageContextStatus::Read;
                let use_again = if is_read {
                    view! {
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| {
                                if let Some(ws_id) = wb.active_id().get_untracked() {
                                    wb.reactivate_workspace_agent_image(ws_id, &id_for_use);
                                }
                                preview.set(None);
                            }
                        >
                            <LxIcon icon=icondata::LuRotateCcw width="0.9rem" height="0.9rem" />
                            <span>"Use again"</span>
                        </button>
                    }
                    .into_any()
                } else {
                    view! {}.into_any()
                };
                view! {
                    <div class="agent-image-preview-backdrop" on:click=move |_| preview.set(None)>
                        <div
                            class="agent-image-preview-dialog"
                            role="dialog"
                            aria-modal="true"
                            aria-label="Image preview"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            <header class="agent-image-preview__head">
                                <div>
                                    <h3>{title}</h3>
                                    <p>{format!("{mime} · {size} · {}", if is_read { "Read" } else { "Pending" })}</p>
                                </div>
                                <button
                                    type="button"
                                    class="agent-context-item__remove"
                                    aria-label="Close preview"
                                    on:click=move |_| preview.set(None)
                                >
                                    <LxIcon icon=icondata::LuX width="0.9rem" height="0.9rem" />
                                </button>
                            </header>
                            <div class="agent-image-preview__stage">
                                <img src=data_url alt="Attached image preview" />
                            </div>
                            <footer class="agent-image-preview__actions">
                                {use_again}
                                <button
                                    type="button"
                                    class="workbench-mini-btn"
                                    on:click=move |_| preview.set(None)
                                >
                                    <span>"Close"</span>
                                </button>
                                <button
                                    type="button"
                                    class="workbench-mini-btn agent-image-preview__remove"
                                    on:click=move |_| {
                                        if let Some(ws_id) = wb.active_id().get_untracked() {
                                            wb.remove_workspace_agent_image(ws_id, &id_for_remove);
                                        }
                                        preview.set(None);
                                    }
                                >
                                    <LxIcon icon=icondata::LuTrash2 width="0.9rem" height="0.9rem" />
                                    <span>"Remove"</span>
                                </button>
                            </footer>
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>
    }
}

fn format_image_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
