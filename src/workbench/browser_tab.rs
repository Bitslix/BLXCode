//! Eingebetteter Browser-Reiter: Toolbar + Host für native Child-Webview oder `<iframe>`-Fallback.
use crate::agent_wire::BrowserBoundsPayload;
use crate::config::NEW_TAB_BROWSER_SHORTLINKS;
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::browser_navigate;
use gloo_timers::future::TimeoutFuture;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::workbench::{BrowserEmbedSurface, RightPanelTab, WorkbenchService};

pub const EMBEDDED_BROWSER_HOST_ID: &str = "blx-embedded-browser-host";
const IFRAME_EMBED_ID: &str = "workbench-browser-iframe";

fn tab_strip_label(url: &str, loc: Locale) -> String {
    let u = url.trim();
    if u.is_empty() {
        return lookup(loc, I18nKey::BrNewTab).to_owned();
    }
    let rest = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .unwrap_or(u);
    let host = rest.split('/').next().unwrap_or(rest);
    host.chars().take(30).collect()
}

async fn navigate_current_url(wb: WorkbenchService) {
    let u = wb.browser_url().get_untracked();
    if u.trim().is_empty() {
        return;
    }
    let _ = browser_navigate(u.trim()).await;
}

fn resolve_host_bounds() -> Option<BrowserBoundsPayload> {
    let win = web_sys::window()?;
    let doc = win.document()?;
    let el = doc.get_element_by_id(EMBEDDED_BROWSER_HOST_ID)?;
    let rect = el.get_bounding_client_rect();
    let w = rect.width();
    let h = rect.height();
    let visible = w >= 8.0 && h >= 8.0;
    Some(BrowserBoundsPayload {
        x: rect.left(),
        y: rect.top(),
        w,
        h,
        visible,
    })
}

fn embed_is_native(surface: BrowserEmbedSurface) -> bool {
    surface.0.get_untracked().as_deref() == Some("native_child")
}

/// Bounds-Sync zwischen Layout und nativer Child-Webview.
pub async fn sync_embedded_browser_layer(wb: WorkbenchService, surface: BrowserEmbedSurface) {
    if !embed_is_native(surface) {
        return;
    }

    let tab = wb.right_active_tab().get_untracked();
    let collapsed = wb.right_collapsed().get_untracked();

    if collapsed || tab != RightPanelTab::Browser {
        let _ = crate::tauri_bridge::browser_sync_bounds(
            BrowserBoundsPayload {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
                visible: false,
            },
            None,
        )
        .await;
        return;
    }

    let url_empty = wb.browser_url().get_untracked().trim().is_empty();

    let mut payload = resolve_host_bounds().unwrap_or(BrowserBoundsPayload {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        visible: false,
    });
    if url_empty {
        payload.visible = false;
    }

    let _ = crate::tauri_bridge::browser_sync_bounds(payload, None).await;
}

fn try_reload_embedded_iframe() {
    let Some(w) = web_sys::window() else {
        return;
    };
    let Some(doc) = w.document() else {
        return;
    };
    let Some(el) = doc.get_element_by_id(IFRAME_EMBED_ID) else {
        return;
    };
    let Ok(frame) = el.dyn_into::<web_sys::HtmlIFrameElement>() else {
        return;
    };
    let Some(sub) = frame.content_window() else {
        return;
    };
    let _ = sub.location().reload();
}

#[component]
fn BrowserNewTabPane(wb: WorkbenchService, surface: BrowserEmbedSurface) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="workbench-browser-new-tab" aria-label=move || i18n.tr(I18nKey::BrNewTab)()>
            <p class="workbench-browser-new-tab-hint">
                {move || i18n.tr(I18nKey::BrNewHint)()}
            </p>
            <p class="workbench-browser-new-tab-shortcuts-title">
                {move || i18n.tr(I18nKey::BrShortcutsHeading)()}
            </p>
            <ul class="workbench-browser-shortlinks" role="list">
                {NEW_TAB_BROWSER_SHORTLINKS.iter().copied().map(|(label, href)| {
                    let dest = href.to_string();
                    view! {
                        <li role="none">
                            <button
                                type="button"
                                class="workbench-mini-btn workbench-browser-shortlink"
                                on:click=move |_| {
                                    wb.persist_browser_url_from_input(dest.clone());
                                    let w = wb;
                                    let embed = surface;
                                    spawn_local(async move {
                                        let url = w.browser_url().get_untracked();
                                        if !url.trim().is_empty() {
                                            let _ = browser_navigate(url.trim()).await;
                                        }
                                        sync_embedded_browser_layer(w, embed).await;
                                    });
                                }
                            >
                                {label}
                            </button>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
pub fn EmbeddedBrowserGlue() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let surface = expect_context::<BrowserEmbedSurface>();

    Effect::new(move |_| {
        let tab = wb.right_active_tab().get();
        let collapsed = wb.right_collapsed().get();
        let _w = wb.right_width_px().get();
        let _sb = wb.sidebar_collapsed().get();
        let _sk = surface.0.get();
        let _url_tick = wb.browser_url().get();
        let wc = wb;
        let sc = surface;
        spawn_local(async move {
            TimeoutFuture::new(16).await;
            sync_embedded_browser_layer(wc, sc).await;
        });

        if !embed_is_native(surface) || collapsed || tab != RightPanelTab::Browser {
            return;
        }

        let wp = wb;
        let sp = surface;
        let h_resize = window_event_listener_untyped("resize", move |_| {
            spawn_local(async move {
                TimeoutFuture::new(16).await;
                sync_embedded_browser_layer(wp, sp).await;
            })
        });

        let wu = wb;
        let su = surface;
        let h_up = window_event_listener_untyped("mouseup", move |_| {
            spawn_local(async move {
                sync_embedded_browser_layer(wu, su).await;
            })
        });

        on_cleanup(move || {
            h_resize.remove();
            h_up.remove();
        });
    });

    view! {
        <span class="workbench-visually-hidden" aria-hidden="true"></span>
    }
}

#[component]
pub fn BrowserTabDock() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let surface = expect_context::<BrowserEmbedSurface>();
    let i18n = expect_context::<I18nService>();
    let draft_url = RwSignal::new(wb.browser_url().get_untracked());

    Effect::new(move |_| {
        let u = wb.browser_url().get();
        draft_url.set(u);
    });

    view! {
        <div class="workbench-browser-dock">
            <div class="workbench-browser-tabstrip" role="tablist" aria-label=move || i18n.tr(I18nKey::BrTabsAria)()>
                {move || {
                    let tabs_vec = wb.embedded_browser_tabs().get();

                    tabs_vec
                        .iter()
                        .map(|t| {
                            let tid = t.id;
                            let url_owned = t.url.clone();
                            view! {
                                <div
                                    class="workbench-browser-page-tab"
                                    class:workbench-browser-page-tab--active=move || {
                                        wb.embedded_browser_active_id().get() == tid
                                    }
                                    role="presentation"
                                >
                                    <button
                                        type="button"
                                        class="workbench-browser-page-tab__main"
                                        role="tab"
                                        aria-selected=move || wb.embedded_browser_active_id().get() == tid
                                        on:click=move |_| {
                                            wb.select_embedded_browser_tab(tid);
                                            let w = wb;
                                            let e = surface;
                                            spawn_local(async move {
                                                sync_embedded_browser_layer(w, e).await;
                                            });
                                        }
                                    >
                                        {move || tab_strip_label(&url_owned, i18n.locale().get())}
                                    </button>
                                    <button
                                        type="button"
                                        class="workbench-browser-page-tab__close workbench-mini-btn"
                                        aria-label=move || i18n.tr(I18nKey::BrCloseTab)()
                                        prop:disabled=move || wb.embedded_browser_tabs().with(|v| v.len() <= 1)
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            wb.close_embedded_browser_tab(tid);
                                            let w = wb;
                                            let e = surface;
                                            spawn_local(async move {
                                                sync_embedded_browser_layer(w, e).await;
                                            });
                                        }
                                    >
                                        "×"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
                <button
                    type="button"
                    class="workbench-mini-btn workbench-browser-new-tab-btn"
                    aria-label=move || i18n.tr(I18nKey::BrNewTabBtnAria)()
                    on:click=move |_| {
                        let _id = wb.add_embedded_browser_tab();
                        let w = wb;
                        let e = surface;
                        spawn_local(async move {
                            sync_embedded_browser_layer(w, e).await;
                        });
                    }
                >
                    "+"
                </button>
            </div>
            <div class="workbench-browser-toolbar" role="toolbar" aria-label=move || i18n.tr(I18nKey::BrToolbarAria)()>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    aria-label=move || i18n.tr(I18nKey::BrBack)()
                    on:click=move |_| {
                        spawn_local(async move {
                            let _ =
                                crate::tauri_bridge::browser_run_js("window.history.back()").await;
                        });
                    }
                >
                    "←"
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    aria-label=move || i18n.tr(I18nKey::BrFwd)()
                    on:click=move |_| {
                        spawn_local(async move {
                            let _ = crate::tauri_bridge::browser_run_js("window.history.forward()")
                                .await;
                        });
                    }
                >
                    "→"
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    aria-label=move || i18n.tr(I18nKey::BrReload)()
                    on:click=move |_| {
                        let embed = surface;
                        let w = wb;
                        spawn_local(async move {
                            if embed_is_native(embed) {
                                navigate_current_url(w).await;
                            } else {
                                try_reload_embedded_iframe();
                            }
                            sync_embedded_browser_layer(w, embed).await;
                        });
                    }
                >
                    "↻"
                </button>

                <input
                    type="url"
                    class="workbench-browser-url"
                    prop:value=move || draft_url.get()
                    on:input=move |ev| {
                        if let Some(el) = ev_target_input(&ev) {
                            draft_url.set(el.value());
                        }
                    }
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key().as_str() == "Enter" {
                            ev.prevent_default();
                            let url = draft_url.get_untracked();
                            wb.persist_browser_url_from_input(url.clone());
                            let w = wb;
                            let embed = surface;
                            spawn_local(async move {
                                let u = w.browser_url().get_untracked();
                                if !u.trim().is_empty() {
                                    let _ =
                                        crate::tauri_bridge::browser_navigate(u.trim()).await;
                                }
                                sync_embedded_browser_layer(w, embed).await;
                            });
                        }
                    }
                />

                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    on:click=move |_| {
                        let url = draft_url.get_untracked();
                        wb.persist_browser_url_from_input(url.clone());
                        let w = wb;
                        let embed = surface;
                        spawn_local(async move {
                            let u = w.browser_url().get_untracked();
                            if !u.trim().is_empty() {
                                let _ = crate::tauri_bridge::browser_navigate(u.trim()).await;
                            }
                            sync_embedded_browser_layer(w, embed).await;
                        })
                    }
                >
                    {move || i18n.tr(I18nKey::BrGo)()}
                </button>
            </div>

            {move || match surface.0.get().as_deref() {
                None => view! {
                    <div
                        class="workbench-browser-host workbench-browser-embed-pending"
                        aria-busy="true"
                    >
                        {move || i18n.tr(I18nKey::BrPreparing)()}
                    </div>
                }
                .into_any(),
                Some("native_child") => view! {
                    <div
                        id=EMBEDDED_BROWSER_HOST_ID
                        class="workbench-browser-host"
                        aria-label=move || i18n.tr(I18nKey::BrNativeAria)()
                    >
                        <Show
                            when=move || wb.browser_url().get().trim().is_empty()
                            fallback=move || ().into_any()
                        >
                            <BrowserNewTabPane wb surface />
                        </Show>
                    </div>
                }
                .into_any(),
                _ => view! {
                    <div class="workbench-browser-frame-area">
                        <Show
                            when=move || !wb.browser_url().get().trim().is_empty()
                            fallback=move || view! { <BrowserNewTabPane wb surface /> }.into_any()
                        >
                            <iframe
                                id=IFRAME_EMBED_ID
                                class="workbench-browser-iframe"
                                title=move || i18n.tr(I18nKey::BrFrameTitle)()
                                prop:src=move || wb.browser_url().get()
                            ></iframe>
                        </Show>
                    </div>
                }
                .into_any(),
            }}
        </div>
    }
}

fn ev_target_input(ev: &web_sys::Event) -> Option<web_sys::HtmlInputElement> {
    ev.target()?.dyn_into().ok()
}
