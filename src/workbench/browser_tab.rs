//! Eingebetteter Browser-Reiter: Toolbar + Host für native Child-Webview oder `<iframe>`-Fallback.
//! Pro Tab eine eigene Webview (Win/Mac) bzw. ein eigenes `<iframe>` (Linux) — Tab-Wechsel
//! verändert den Page-State nicht (kein Reload).
use crate::agent_wire::BrowserBoundsPayload;
use crate::config::NEW_TAB_BROWSER_SHORTLINKS;
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{browser_navigate, open_external_url};
use gloo_timers::future::TimeoutFuture;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashSet;
use wasm_bindgen::JsCast;

use crate::workbench::{BrowserEmbedSurface, RightPanelTab, WorkbenchService};

pub const EMBEDDED_BROWSER_HOST_ID: &str = "blx-embedded-browser-host";

/// Dispatched from `terminal_bootstrap.mjs` when the user activates a terminal web link.
pub const BLXCODE_OPEN_HTTP_EVENT: &str = "blxcode-open-http";

/// Opens a new embedded-browser tab and refreshes native webview / iframe bounds (same path as toolbar shortlinks).
pub fn open_http_in_embedded_browser(wb: WorkbenchService, surface: BrowserEmbedSurface, href: &str) {
    if wb.open_http_in_new_embedded_tab(href) {
        spawn_refresh_embedded_browser_nav(wb, surface);
    }
}

fn spawn_refresh_embedded_browser_nav(wb: WorkbenchService, surface: BrowserEmbedSurface) {
    spawn_local(async move {
        if embed_is_native(surface) {
            let aid = wb.embedded_browser_active_id().get_untracked();
            let u = active_tab_url(wb);
            if !u.trim().is_empty() {
                let _ = browser_navigate(aid, u.trim()).await;
            }
        }
        sync_embedded_browser_layer(wb, surface).await;
    });
}
const RIGHT_PANEL_BODY_ID: &str = "blx-right-panel-body";

fn iframe_id_for(tab_id: u64) -> String {
    format!("blx-browser-iframe-{tab_id}")
}

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

fn resolve_host_bounds() -> Option<BrowserBoundsPayload> {
    let win = web_sys::window()?;
    let doc = win.document()?;

    // Panel-Body als autoritatives Clip-Rect — schützt gegen CSS-Overflow, der
    // das Host-Element ausserhalb des Panels meldet.
    let panel = doc.get_element_by_id(RIGHT_PANEL_BODY_ID)?;
    let pr = panel.get_bounding_client_rect();

    let el = doc.get_element_by_id(EMBEDDED_BROWSER_HOST_ID)?;
    let r = el.get_bounding_client_rect();

    let x = r.left().max(pr.left());
    let y = r.top().max(pr.top());
    let right = r.right().min(pr.right());
    let bottom = r.bottom().min(pr.bottom());
    let w = (right - x).max(0.);
    let h = (bottom - y).max(0.);

    let visible = w >= 8.0 && h >= 8.0;
    Some(BrowserBoundsPayload {
        x,
        y,
        w,
        h,
        visible,
    })
}

fn embed_is_native(surface: BrowserEmbedSurface) -> bool {
    surface.0.get_untracked().as_deref() == Some("native_child")
}

fn active_tab_url(wb: WorkbenchService) -> String {
    let aid = wb.embedded_browser_active_id().get_untracked();
    wb.embedded_browser_tabs()
        .with_untracked(|tabs| tabs.iter().find(|t| t.id == aid).map(|t| t.url.clone()))
        .unwrap_or_default()
}

/// Bounds-Sync + Tab-Auswahl an die nativen Child-Webviews. Im iframe-Modus
/// räumt der Backend-Sync nur evtl. übrige native Webviews auf.
pub async fn sync_embedded_browser_layer(wb: WorkbenchService, surface: BrowserEmbedSurface) {
    let zero = BrowserBoundsPayload {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        visible: false,
    };

    if !embed_is_native(surface) {
        let _ = crate::tauri_bridge::browser_sync_bounds(None, zero, None).await;
        return;
    }

    let tab = wb.right_active_tab().get_untracked();
    let collapsed = wb.right_collapsed().get_untracked();

    if collapsed || tab != RightPanelTab::Browser {
        let _ = crate::tauri_bridge::browser_sync_bounds(None, zero, None).await;
        return;
    }

    let active_id = wb.embedded_browser_active_id().get_untracked();
    let current_url = active_tab_url(wb);
    let navigate_to = current_url.trim().to_string();
    let url_empty = navigate_to.is_empty();

    let mut payload = resolve_host_bounds().unwrap_or(zero);
    if url_empty {
        payload.visible = false;
    }

    let _ = crate::tauri_bridge::browser_sync_bounds(
        Some(active_id),
        payload,
        if url_empty {
            None
        } else {
            Some(navigate_to.as_str())
        },
    )
    .await;
}

fn try_reload_iframe_for(tab_id: u64) {
    let Some(w) = web_sys::window() else {
        return;
    };
    let Some(doc) = w.document() else {
        return;
    };
    let Some(el) = doc.get_element_by_id(&iframe_id_for(tab_id)) else {
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
                                        if embed_is_native(embed) {
                                            let url = active_tab_url(w);
                                            let aid = w.embedded_browser_active_id().get_untracked();
                                            if !url.trim().is_empty() {
                                                let _ = browser_navigate(aid, url.trim()).await;
                                            }
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
        let _aid = wb.embedded_browser_active_id().get();
        let _tabs = wb.embedded_browser_tabs().get();
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
    let draft_url = RwSignal::new(active_tab_url(wb));

    // Pro Tab markieren, ob das Embedding blockiert ist (X-Frame-Options / CSP).
    let failed_tabs: RwSignal<HashSet<u64>> = RwSignal::new(HashSet::new());

    // Draft-URL synchronisieren wenn der aktive Tab oder dessen URL wechselt.
    Effect::new(move |_| {
        let aid = wb.embedded_browser_active_id().get();
        let tabs = wb.embedded_browser_tabs().get();
        let url = tabs
            .iter()
            .find(|t| t.id == aid)
            .map(|t| t.url.clone())
            .unwrap_or_default();
        draft_url.set(url);
    });

    // Wenn sich URL eines Tabs ändert, im iframe-Modus den Iframable-Check fahren.
    Effect::new(move |_| {
        if embed_is_native(surface) {
            return;
        }
        let tabs = wb.embedded_browser_tabs().get();
        for tab in tabs {
            let tid = tab.id;
            let url = tab.url.trim().to_string();
            if url.is_empty() {
                failed_tabs.update(|s| {
                    s.remove(&tid);
                });
                continue;
            }
            // Probe nur einmalig pro (tab, url) — bei Wechsel der URL erneut.
            spawn_local(async move {
                match crate::tauri_bridge::browser_check_iframable(&url).await {
                    Ok(true) => {
                        failed_tabs.update(|s| {
                            s.remove(&tid);
                        });
                    }
                    Ok(false) => {
                        failed_tabs.update(|s| {
                            s.insert(tid);
                        });
                    }
                    Err(_) => {}
                }
            });
        }
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
                                            failed_tabs.update(|s| { s.remove(&tid); });
                                            let w = wb;
                                            let e = surface;
                                            spawn_local(async move {
                                                // Native Child-Webview für diesen Tab abräumen.
                                                let _ = crate::tauri_bridge::browser_close_tab(tid).await;
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
                    prop:disabled=move || !wb.tab_can_go_back()
                    on:click=move |_| {
                        let Some(target) = wb.tab_navigate_back() else { return; };
                        let embed = surface;
                        let aid = wb.embedded_browser_active_id().get_untracked();
                        spawn_local(async move {
                            if embed_is_native(embed) {
                                let _ = browser_navigate(aid, target.trim()).await;
                            }
                            // iframe path: prop:src is bound to the tab url
                            // memo, which we just mutated, so the iframe
                            // reloads automatically.
                            sync_embedded_browser_layer(wb, embed).await;
                        });
                    }
                >
                    "←"
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    aria-label=move || i18n.tr(I18nKey::BrFwd)()
                    prop:disabled=move || !wb.tab_can_go_forward()
                    on:click=move |_| {
                        let Some(target) = wb.tab_navigate_forward() else { return; };
                        let embed = surface;
                        let aid = wb.embedded_browser_active_id().get_untracked();
                        spawn_local(async move {
                            if embed_is_native(embed) {
                                let _ = browser_navigate(aid, target.trim()).await;
                            }
                            sync_embedded_browser_layer(wb, embed).await;
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
                            let aid = w.embedded_browser_active_id().get_untracked();
                            if embed_is_native(embed) {
                                let url = active_tab_url(w);
                                if !url.trim().is_empty() {
                                    let _ = browser_navigate(aid, url.trim()).await;
                                }
                            } else {
                                try_reload_iframe_for(aid);
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
                            wb.persist_browser_url_from_input(url);
                            let w = wb;
                            let embed = surface;
                            spawn_local(async move {
                                if embed_is_native(embed) {
                                    let aid = w.embedded_browser_active_id().get_untracked();
                                    let u = active_tab_url(w);
                                    if !u.trim().is_empty() {
                                        let _ = browser_navigate(aid, u.trim()).await;
                                    }
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
                        wb.persist_browser_url_from_input(url);
                        let w = wb;
                        let embed = surface;
                        spawn_local(async move {
                            if embed_is_native(embed) {
                                let aid = w.embedded_browser_active_id().get_untracked();
                                let u = active_tab_url(w);
                                if !u.trim().is_empty() {
                                    let _ = browser_navigate(aid, u.trim()).await;
                                }
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
                }.into_any(),

                // Native: ein einziger Host-Slot — Backend platziert die echte
                // Webview für `active_tab_id` an dessen Rect.
                Some("native_child") => view! {
                    <div
                        id=EMBEDDED_BROWSER_HOST_ID
                        class="workbench-browser-host"
                        aria-label=move || i18n.tr(I18nKey::BrNativeAria)()
                    >
                        <Show
                            when=move || active_tab_url(wb).trim().is_empty()
                            fallback=move || ().into_any()
                        >
                            <BrowserNewTabPane wb surface />
                        </Show>
                    </div>
                }.into_any(),

                // iframe: ein eigenes <iframe> pro Tab, alle gleichzeitig im DOM,
                // nur das aktive sichtbar — Tab-Wechsel = `display: block/none`,
                // kein Reload.
                _ => view! {
                    <div id=EMBEDDED_BROWSER_HOST_ID class="workbench-browser-host">
                        <For
                            each=move || wb.embedded_browser_tabs().get()
                            key=|t| t.id
                            children=move |tab| {
                                let tid = tab.id;
                                let url_memo = Memo::new(move |_| {
                                    wb.embedded_browser_tabs().with(|tabs| {
                                        tabs.iter().find(|t| t.id == tid)
                                            .map(|t| t.url.clone())
                                            .unwrap_or_default()
                                    })
                                });
                                view! {
                                    <div
                                        class="workbench-browser-frame-area"
                                        class:workbench-browser-frame-area--hidden=move || {
                                            wb.embedded_browser_active_id().get() != tid
                                        }
                                    >
                                        <Show
                                            when=move || !url_memo.with(|u| u.trim().is_empty())
                                            fallback=move || view! { <BrowserNewTabPane wb surface /> }.into_any()
                                        >
                                            <Show
                                                when=move || !failed_tabs.with(|s| s.contains(&tid))
                                                fallback=move || {
                                                    let target = url_memo.get_untracked();
                                                    view! {
                                                        <div class="workbench-browser-new-tab">
                                                            <p class="workbench-browser-new-tab-hint">
                                                                "This page blocks iframe embedding in the app."
                                                            </p>
                                                            <button
                                                                type="button"
                                                                class="workbench-mini-btn workbench-mini-btn--primary"
                                                                on:click=move |_| {
                                                                    let t = target.clone();
                                                                    if t.trim().is_empty() { return; }
                                                                    spawn_local(async move {
                                                                        let _ = open_external_url(t.trim()).await;
                                                                    });
                                                                }
                                                            >
                                                                "Open In Browser"
                                                            </button>
                                                        </div>
                                                    }.into_any()
                                                }
                                            >
                                                <iframe
                                                    id=iframe_id_for(tid)
                                                    class="workbench-browser-iframe"
                                                    title=move || i18n.tr(I18nKey::BrFrameTitle)()
                                                    prop:src=move || url_memo.get()
                                                ></iframe>
                                            </Show>
                                        </Show>
                                    </div>
                                }
                            }
                        />
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

fn ev_target_input(ev: &web_sys::Event) -> Option<web_sys::HtmlInputElement> {
    ev.target()?.dyn_into().ok()
}
