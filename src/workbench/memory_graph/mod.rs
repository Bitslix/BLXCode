mod graph_glue;

use crate::i18n::I18nKey;
use crate::open_http::{dom_click_nav_href, DomNavHref};
use crate::service::I18nService;
use crate::tauri_bridge::{self, GraphData, NoteContent};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::memory_graph::graph_glue::{
    ensure_graph3d_script, graph3d_create, graph3d_dispose, graph3d_fly_to_node,
    graph3d_reset_view, graph3d_resize, graph3d_set_data, graph3d_zoom,
};
use crate::workbench::agent_context_handoff::HandoffMenu;
use crate::workbench::memory_panel::{
    expand_files_group_for_path, load_note, refresh_graph, MemoryState, MemoryView,
};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use leptos::html;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphMode {
    ThreeD,
    TwoD,
}

impl GraphMode {
    fn from_storage() -> Self {
        let raw = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| {
                s.get_item(crate::config::GRAPH_MODE_STORAGE_KEY)
                    .ok()
                    .flatten()
            });
        match raw.as_deref() {
            Some("2d") => Self::TwoD,
            _ => Self::ThreeD,
        }
    }

    fn persist(self) {
        let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
            return;
        };
        let value = match self {
            Self::ThreeD => "3d",
            Self::TwoD => "2d",
        };
        let _ = storage.set_item(crate::config::GRAPH_MODE_STORAGE_KEY, value);
    }

    fn toggle(self) -> Self {
        match self {
            Self::ThreeD => Self::TwoD,
            Self::TwoD => Self::ThreeD,
        }
    }
}

#[derive(Clone, Copy)]
struct GraphPreviewState {
    open: RwSignal<bool>,
    path: RwSignal<Option<String>>,
    label: RwSignal<String>,
    content: RwSignal<String>,
    loading: RwSignal<bool>,
}

impl GraphPreviewState {
    fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            path: RwSignal::new(None),
            label: RwSignal::new(String::new()),
            content: RwSignal::new(String::new()),
            loading: RwSignal::new(false),
        }
    }
}

#[component]
pub fn MemoryGraphView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let mode = RwSignal::new(GraphMode::from_storage());
    let load_failed = RwSignal::new(false);
    let reset_tick = RwSignal::new(0_u32);
    let zoom_tick = RwSignal::new(0_i32);
    let preview = GraphPreviewState::new();

    Effect::new({
        let state = state.clone();
        move |_| {
            if state.view.get() != MemoryView::Graph {
                return;
            }
            let Some(ws) = state.workspace_cwd.get() else {
                return;
            };
            refresh_graph(state.clone(), ws);
        }
    });

    Effect::new(move |_| {
        let next = mode.get();
        next.persist();
        if next == GraphMode::ThreeD {
            load_failed.set(false);
        }
    });

    Effect::new({
        let state = state.clone();
        move |_| {
            if state.graph_prefer_3d.get_untracked() {
                mode.set(GraphMode::ThreeD);
                load_failed.set(false);
                state.graph_prefer_3d.set(false);
            }
        }
    });

    let open_preview = Callback::new({
        let state = state.clone();
        move |path: String| {
            open_graph_preview(state.clone(), preview, path);
        }
    });

    view! {
        <div class="workbench-memory-graph">
            <Show
                when={
                    let s = state.clone();
                    move || s.graph.get().as_ref().is_some_and(|g| !g.nodes.is_empty())
                }
                fallback={
                    let i = i18n.clone();
                    move || view! {
                        <p class="workbench-memory-graph__empty">{move || i.tr(I18nKey::MemGraphEmpty)()}</p>
                    }
                }
            >
                <GraphToolbar mode=mode reset_tick=reset_tick zoom_tick=zoom_tick />
                <div
                    class="workbench-memory-graph__canvas"
                    class:workbench-memory-graph__canvas--preview=move || preview.open.get()
                >
                    <Show
                        when=move || mode.get() == GraphMode::ThreeD && !load_failed.get()
                        fallback={
                            let state = state.clone();
                            let i18n = i18n.clone();
                            move || view! {
                                {move || load_failed.get().then(|| view! {
                                    <p class="workbench-memory-graph__warning">
                                        {move || i18n.tr(I18nKey::MemGraph3dLoadFailed)()}
                                    </p>
                                })}
                                <Graph2dView
                                    state=state.clone()
                                    wb=wb
                                reset_tick=reset_tick
                                zoom_tick=zoom_tick
                                open_preview=open_preview
                            />
                        }
                    }
                >
                    <Graph3dView
                        state=state.clone()
                        wb=wb
                        reset_tick=reset_tick
                        zoom_tick=zoom_tick
                        open_preview=open_preview
                        load_failed=load_failed
                        preview_open=preview.open
                    />
                </Show>
                    <GraphPreviewPopover state=state preview=preview open_preview=open_preview />
                </div>
                <p class="workbench-memory-graph__legend">
                    {move || i18n.tr(I18nKey::MemGraphLegend)()}
                </p>
            </Show>
        </div>
    }
}

#[component]
fn GraphToolbar(
    mode: RwSignal<GraphMode>,
    reset_tick: RwSignal<u32>,
    zoom_tick: RwSignal<i32>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="workbench-memory-graph__toolbar" role="toolbar">
            <button
                type="button"
                class="workbench-memory-graph__btn"
                title=move || i18n.tr(I18nKey::MemGraphReset)()
                aria-label=move || i18n.tr(I18nKey::MemGraphReset)()
                on:click=move |_| reset_tick.update(|n| *n = n.wrapping_add(1))
            >
                <LxIcon icon=icondata::LuRefreshCw width="0.86rem" height="0.86rem" />
            </button>
            <button
                type="button"
                class="workbench-memory-graph__btn"
                title=move || i18n.tr(I18nKey::MemGraphZoomIn)()
                aria-label=move || i18n.tr(I18nKey::MemGraphZoomIn)()
                on:click=move |_| zoom_tick.update(|n| *n += 1)
            >
                <LxIcon icon=icondata::LuZoomIn width="0.86rem" height="0.86rem" />
            </button>
            <button
                type="button"
                class="workbench-memory-graph__btn"
                title=move || i18n.tr(I18nKey::MemGraphZoomOut)()
                aria-label=move || i18n.tr(I18nKey::MemGraphZoomOut)()
                on:click=move |_| zoom_tick.update(|n| *n -= 1)
            >
                <LxIcon icon=icondata::LuZoomOut width="0.86rem" height="0.86rem" />
            </button>
            <button
                type="button"
                class="workbench-memory-graph__btn"
                class:workbench-memory-graph__btn--active=move || mode.get() == GraphMode::ThreeD
                title=move || {
                    if mode.get() == GraphMode::ThreeD {
                        i18n.tr(I18nKey::MemGraphMode2d)()
                    } else {
                        i18n.tr(I18nKey::MemGraphMode3d)()
                    }
                }
                aria-label=move || {
                    if mode.get() == GraphMode::ThreeD {
                        i18n.tr(I18nKey::MemGraphMode2d)()
                    } else {
                        i18n.tr(I18nKey::MemGraphMode3d)()
                    }
                }
                on:click=move |_| mode.update(|m| *m = m.toggle())
            >
                {move || {
                    if mode.get() == GraphMode::ThreeD {
                        view! { <LxIcon icon=icondata::LuBox width="0.86rem" height="0.86rem" /> }.into_any()
                    } else {
                        view! { <LxIcon icon=icondata::LuSquare width="0.86rem" height="0.86rem" /> }.into_any()
                    }
                }}
            </button>
        </div>
    }
}

#[component]
fn Graph3dView(
    state: MemoryState,
    wb: WorkbenchService,
    reset_tick: RwSignal<u32>,
    zoom_tick: RwSignal<i32>,
    open_preview: Callback<String>,
    load_failed: RwSignal<bool>,
    preview_open: RwSignal<bool>,
) -> impl IntoView {
    let node_ref = NodeRef::<html::Div>::new();
    let graph_id = RwSignal::new(None::<f64>);
    let bootstrap_inflight = RwSignal::new(false);
    let disposed = RwSignal::new(false);
    let last_zoom_tick = RwSignal::new(zoom_tick.get_untracked());
    let last_reset_tick = RwSignal::new(reset_tick.get_untracked());

    Effect::new({
        let state = state.clone();
        move |_| {
            let Some(graph) = configured_graph(wb, state.graph.get()) else {
                return;
            };
            if graph_id.get_untracked().is_some() || bootstrap_inflight.get_untracked() {
                return;
            }
            let Some(el) = node_ref.get() else {
                return;
            };
            let Ok(container) = el.dyn_into::<HtmlElement>() else {
                load_failed.set(true);
                return;
            };
            bootstrap_inflight.set(true);
            spawn_local(async move {
                let result = async {
                    ensure_graph3d_script().await?;
                    let id = graph3d_create(&container)?;
                    graph3d_set_data(id, &graph)?;
                    graph3d_resize(id);
                    Ok::<f64, String>(id)
                }
                .await;
                bootstrap_inflight.set(false);
                match result {
                    Ok(id) => {
                        if disposed.get_untracked() {
                            graph3d_dispose(id);
                        } else {
                            graph_id.set(Some(id));
                            load_failed.set(false);
                        }
                    }
                    Err(_) => load_failed.set(true),
                }
            });
        }
    });

    Effect::new({
        let state = state.clone();
        move |_| {
            let Some(id) = graph_id.get() else {
                return;
            };
            let Some(graph) = configured_graph(wb, state.graph.get()) else {
                return;
            };
            if graph3d_set_data(id, &graph).is_err() {
                load_failed.set(true);
            }
        }
    });

    Effect::new(move |_| {
        let Some(id) = graph_id.get() else {
            return;
        };
        let tick = reset_tick.get();
        if tick != last_reset_tick.get_untracked() {
            last_reset_tick.set(tick);
            graph3d_reset_view(id);
        }
    });

    Effect::new(move |_| {
        let Some(id) = graph_id.get() else {
            return;
        };
        let tick = zoom_tick.get();
        let prev = last_zoom_tick.get_untracked();
        if tick != prev {
            last_zoom_tick.set(tick);
            let factor = if tick > prev { 1.2 } else { 0.8 };
            graph3d_zoom(id, factor);
        }
    });

    Effect::new({
        let state = state.clone();
        move |_| {
            let _focus = state.graph_focus_generation.get();
            let node = state.graph_selected_node.get();
            let Some(id) = graph_id.get_untracked() else {
                return;
            };
            let Some(node) = node else {
                return;
            };
            let node = node.clone();
            spawn_local(async move {
                TimeoutFuture::new(60).await;
                graph3d_fly_to_node(id, &node, 800.0);
            });
        }
    });

    Effect::new(move |_| {
        let _ = preview_open.get();
        let Some(id) = graph_id.get_untracked() else {
            return;
        };
        spawn_local(async move {
            TimeoutFuture::new(0).await;
            graph3d_resize(id);
        });
    });

    let click_handle = {
        let state = state.clone();
        window_event_listener_untyped("blxcode-graph3d-node-click", move |ev| {
        let Some(custom) = ev.dyn_ref::<web_sys::CustomEvent>() else {
            return;
        };
        let detail = custom.detail();
        let event_graph_id =
            js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("graphId"))
                .ok()
                .and_then(|v| v.as_f64());
        if event_graph_id != graph_id.get_untracked() {
            return;
        }
        let Some(node_id) =
            js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("nodeId"))
                .ok()
                .and_then(|v| v.as_string())
        else {
            return;
        };
        state.graph_selected_node.set(Some(node_id.clone()));
        open_preview.run(node_id);
        })
    };

    on_cleanup(move || {
        disposed.set(true);
        drop(click_handle);
        if let Some(id) = graph_id.get_untracked() {
            graph3d_dispose(id);
        }
    });

    view! {
        <div class="workbench-memory-graph__stage">
            <div node_ref=node_ref class="workbench-memory-graph__3d" />
        </div>
    }
}

#[component]
fn Graph2dView(
    state: MemoryState,
    wb: WorkbenchService,
    reset_tick: RwSignal<u32>,
    zoom_tick: RwSignal<i32>,
    open_preview: Callback<String>,
) -> impl IntoView {
    let layout = RwSignal::new(HashMap::<String, (f32, f32)>::new());
    let viewbox = RwSignal::new((0.0_f32, 0.0_f32, 400.0_f32, 320.0_f32));
    let panning = RwSignal::new(false);
    let last_pos = RwSignal::new((0.0_f32, 0.0_f32));
    let user_interacted = RwSignal::new(false);
    let last_node_set: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let hovered: RwSignal<Option<String>> = RwSignal::new(None);
    let last_zoom_tick = RwSignal::new(zoom_tick.get_untracked());
    let last_reset_tick = RwSignal::new(reset_tick.get_untracked());

    let fit_viewbox = move |pos: &HashMap<String, (f32, f32)>| {
        if pos.is_empty() {
            viewbox.set((0.0, 0.0, 400.0, 320.0));
            return;
        }
        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pos.values() {
            minx = minx.min(x);
            maxx = maxx.max(x);
            miny = miny.min(y);
            maxy = maxy.max(y);
        }
        let pad = 60.0;
        let vw = (maxx - minx + pad * 2.0).max(120.0);
        let vh = (maxy - miny + pad * 2.0).max(120.0);
        viewbox.set((minx - pad, miny - pad, vw, vh));
    };

    Effect::new({
        let state = state.clone();
        move |_| {
            let Some(g) = state.graph.get() else { return };
            let pos = force_layout(&g, 400.0, 320.0, 180);
            let mut ids: Vec<String> = g.nodes.iter().map(|n| n.id.clone()).collect();
            ids.sort();
            let node_set_changed = last_node_set.get_untracked() != ids;
            if node_set_changed {
                last_node_set.set(ids);
            }
            if !user_interacted.get_untracked() || node_set_changed {
                fit_viewbox(&pos);
                if node_set_changed {
                    user_interacted.set(false);
                }
            }
            layout.set(pos);
        }
    });

    Effect::new(move |_| {
        let tick = reset_tick.get();
        if tick == last_reset_tick.get_untracked() {
            return;
        }
        last_reset_tick.set(tick);
        let pos = layout.get_untracked();
        fit_viewbox(&pos);
        user_interacted.set(false);
    });

    Effect::new(move |_| {
        let tick = zoom_tick.get();
        let prev = last_zoom_tick.get_untracked();
        if tick == prev {
            return;
        }
        last_zoom_tick.set(tick);
        let factor = if tick > prev { 0.87 } else { 1.15 };
        let (vx, vy, vw, vh) = viewbox.get_untracked();
        let new_w = (vw * factor).clamp(20.0, 8000.0);
        let new_h = (vh * factor).clamp(20.0, 8000.0);
        viewbox.set((
            vx + (vw - new_w) * 0.5,
            vy + (vh - new_h) * 0.5,
            new_w,
            new_h,
        ));
        user_interacted.set(true);
    });

    Effect::new({
        let state = state.clone();
        move |_| {
            let _focus = state.graph_focus_generation.get();
            let Some(node) = state.graph_selected_node.get() else {
                return;
            };
            let pos = layout.get_untracked();
            let Some((x, y)) = pos.get(&node).copied() else {
                return;
            };
            let (_, _, vw, vh) = viewbox.get_untracked();
            viewbox.set((x - vw * 0.5, y - vh * 0.5, vw, vh));
        }
    });

    let on_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let Some(t) = ev.current_target() else { return };
        let Ok(svg) = t.dyn_into::<web_sys::Element>() else {
            return;
        };
        let rect = svg.get_bounding_client_rect();
        let w = rect.width() as f32;
        let h = rect.height() as f32;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let mx = ev.client_x() as f32 - rect.left() as f32;
        let my = ev.client_y() as f32 - rect.top() as f32;
        let (vx, vy, vw, vh) = viewbox.get_untracked();
        let sx = vx + (mx / w) * vw;
        let sy = vy + (my / h) * vh;
        let factor = if ev.delta_y() > 0.0 { 1.15 } else { 1.0 / 1.15 };
        let new_vw = (vw * factor).clamp(20.0, 8000.0);
        let new_vh = (vh * factor).clamp(20.0, 8000.0);
        let new_vx = sx - (mx / w) * new_vw;
        let new_vy = sy - (my / h) * new_vh;
        viewbox.set((new_vx, new_vy, new_vw, new_vh));
        user_interacted.set(true);
    };

    let on_mousedown = move |ev: web_sys::MouseEvent| {
        panning.set(true);
        last_pos.set((ev.client_x() as f32, ev.client_y() as f32));
    };
    let on_mousemove = move |ev: web_sys::MouseEvent| {
        if !panning.get_untracked() {
            return;
        }
        let (lx, ly) = last_pos.get_untracked();
        let dx = ev.client_x() as f32 - lx;
        let dy = ev.client_y() as f32 - ly;
        last_pos.set((ev.client_x() as f32, ev.client_y() as f32));
        let Some(t) = ev.current_target() else { return };
        let Ok(svg) = t.dyn_into::<web_sys::Element>() else {
            return;
        };
        let rect = svg.get_bounding_client_rect();
        let w = rect.width() as f32;
        let h = rect.height() as f32;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (vx, vy, vw, vh) = viewbox.get_untracked();
        viewbox.set((vx - dx * (vw / w), vy - dy * (vh / h), vw, vh));
        if dx.abs() + dy.abs() > 0.0 {
            user_interacted.set(true);
        }
    };
    let on_mouseup = move |_: web_sys::MouseEvent| panning.set(false);
    let on_mouseleave = move |_: web_sys::MouseEvent| panning.set(false);

    let viewbox_str = move || {
        let (x, y, w, h) = viewbox.get();
        format!("{} {} {} {}", x, y, w, h)
    };

    view! {
        <div class="workbench-memory-graph__stage">
            <svg
                class="workbench-memory-graph__svg"
                viewBox=viewbox_str
                xmlns="http://www.w3.org/2000/svg"
                on:wheel=on_wheel
                on:mousedown=on_mousedown
                on:mousemove=on_mousemove
                on:mouseup=on_mouseup
                on:mouseleave=on_mouseleave
            >
                <g class="workbench-memory-graph__edges" fill="none">
                    {
                        let s = state.clone();
                        move || {
                            let graph = s.graph.get();
                            let graph = configured_graph(wb, graph);
                            let edges = graph.as_ref().map(|g| g.edges.clone()).unwrap_or_default();
                            let pos = layout.get();
                            let hov = hovered.get();
                            edges.into_iter().filter_map(|e| {
                                let (sx, sy) = *pos.get(&e.source)?;
                                let (tx, ty) = *pos.get(&e.target)?;
                                let incident = match hov.as_deref() {
                                    Some(h) => e.source == h || e.target == h,
                                    None => true,
                                };
                                let (stroke, width) = if hov.is_none() {
                                    ("rgba(255,255,255,0.18)", "1")
                                } else if incident {
                                    ("rgba(180,210,255,0.85)", "1.6")
                                } else {
                                    ("rgba(255,255,255,0.04)", "1")
                                };
                                Some(view! {
                                    <line
                                        x1=sx.to_string()
                                        y1=sy.to_string()
                                        x2=tx.to_string()
                                        y2=ty.to_string()
                                        stroke=stroke
                                        stroke-width=width
                                        stroke-linecap="round"
                                    />
                                })
                            }).collect::<Vec<_>>()
                        }
                    }
                </g>
                <g class="workbench-memory-graph__nodes">
                    {
                        let s = state.clone();
                        move || {
                            let pos = layout.get();
                            let graph = s.graph.get();
                            let graph = configured_graph(wb, graph);
                            let nodes = graph.as_ref().map(|g| g.nodes.clone()).unwrap_or_default();
                            let edges = graph.as_ref().map(|g| g.edges.clone()).unwrap_or_default();
                            let degrees = compute_degrees(&nodes, &edges);
                            let neighbors = compute_neighbors(&edges);
                            let hov = hovered.get();
                            let selected = state.graph_selected_node.get();
                            let (_, _, vw, vh) = viewbox.get();
                            let zoom_scale = (vw * vh).sqrt();
                            let show_labels = zoom_scale < 900.0;
                            nodes.into_iter().filter_map(|n| {
                                let (x, y) = *pos.get(&n.id)?;
                                let deg = degrees.get(&n.id).copied().unwrap_or(0);
                                let radius = 4.0_f32 + (deg as f32).sqrt() * 2.5;
                                let is_selected = selected.as_deref() == Some(n.id.as_str());
                                let (focus_state, is_hovered) = match hov.as_deref() {
                                    Some(h) if h == n.id => (NodeFocus::Hovered, true),
                                    Some(h) => {
                                        let near = neighbors.get(h).is_some_and(|set| set.contains(&n.id));
                                        (if near { NodeFocus::Neighbor } else { NodeFocus::Dim }, false)
                                    }
                                    None => (NodeFocus::Normal, false),
                                };
                                let base_fill = n.color.clone().unwrap_or_else(|| cluster_color(&n.tags, n.orphan));
                                let fill = match focus_state {
                                    NodeFocus::Dim => fade_color(&base_fill, 0.18),
                                    _ => base_fill,
                                };
                                let stroke = if is_selected {
                                    "rgba(255,255,255,1)"
                                } else {
                                    match focus_state {
                                        NodeFocus::Hovered => "rgba(255,255,255,0.95)",
                                        NodeFocus::Neighbor => "rgba(255,255,255,0.6)",
                                        NodeFocus::Dim => "rgba(255,255,255,0.08)",
                                        NodeFocus::Normal => "rgba(255,255,255,0.4)",
                                    }
                                };
                                let stroke_width = if is_selected || matches!(focus_state, NodeFocus::Hovered) { "1.6" } else { "0.5" };
                                let label_opacity = match focus_state {
                                    NodeFocus::Hovered => 1.0_f32,
                                    NodeFocus::Neighbor => 0.95,
                                    NodeFocus::Dim => 0.0,
                                    NodeFocus::Normal => if show_labels || is_selected { 0.9 } else { 0.0 },
                                };
                                let label_force_visible = is_hovered || is_selected;
                                let id_for_click = n.id.clone();
                                let id_for_enter = n.id.clone();
                                let label = clean_display_label(&n.label);
                                Some(view! {
                                    <g class="workbench-memory-graph__node"
                                        on:click=move |_| {
                                            state.graph_selected_node.set(Some(id_for_click.clone()));
                                            open_preview.run(id_for_click.clone());
                                        }
                                        on:mouseenter=move |_| hovered.set(Some(id_for_enter.clone()))
                                        on:mouseleave=move |_| hovered.set(None)
                                    >
                                        <circle
                                            cx=x.to_string()
                                            cy=y.to_string()
                                            r=radius.to_string()
                                            fill=fill
                                            stroke=stroke
                                            stroke-width=stroke_width
                                        />
                                        {(label_opacity > 0.0 || label_force_visible).then(|| view! {
                                            <text
                                                x=(x + radius + 3.0).to_string()
                                                y=(y + 3.0).to_string()
                                                font-size="9"
                                                fill="rgba(238,239,245,0.95)"
                                                opacity=label_opacity.to_string()
                                            >{label}</text>
                                        })}
                                    </g>
                                })
                            }).collect::<Vec<_>>()
                        }
                    }
                </g>
            </svg>
        </div>
    }
}

#[component]
fn GraphPreviewPopover(
    state: MemoryState,
    preview: GraphPreviewState,
    open_preview: Callback<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb_for_handoff = expect_context::<WorkbenchService>();
    let handoff_open = RwSignal::new(false);
    let on_preview_click = {
        let state = state.clone();
        move |ev: web_sys::MouseEvent| {
            if let Some(DomNavHref::Memory(path)) = dom_click_nav_href(&ev) {
                ev.prevent_default();
                ev.stop_propagation();
                state.graph_selected_node.set(Some(path.clone()));
                open_preview.run(path);
            }
        }
    };
    view! {
        <Show when=move || preview.open.get()>
            <aside class="workbench-memory-graph-preview">
                <header class="workbench-memory-graph-preview__head">
                    <span class="workbench-memory-graph-preview__title">{move || preview.label.get()}</span>
                    <div class="workbench-memory-graph-preview__actions">
                        <div class="workbench-handoff-anchor">
                            <button
                                type="button"
                                class="workbench-memory-graph-preview__btn"
                                title=move || i18n.tr(I18nKey::MemGraphSendToTerminal)()
                                aria-label=move || i18n.tr(I18nKey::MemGraphSendToTerminal)()
                                on:click=move |_| handoff_open.update(|v| *v = !*v)
                            >
                                <LxIcon icon=icondata::LuSquareTerminal width="0.82rem" height="0.82rem" />
                            </button>
                            <Show when=move || handoff_open.get()>
                                <HandoffMenu
                                    wb=wb_for_handoff
                                    label=Signal::derive(move || preview.label.get())
                                    note_path=Signal::derive(move || preview.path.get())
                                    source_slot=Signal::derive(|| None::<u64>)
                                    source_terminal_title=Signal::derive(String::new)
                                    on_close=Callback::new(move |_| handoff_open.set(false))
                                />
                            </Show>
                        </div>
                        <button
                            type="button"
                            class="workbench-memory-graph-preview__btn"
                            title=move || i18n.tr(I18nKey::MemGraphOpenInFiles)()
                            aria-label=move || i18n.tr(I18nKey::MemGraphOpenInFiles)()
                            on:click=move |_| {
                                let Some(ws) = state.workspace_cwd.get_untracked() else { return };
                                let Some(path) = preview.path.get_untracked() else { return };
                                expand_files_group_for_path(state.clone(), &path);
                                preview.open.set(false);
                                load_note(state.clone(), ws, path);
                                state.view.set(MemoryView::Files);
                            }
                        >
                            <LxIcon icon=icondata::LuFolderOpen width="0.82rem" height="0.82rem" />
                        </button>
                        <button
                            type="button"
                            class="workbench-memory-graph-preview__btn"
                            title=move || i18n.tr(I18nKey::MemGraphPreviewClose)()
                            aria-label=move || i18n.tr(I18nKey::MemGraphPreviewClose)()
                            on:click=move |_| preview.open.set(false)
                        >
                            <LxIcon icon=icondata::LuX width="0.82rem" height="0.82rem" />
                        </button>
                    </div>
                </header>
                <div
                    class="workbench-memory-graph-preview__body workbench-memory-editor__preview"
                    on:click=on_preview_click
                >
                    <Show
                        when=move || !preview.loading.get()
                        fallback=move || view! { <p class="workbench-memory-graph-preview__loading">"Loading..."</p> }
                    >
                        <div inner_html=move || render_markdown_to_html(&preview.content.get()) />
                    </Show>
                </div>
            </aside>
        </Show>
    }
}

fn open_graph_preview(state: MemoryState, preview: GraphPreviewState, path: String) {
    state.graph_selected_node.set(Some(path.clone()));
    preview.open.set(true);
    preview.path.set(Some(path.clone()));
    preview.label.set(label_for_path(&state, &path));
    preview.content.set(String::new());
    preview.loading.set(true);
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        preview.loading.set(false);
        return;
    };
    spawn_local(async move {
        TimeoutFuture::new(40).await;
        match tauri_bridge::memory_read(&ws, &path).await {
            Ok(NoteContent { content, .. }) => {
                preview.content.set(content);
                preview.label.set(label_for_path(&state, &path));
                preview.path.set(Some(path));
            }
            Err(e) => {
                preview.content.set(e);
            }
        }
        preview.loading.set(false);
    });
}

fn label_for_path(state: &MemoryState, path: &str) -> String {
    state
        .graph
        .get_untracked()
        .and_then(|g| {
            g.nodes
                .iter()
                .find(|node| node.id == path)
                .map(|node| clean_display_label(&node.label))
        })
        .unwrap_or_else(|| clean_display_label(path))
}

fn clean_display_label(raw: &str) -> String {
    let tail = raw
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .last()
        .unwrap_or(raw)
        .trim_end_matches(".md")
        .trim_end_matches(".MD")
        .to_string();
    let words: Vec<String> = tail
        .split(|ch: char| matches!(ch, '-' | '_' | '.' | ' ') || ch.is_whitespace())
        .filter(|word| !word.trim().is_empty())
        .map(format_label_word)
        .collect();
    if words.is_empty() {
        raw.to_string()
    } else {
        words.join(" ")
    }
}

fn format_label_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "api"
            | "ui"
            | "ux"
            | "url"
            | "http"
            | "https"
            | "json"
            | "css"
            | "html"
            | "js"
            | "ts"
            | "2d"
            | "3d"
    ) {
        return lower.to_ascii_uppercase();
    }
    let mut chars = lower.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
}

#[derive(Clone, Copy)]
enum NodeFocus {
    Normal,
    Hovered,
    Neighbor,
    Dim,
}

fn compute_degrees(
    nodes: &[crate::tauri_bridge::GraphNode],
    edges: &[crate::tauri_bridge::GraphEdge],
) -> HashMap<String, u32> {
    let mut d: HashMap<String, u32> = nodes.iter().map(|n| (n.id.clone(), 0)).collect();
    for e in edges {
        if let Some(v) = d.get_mut(&e.source) {
            *v += 1;
        }
        if let Some(v) = d.get_mut(&e.target) {
            *v += 1;
        }
    }
    d
}

fn compute_neighbors(edges: &[crate::tauri_bridge::GraphEdge]) -> HashMap<String, HashSet<String>> {
    let mut m: HashMap<String, HashSet<String>> = HashMap::new();
    for e in edges {
        m.entry(e.source.clone())
            .or_default()
            .insert(e.target.clone());
        m.entry(e.target.clone())
            .or_default()
            .insert(e.source.clone());
    }
    m
}

fn configured_graph(wb: WorkbenchService, graph: Option<GraphData>) -> Option<GraphData> {
    let ws_id = wb.active_id().get()?;
    let mut graph = graph?;
    graph.nodes.retain_mut(|node| {
        let category = graph_category_for_path(&node.id);
        let settings = wb.memory_category_settings_for_workspace(ws_id, &category);
        if !settings.show_in_graph {
            return false;
        }
        node.color = Some(settings.color);
        node.category = Some(category);
        true
    });
    let visible: HashSet<String> = graph.nodes.iter().map(|node| node.id.clone()).collect();
    graph
        .edges
        .retain(|edge| visible.contains(&edge.source) && visible.contains(&edge.target));
    Some(graph)
}

pub(crate) fn graph_category_for_path(path: &str) -> String {
    if path.starts_with("learnings/") {
        return "learnings".to_string();
    }
    if let Some((head, _)) = path.split_once('/') {
        if !head.is_empty() {
            return head.to_string();
        }
    }
    "memory".to_string()
}

fn cluster_color(tags: &[String], orphan: bool) -> String {
    if orphan {
        return "rgba(170,170,185,0.55)".to_string();
    }
    let hue = tags.first().map_or(215.0, |tag| stable_hue(tag));
    format!("hsla({hue:.0}, 70%, 64%, 0.9)")
}

fn stable_hue(s: &str) -> f32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    (h % 360) as f32
}

fn fade_color(css: &str, alpha: f32) -> String {
    if let Some(open) = css.find('(') {
        if let Some(close) = css.rfind(')') {
            let inner = &css[open + 1..close];
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 4 {
                let prefix = &css[..open + 1];
                return format!(
                    "{}{}, {}, {}, {:.3})",
                    prefix, parts[0], parts[1], parts[2], alpha
                );
            }
        }
    }
    css.to_string()
}

fn force_layout(g: &GraphData, w: f32, h: f32, iters: u32) -> HashMap<String, (f32, f32)> {
    let n = g.nodes.len();
    if n == 0 {
        return HashMap::new();
    }
    let mut pos: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = (i as f32) / (n.max(1) as f32) * std::f32::consts::TAU;
            let r = (w.min(h) * 0.35).max(20.0);
            (w / 2.0 + r * a.cos(), h / 2.0 + r * a.sin())
        })
        .collect();
    let idx: HashMap<&str, usize> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let edges: Vec<(usize, usize)> = g
        .edges
        .iter()
        .filter_map(|e| Some((*idx.get(e.source.as_str())?, *idx.get(e.target.as_str())?)))
        .collect();
    let k = (w * h / (n as f32 + 1.0)).sqrt().max(20.0);
    let mut t = w.min(h) * 0.1;
    let categories: Vec<String> = g
        .nodes
        .iter()
        .map(|node| {
            node.category
                .clone()
                .unwrap_or_else(|| graph_category_for_path(&node.id))
        })
        .collect();
    let cluster_strength = 0.04_f32;
    for _ in 0..iters {
        // Per-category centroid (attracts same-category nodes toward each other).
        let mut centroids: HashMap<&str, (f32, f32, u32)> = HashMap::new();
        for i in 0..n {
            let entry = centroids.entry(categories[i].as_str()).or_insert((0.0, 0.0, 0));
            entry.0 += pos[i].0;
            entry.1 += pos[i].1;
            entry.2 += 1;
        }
        let mut disp = vec![(0.0_f32, 0.0_f32); n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(0.5);
                let force = k * k / dist;
                disp[i].0 += dx / dist * force;
                disp[i].1 += dy / dist * force;
            }
        }
        for &(a, b) in &edges {
            let dx = pos[a].0 - pos[b].0;
            let dy = pos[a].1 - pos[b].1;
            let dist = (dx * dx + dy * dy).sqrt().max(0.5);
            let force = dist * dist / k;
            let ux = dx / dist * force;
            let uy = dy / dist * force;
            disp[a].0 -= ux;
            disp[a].1 -= uy;
            disp[b].0 += ux;
            disp[b].1 += uy;
        }
        for i in 0..n {
            if let Some(&(cx_sum, cy_sum, count)) = centroids.get(categories[i].as_str()) {
                if count > 1 {
                    let cx = cx_sum / count as f32;
                    let cy = cy_sum / count as f32;
                    disp[i].0 += (cx - pos[i].0) * cluster_strength * k;
                    disp[i].1 += (cy - pos[i].1) * cluster_strength * k;
                }
            }
        }
        for i in 0..n {
            let d = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1)
                .sqrt()
                .max(0.001);
            let limit = t.min(d);
            pos[i].0 += disp[i].0 / d * limit;
            pos[i].1 += disp[i].1 / d * limit;
            pos[i].0 = pos[i].0.clamp(10.0, w - 10.0);
            pos[i].1 = pos[i].1.clamp(10.0, h - 10.0);
        }
        t *= 0.96;
    }
    g.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), pos[i]))
        .collect()
}

/// Switch to Graph (3D), select `path`, and fly to that node when the graph is ready.
pub fn navigate_to_graph_node(state: MemoryState, path: String) {
    state.graph_selected_node.set(Some(path));
    state.graph_focus_generation.update(|n| *n += 1);
    state.graph_prefer_3d.set(true);
    state.view.set(MemoryView::Graph);
    if let Some(ws) = state.workspace_cwd.get_untracked() {
        refresh_graph(state, ws);
    }
}
