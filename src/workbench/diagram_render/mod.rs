//! Reusable inline Mermaid renderer.
//!
//! Unlike [`crate::workbench::file_preview::mermaid_view::MermaidView`], which
//! reads a workspace file, this component renders Mermaid source passed
//! directly as a prop. It is shared by the diagram cards in the agent timeline
//! and the centered diagram gallery.
//!
//! The rendered `<svg>` is written into a node carrying the supplied `dom_id`,
//! so callers can read it back out (e.g. for PDF export) via
//! [`rendered_svg_outer_html`].

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::file_preview::codemirror_glue as cm;
use crate::workbench::file_preview::codemirror_glue::EditorKeyBinding;
use crate::workbench::file_preview::mermaid_glue::render_mermaid_to_svg;
use crate::workbench::theme_service::ThemeService;
use gloo_timers::callback::Timeout;
use leptos::html;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, PointerEvent, WheelEvent};

/// Zoom step / bounds for interactive diagram viewports.
const ZOOM_STEP: f64 = 1.2;
const ZOOM_MIN: f64 = 0.25;
const ZOOM_MAX: f64 = 4.0;
const VIEWPORT_ANCHOR_X: f64 = 0.25;
const VIEWPORT_ANCHOR_Y: f64 = 0.25;
const SOURCE_RENDER_DEBOUNCE_MS: u32 = 180;
const INSPECTOR_DEFAULT_WIDTH_PX: f64 = 420.0;
const INSPECTOR_MIN_WIDTH_PX: f64 = 280.0;
const INSPECTOR_MAX_WIDTH_PX: f64 = 760.0;
const VIEWPORT_MIN_WIDTH_PX: f64 = 360.0;
const INSPECTOR_BOTTOM_BREAKPOINT_PX: f64 = 900.0;

type SourceEditorClosures = (
    Closure<dyn Fn(String)>,
    Closure<dyn Fn()>,
    Closure<dyn Fn(f64, f64)>,
);

// Process-wide counter for unique Mermaid render ids (the library requires a
// unique element id per `render` call).
thread_local! {
    static RENDER_SEQ: Cell<u64> = const { Cell::new(0) };
}

fn next_render_id() -> String {
    RENDER_SEQ.with(|c| {
        let n = c.get().wrapping_add(1);
        c.set(n);
        format!("mmd-render-{n}")
    })
}

fn update_inspector_narrow_layout(
    workspace_ref: NodeRef<html::Div>,
    inspector_narrow: RwSignal<bool>,
    resizing_inspector: RwSignal<bool>,
) {
    let Some(el) = workspace_ref.get_untracked() else {
        return;
    };
    let narrow = el.get_bounding_client_rect().width() < INSPECTOR_BOTTOM_BREAKPOINT_PX;
    inspector_narrow.set(narrow);
    if narrow && resizing_inspector.get_untracked() {
        resizing_inspector.set(false);
    }
}

thread_local! {
    /// Caches successfully rendered diagram markup keyed by `(theme, code)`.
    ///
    /// The timeline tree is rebuilt on every agent update (no keyed `<For>`),
    /// so an inline `DiagramRender` re-mounts repeatedly. Mermaid renders
    /// asynchronously, so without a cache each re-mount restarts the render and
    /// the just-produced SVG is discarded before it is visible. Re-injecting the
    /// cached SVG synchronously on mount makes inline diagrams appear instantly
    /// and stay put. Keyed by theme so a theme switch forces a re-render.
    static SVG_CACHE: RefCell<HashMap<(String, String), String>> = RefCell::new(HashMap::new());
}

thread_local! {
    /// Records the wall-clock time (epoch ms) a diagram id was first rendered.
    /// Inline timeline cards render as soon as the `mermaid_create` tool result
    /// arrives, so this is a good proxy for the diagram's generation time and is
    /// shown in the centered gallery's stats. Session-scoped (not persisted).
    static FIRST_SEEN: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
}

/// Stamp `id` as seen now if not already recorded; returns the recorded time.
pub fn mark_diagram_seen(id: &str) -> f64 {
    FIRST_SEEN.with(|c| {
        *c.borrow_mut()
            .entry(id.to_owned())
            .or_insert_with(js_sys::Date::now)
    })
}

/// The first-seen time (epoch ms) for `id`, if it has been rendered this session.
pub fn diagram_first_seen(id: &str) -> Option<f64> {
    FIRST_SEEN.with(|c| c.borrow().get(id).copied())
}

fn cache_get(theme: &str, code: &str) -> Option<String> {
    SVG_CACHE.with(|c| {
        c.borrow()
            .get(&(theme.to_owned(), code.to_owned()))
            .cloned()
    })
}

fn cache_put(theme: &str, code: &str, html: String) {
    SVG_CACHE.with(|c| {
        let mut map = c.borrow_mut();
        // Bound the cache so long sessions don't grow it without limit.
        if map.len() > 256 {
            map.clear();
        }
        map.insert((theme.to_owned(), code.to_owned()), html);
    });
}

/// Read back the rendered SVG markup from a diagram node by its `dom_id`.
/// Returns `None` until Mermaid has finished rendering.
pub fn rendered_svg_outer_html(dom_id: &str) -> Option<String> {
    let svg = web_sys::window()?
        .document()?
        .get_element_by_id(dom_id)?
        .query_selector("svg")
        .ok()
        .flatten()?;
    svg.dyn_ref::<web_sys::Element>().map(|e| e.outer_html())
}

#[component]
pub fn MermaidPreviewWithInspector(
    #[prop(into)] source: RwSignal<String>,
    dom_id: String,
    #[prop(into)] inspector_open: Signal<bool>,
    #[prop(into)] can_save: Signal<bool>,
    #[prop(into)] can_revert: Signal<bool>,
    on_save: Callback<()>,
    on_revert: Callback<()>,
    #[prop(default = true)] allow_save: bool,
    #[prop(default = false)] compact: bool,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let preview_code = RwSignal::new(source.get_untracked());
    let pending_debounce = StoredValue::new_local(None::<Timeout>);
    let workspace_ref = NodeRef::<html::Div>::new();
    let inspector_width = RwSignal::new(INSPECTOR_DEFAULT_WIDTH_PX);
    let inspector_narrow = RwSignal::new(false);
    let resizing_inspector = RwSignal::new(false);
    let resize_start = RwSignal::new((0.0_f64, INSPECTOR_DEFAULT_WIDTH_PX));
    let pending_layout_measure = StoredValue::new_local(None::<Timeout>);

    Effect::new(move |_| {
        let next = source.get();
        pending_debounce.update_value(|slot| {
            *slot = Some(Timeout::new(SOURCE_RENDER_DEBOUNCE_MS, move || {
                preview_code.set(next);
            }));
        });
    });

    let on_resize_down = move |ev: PointerEvent| {
        if inspector_narrow.get_untracked() {
            return;
        }
        ev.prevent_default();
        ev.stop_propagation();
        resizing_inspector.set(true);
        resize_start.set((ev.client_x() as f64, inspector_width.get_untracked()));
        if let Some(target) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = target.set_pointer_capture(ev.pointer_id());
        }
    };

    let move_listener = window_event_listener_untyped("pointermove", move |ev| {
        if !resizing_inspector.get_untracked() {
            return;
        }
        let Some(pe) = ev.dyn_ref::<PointerEvent>() else {
            return;
        };
        let (start_x, start_width) = resize_start.get_untracked();
        let raw = start_width - (pe.client_x() as f64 - start_x);
        let max_width = workspace_ref
            .get_untracked()
            .map(|el| {
                let rect = el.get_bounding_client_rect();
                (rect.width() - VIEWPORT_MIN_WIDTH_PX)
                    .max(INSPECTOR_MIN_WIDTH_PX)
                    .min(INSPECTOR_MAX_WIDTH_PX)
            })
            .unwrap_or(INSPECTOR_MAX_WIDTH_PX);
        inspector_width.set(raw.max(INSPECTOR_MIN_WIDTH_PX).min(max_width));
    });

    let up_listener = window_event_listener_untyped("pointerup", move |_| {
        if resizing_inspector.get_untracked() {
            resizing_inspector.set(false);
        }
    });

    Effect::new(move |_| {
        let _ = inspector_open.get();
        pending_layout_measure.update_value(|slot| {
            *slot = Some(Timeout::new(0, move || {
                update_inspector_narrow_layout(workspace_ref, inspector_narrow, resizing_inspector);
            }));
        });
    });

    let window_resize_listener = window_event_listener_untyped("resize", move |_| {
        update_inspector_narrow_layout(workspace_ref, inspector_narrow, resizing_inspector);
    });

    on_cleanup(move || {
        pending_debounce.update_value(|slot| {
            *slot = None;
        });
        pending_layout_measure.update_value(|slot| {
            *slot = None;
        });
        move_listener.remove();
        up_listener.remove();
        window_resize_listener.remove();
    });

    view! {
        <div
            node_ref=workspace_ref
            class="mermaid-workspace"
            class:mermaid-workspace--inspector=move || inspector_open.get()
            class:mermaid-workspace--narrow=move || inspector_narrow.get()
            class:mermaid-workspace--resizing=move || resizing_inspector.get()
            style=move || format!("--mermaid-inspector-width: {:.0}px;", inspector_width.get())
        >
            <InteractiveDiagramViewport code=preview_code dom_id=dom_id compact=compact>
                {children.map(|children| children())}
            </InteractiveDiagramViewport>
            <Show when=move || inspector_open.get()>
                <button
                    type="button"
                    class="mermaid-inspector__resizer"
                    class:mermaid-inspector__resizer--active=move || resizing_inspector.get()
                    aria-label="Resize Mermaid source inspector"
                    on:pointerdown=on_resize_down
                >
                    <span aria-hidden="true"></span>
                </button>
                <MermaidSourceInspector
                    source=source
                    can_save=can_save
                    can_revert=can_revert
                    on_save=on_save
                    on_revert=on_revert
                    allow_save=allow_save
                />
            </Show>
            <Show when=move || resizing_inspector.get()>
                <div class="mermaid-inspector__resize-shield" aria-hidden="true"></div>
            </Show>
        </div>
    }
}

#[component]
pub fn MermaidSourceInspector(
    #[prop(into)] source: RwSignal<String>,
    #[prop(into)] can_save: Signal<bool>,
    #[prop(into)] can_revert: Signal<bool>,
    on_save: Callback<()>,
    on_revert: Callback<()>,
    #[prop(default = true)] allow_save: bool,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <aside class="mermaid-inspector">
            <div class="mermaid-inspector__head">
                <span class="mermaid-inspector__title">"Mermaid"</span>
                <Show when=move || can_revert.get()>
                    <span class="mermaid-inspector__dirty">
                        {move || i18n.tr(I18nKey::FilePreviewEditorModified)}
                    </span>
                </Show>
            </div>
            <MermaidSourceEditor source=source on_save=on_save read_only=false />
            <div class="mermaid-inspector__actions">
                <button
                    class="mermaid-inspector__btn"
                    disabled=move || !can_revert.get()
                    on:click=move |_| on_revert.run(())
                >
                    {move || i18n.tr(I18nKey::FilePreviewEditorRevert)}
                </button>
                <Show when=move || allow_save>
                    <button
                        class="mermaid-inspector__btn mermaid-inspector__btn--primary"
                        disabled=move || !can_save.get()
                        on:click=move |_| on_save.run(())
                    >
                        {move || i18n.tr(I18nKey::FilePreviewEditorSave)}
                    </button>
                </Show>
            </div>
        </aside>
    }
}

#[component]
fn MermaidSourceEditor(
    #[prop(into)] source: RwSignal<String>,
    on_save: Callback<()>,
    #[prop(default = false)] read_only: bool,
) -> impl IntoView {
    let host_ref = NodeRef::<html::Div>::new();
    let view_handle = StoredValue::new_local(None::<JsValue>);
    let closures = StoredValue::new_local(None::<SourceEditorClosures>);

    Effect::new(move |_| {
        let Some(host) = host_ref.get() else {
            return;
        };
        if view_handle.with_value(|v| v.is_some()) {
            return;
        }
        let on_change = Closure::<dyn Fn(String)>::new(move |s: String| {
            source.set(s);
        });
        let on_save_closure = Closure::<dyn Fn()>::new(move || {
            if !read_only {
                on_save.run(());
            }
        });
        let on_cursor = Closure::<dyn Fn(f64, f64)>::new(move |_, _| {});
        let on_change_fn: js_sys::Function = on_change
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        let on_save_fn: js_sys::Function = on_save_closure
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        let on_cursor_fn: js_sys::Function = on_cursor
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        closures.set_value(Some((on_change, on_save_closure, on_cursor)));

        let host_el: web_sys::Element = host.unchecked_into();
        let doc = source.get_untracked();
        spawn_local(async move {
            let keymap: Vec<EditorKeyBinding> = Vec::new();
            match cm::create_editor(
                &host_el,
                &doc,
                None,
                read_only,
                false,
                &keymap,
                &on_change_fn,
                &on_save_fn,
                &on_cursor_fn,
            )
            .await
            {
                Ok(view) => view_handle.set_value(Some(view)),
                Err(e) => web_sys::console::error_1(&format!("codemirror init: {e}").into()),
            }
        });
    });

    Effect::new(move |_| {
        let text = source.get();
        view_handle.with_value(|v| {
            if let Some(view) = v {
                cm::set_doc(view, &text);
            }
        });
    });

    on_cleanup(move || {
        view_handle.update_value(|v| {
            if let Some(view) = v.take() {
                cm::destroy(&view);
            }
        });
        closures.update_value(|c| *c = None);
    });

    view! {
        <div class="mermaid-inspector__editor" node_ref=host_ref />
    }
}

#[component]
pub fn InteractiveDiagramViewport(
    /// Raw Mermaid source.
    #[prop(into)]
    code: Signal<String>,
    /// Stable DOM id of the render container (used to read back the SVG).
    dom_id: String,
    #[prop(default = false)] compact: bool,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let zoom = RwSignal::new(1.0_f64);
    let pan_x = RwSignal::new(0.0_f64);
    let pan_y = RwSignal::new(0.0_f64);
    let dragging = RwSignal::new(false);
    let drag_start = RwSignal::new((0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64));
    let viewport_ref: NodeRef<html::Div> = NodeRef::new();

    let zoom_at = move |client_x: f64, client_y: f64, direction: f64| {
        let old_zoom = zoom.get_untracked();
        let new_zoom = if direction < 0.0 {
            (old_zoom * ZOOM_STEP).min(ZOOM_MAX)
        } else {
            (old_zoom / ZOOM_STEP).max(ZOOM_MIN)
        };
        if (new_zoom - old_zoom).abs() < f64::EPSILON {
            return;
        }
        let Some(el) = viewport_ref.get_untracked() else {
            zoom.set(new_zoom);
            return;
        };
        let rect = el.get_bounding_client_rect();
        let local_x = client_x - rect.left();
        let local_y = client_y - rect.top();
        let anchor_x = rect.width() * VIEWPORT_ANCHOR_X;
        let anchor_y = rect.height() * VIEWPORT_ANCHOR_Y;
        let content_x = (local_x - anchor_x - pan_x.get_untracked()) / old_zoom;
        let content_y = (local_y - anchor_y - pan_y.get_untracked()) / old_zoom;
        pan_x.set(local_x - anchor_x - (content_x * new_zoom));
        pan_y.set(local_y - anchor_y - (content_y * new_zoom));
        zoom.set(new_zoom);
    };

    let zoom_in = move |_| {
        let Some(el) = viewport_ref.get_untracked() else {
            zoom.update(|z| *z = (*z * ZOOM_STEP).min(ZOOM_MAX));
            return;
        };
        let rect = el.get_bounding_client_rect();
        zoom_at(
            rect.left() + rect.width() / 2.0,
            rect.top() + rect.height() / 2.0,
            -1.0,
        );
    };
    let zoom_out = move |_| {
        let Some(el) = viewport_ref.get_untracked() else {
            zoom.update(|z| *z = (*z / ZOOM_STEP).max(ZOOM_MIN));
            return;
        };
        let rect = el.get_bounding_client_rect();
        zoom_at(
            rect.left() + rect.width() / 2.0,
            rect.top() + rect.height() / 2.0,
            1.0,
        );
    };
    let zoom_reset = move |_| {
        zoom.set(1.0);
        pan_x.set(0.0);
        pan_y.set(0.0);
    };
    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        zoom_at(ev.client_x() as f64, ev.client_y() as f64, ev.delta_y());
    };
    let on_pointer_down = move |ev: PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        ev.prevent_default();
        dragging.set(true);
        drag_start.set((
            ev.client_x() as f64,
            ev.client_y() as f64,
            pan_x.get_untracked(),
            pan_y.get_untracked(),
        ));
        if let Some(el) = viewport_ref.get_untracked() {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
    };
    let on_pointer_move = move |ev: PointerEvent| {
        if !dragging.get_untracked() {
            return;
        }
        ev.prevent_default();
        let (start_x, start_y, origin_x, origin_y) = drag_start.get_untracked();
        pan_x.set(origin_x + (ev.client_x() as f64 - start_x));
        pan_y.set(origin_y + (ev.client_y() as f64 - start_y));
    };
    let on_pointer_up = move |ev: PointerEvent| {
        dragging.set(false);
        if let Some(el) = viewport_ref.get_untracked() {
            let _ = el.release_pointer_capture(ev.pointer_id());
        }
    };

    view! {
        <div
            node_ref=viewport_ref
            class="diagram-viewport"
            class:diagram-viewport--compact=move || compact
            class:diagram-viewport--dragging=move || dragging.get()
            on:wheel=on_wheel
            on:pointerdown=on_pointer_down
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up
            on:pointercancel=on_pointer_up
        >
            <div
                class="diagram-viewport__surface"
                style=move || format!(
                    "transform: translate({:.2}px, {:.2}px) scale({:.3});",
                    pan_x.get(),
                    pan_y.get(),
                    zoom.get(),
                )
            >
                <DiagramRender code=code dom_id=dom_id />
            </div>
            {children.map(|children| children())}
            <div
                class="diagram-viewport__zoom"
                role="group"
                aria-label=move || i18n.tr(I18nKey::DiagramZoomGroupAria)()
            >
                <button
                    class="diagram-viewport__zoom-btn"
                    on:click=zoom_out
                    title=move || i18n.tr(I18nKey::DiagramZoomOut)()
                >
                    "−"
                </button>
                <span class="diagram-viewport__zoom-level">
                    {move || format!("{:.0}%", zoom.get() * 100.0)}
                </span>
                <button
                    class="diagram-viewport__zoom-btn"
                    on:click=zoom_in
                    title=move || i18n.tr(I18nKey::DiagramZoomIn)()
                >
                    "+"
                </button>
                <button
                    class="diagram-viewport__zoom-btn"
                    on:click=zoom_reset
                    title=move || i18n.tr(I18nKey::DiagramZoomReset)()
                >
                    "⟳"
                </button>
            </div>
        </div>
    }
}

#[component]
pub fn DiagramRender(
    /// Raw Mermaid source.
    #[prop(into)]
    code: Signal<String>,
    /// Stable DOM id of the render container (used to read back the SVG).
    dom_id: String,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let theme = expect_context::<ThemeService>();
    let node_ref: NodeRef<html::Div> = NodeRef::new();
    let render_err = RwSignal::new(false);

    Effect::new(move |_| {
        let text = code.get();
        // Subscribe to theme changes so the diagram re-renders with the active
        // theme's tokens (multi-theme support, see rule-theme-tokens.md).
        let theme_id = theme.active_theme_id().get();
        let Some(el) = node_ref.get() else {
            return;
        };
        let element: HtmlElement = el.unchecked_into();
        render_err.set(false);
        if text.trim().is_empty() {
            element.set_inner_html("");
            return;
        }
        // Fast path: a previously rendered SVG for this theme+code is injected
        // synchronously, so a re-mount shows the diagram immediately instead of
        // racing an async render whose result could be discarded.
        if let Some(svg) = cache_get(&theme_id, &text) {
            element.set_inner_html(&svg);
            return;
        }
        element.set_inner_html("");
        let id = next_render_id();
        spawn_local(async move {
            match render_mermaid_to_svg(&id, &text).await {
                Ok(svg) => {
                    // The SVG is fully self-contained (Mermaid measured it in its
                    // own offscreen container), so caching it is always valid —
                    // even if this node was discarded by a timeline rebuild.
                    cache_put(&theme_id, &text, svg.clone());
                    if let Some(el) = node_ref.get_untracked() {
                        let el: HtmlElement = el.unchecked_into();
                        el.set_inner_html(&svg);
                    }
                }
                Err(e) => {
                    web_sys::console::warn_1(&format!("mermaid render: {e}").into());
                    render_err.set(true);
                }
            }
        });
    });

    view! {
        <div class="diagram-render">
            <div node_ref=node_ref id=dom_id class="diagram-render__stage" />
            <Show when=move || render_err.get()>
                <div class="diagram-render__error">
                    {move || i18n.tr(I18nKey::FilePreviewMermaidError)}
                </div>
            </Show>
        </div>
    }
}
