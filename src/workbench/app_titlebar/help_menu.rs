//! HELP popover: product metadata, docs/community links, embedded website,
//! and the same manual update check exposed from Settings.
use crate::tauri_bridge::{is_tauri_shell, open_external_url};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::{JsCast, JsValue};

pub(crate) const BLXCODE_CHECK_UPDATE_EVENT: &str = "blxcode-check-update";

const DOCS_URL: &str = "https://blxcode.com/docs/";
const DISCUSSIONS_URL: &str = "https://github.com/Bitslix/BLXCode/discussions";
const ISSUES_URL: &str = "https://github.com/Bitslix/BLXCode/issues";
const KOFI_URL: &str = "https://ko-fi.com/bitslix";
const REPO_URL: &str = "https://github.com/Bitslix/BLXCode";
const WEBSITE_URL: &str = "https://blxcode.com";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[component]
pub fn HelpMenu() -> impl IntoView {
    let open = RwSignal::new(false);
    let about_open = RwSignal::new(false);

    // Close on outside-click / Esc while the popover is open.
    let close_click = window_event_listener_untyped("click", move |ev| {
        if !open.get_untracked() {
            return;
        }
        let inside = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".app-titlebar__menu-wrap").ok().flatten())
            .is_some();
        if !inside {
            open.set(false);
        }
    });
    let close_esc = window_event_listener_untyped("keydown", move |ev| {
        let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if ev.key() == "Escape" {
            open.set(false);
            about_open.set(false);
        }
    });
    on_cleanup(move || {
        close_click.remove();
        close_esc.remove();
    });

    let show_about = move |_| {
        open.set(false);
        about_open.set(true);
    };
    let open_docs = move |_| {
        open.set(false);
        open_external(DOCS_URL);
    };
    let open_discussions = move |_| {
        open.set(false);
        open_external(DISCUSSIONS_URL);
    };
    let open_issues = move |_| {
        open.set(false);
        open_external(ISSUES_URL);
    };
    let open_website = move |_| {
        open.set(false);
        dispatch_open_http(WEBSITE_URL);
    };
    let check_update = move |_| {
        open.set(false);
        dispatch_simple_event(BLXCODE_CHECK_UPDATE_EVENT);
    };

    view! {
        <div class="app-titlebar__menu-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label="Help"
                title="Help"
                on:click=move |ev| {
                    ev.stop_propagation();
                    open.update(|o| *o = !*o);
                }
            >
                <LxIcon icon=icondata::LuCircleHelp width="1rem" height="1rem" />
            </button>
            <Show when=move || open.get()>
                <div class="app-titlebar__popover app-titlebar__popover--help" role="menu">
                    <p class="app-titlebar__popover-head">"Help"</p>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=show_about
                    >
                        <LxIcon icon=icondata::LuBadgeInfo width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">"About"</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=open_docs
                    >
                        <LxIcon icon=icondata::LuBookOpen width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">"Docs"</span>
                    </button>
                    <div class="app-titlebar__menu-sep" role="separator"></div>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=open_discussions
                    >
                        <LxIcon icon=icondata::LuMessagesSquare width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">"Discuss"</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=open_issues
                    >
                        <LxIcon icon=icondata::LuBug width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">"Report Bug"</span>
                    </button>
                    <div class="app-titlebar__menu-sep" role="separator"></div>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=open_website
                    >
                        <LxIcon icon=icondata::LuGlobe width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">"Website"</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=check_update
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">"Update"</span>
                    </button>
                </div>
            </Show>
            <Show when=move || about_open.get()>
                <div class="app-titlebar__about-overlay" role="presentation">
                    <button
                        type="button"
                        class="app-titlebar__about-scrim"
                        tabindex="-1"
                        aria-label="Close"
                        on:click=move |_| about_open.set(false)
                    ></button>
                    <section
                        class="app-titlebar__about-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="app-titlebar-about-title"
                    >
                        <header class="app-titlebar__about-head">
                            <button
                                type="button"
                                class="app-titlebar__icon-btn"
                                aria-label="Close"
                                title="Close"
                                on:click=move |_| about_open.set(false)
                            >
                                <LxIcon icon=icondata::LuX width="0.95rem" height="0.95rem" />
                            </button>
                        </header>
                        <div class="app-titlebar__about-body">
                            <div class="app-titlebar__about-hero">
                                <span class="app-titlebar__about-logo" aria-hidden="true">
                                    <img src="/blxcode.png" alt="" />
                                </span>
                                <p class="app-titlebar__about-kicker">"Open-source AI workbench"</p>
                                <h2 id="app-titlebar-about-title">"BLXCode"</h2>
                                <p class="app-titlebar__about-desc">
                                    "Local-first desktop workbench for AI-assisted development: terminals, agent, memory, plans, Git, file preview, and an embedded browser in one Tauri shell."
                                </p>
                            </div>

                            <div class="app-titlebar__about-status" aria-label="Project status">
                                <span class="app-titlebar__about-status-pill app-titlebar__about-status-pill--free">
                                    <LxIcon icon=icondata::LuShieldCheck width="0.8rem" height="0.8rem" />
                                    <span>"Yes, Free!"</span>
                                </span>
                                <span class="app-titlebar__about-status-pill">
                                    <LxIcon icon=icondata::LuGithub width="0.8rem" height="0.8rem" />
                                    <span>"Open Source"</span>
                                </span>
                                <span class="app-titlebar__about-status-pill">
                                    <span>"MIT"</span>
                                </span>
                            </div>

                            <dl class="app-titlebar__about-meta">
                                <div>
                                    <dt>"Version"</dt>
                                    <dd>{APP_VERSION}</dd>
                                </div>
                                <div>
                                    <dt>"Inventor & main author"</dt>
                                    <dd>"Bitslix / iptoux"</dd>
                                </div>
                            </dl>

                            <div class="app-titlebar__about-stack" aria-label="Stack">
                                <span>"Rust 2021"</span>
                                <span>"Tauri 2"</span>
                                <span>"Leptos 0.8"</span>
                                <span>"Linux · macOS · Windows"</span>
                                <span>"14 locales"</span>
                                <span>"20 themes"</span>
                            </div>

                            <div class="app-titlebar__about-links" aria-label="Project links">
                                <button
                                    type="button"
                                    class="app-titlebar__about-link app-titlebar__about-link--primary"
                                    on:click=move |_| open_external(KOFI_URL)
                                >
                                    <LxIcon icon=icondata::LuSparkles width="0.88rem" height="0.88rem" />
                                    <span>"Sponsor / Ko-fi"</span>
                                </button>
                                <button
                                    type="button"
                                    class="app-titlebar__about-link"
                                    on:click=move |_| open_external(REPO_URL)
                                >
                                    <LxIcon icon=icondata::LuGithub width="0.88rem" height="0.88rem" />
                                    <span>"Repository"</span>
                                </button>
                                <button
                                    type="button"
                                    class="app-titlebar__about-link"
                                    on:click=move |_| dispatch_open_http(WEBSITE_URL)
                                >
                                    <LxIcon icon=icondata::LuGlobe width="0.88rem" height="0.88rem" />
                                    <span>"Website"</span>
                                </button>
                                <button
                                    type="button"
                                    class="app-titlebar__about-link"
                                    on:click=move |_| open_external(ISSUES_URL)
                                >
                                    <LxIcon icon=icondata::LuBug width="0.88rem" height="0.88rem" />
                                    <span>"Issues"</span>
                                </button>
                                <button
                                    type="button"
                                    class="app-titlebar__about-link"
                                    on:click=move |_| open_external(DISCUSSIONS_URL)
                                >
                                    <LxIcon icon=icondata::LuMessagesSquare width="0.88rem" height="0.88rem" />
                                    <span>"Discussions"</span>
                                </button>
                            </div>
                        </div>
                    </section>
                </div>
            </Show>
        </div>
    }
}

fn open_external(url: &'static str) {
    if is_tauri_shell() {
        spawn_local(async move {
            if open_external_url(url).await.is_err() {
                open_via_dom_window(url);
            }
        });
        return;
    }
    open_via_dom_window(url);
}

fn open_via_dom_window(url: &str) {
    let Some(win) = web_sys::window() else {
        return;
    };
    let opened = win.open_with_url_and_target(url, "_blank").ok().flatten();
    if opened.is_none() {
        let _ = win.location().set_href(url);
    }
}

fn dispatch_open_http(url: &str) {
    let detail = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&detail, &JsValue::from_str("url"), &JsValue::from_str(url));
    if let Ok(ev) =
        web_sys::CustomEvent::new(crate::workbench::browser_tab::BLXCODE_OPEN_HTTP_EVENT)
    {
        let _ = js_sys::Reflect::set(&ev, &JsValue::from_str("detail"), &detail);
        dispatch_event(&ev);
    }
}

fn dispatch_simple_event(name: &str) {
    if let Ok(ev) = web_sys::CustomEvent::new(name) {
        dispatch_event(&ev);
    }
}

fn dispatch_event(ev: &web_sys::CustomEvent) {
    if let Some(win) = web_sys::window() {
        let _ = win.dispatch_event(ev);
    }
}
