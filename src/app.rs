use crate::config::EULA_STORAGE_KEY;
use crate::i18n::{localized_eula_html, I18nKey};
use crate::open_http::dom_click_http_url_from_mouse_event;
use crate::quit::request_app_quit;
use crate::service::I18nService;
use crate::workbench::WorkbenchShell;
use leptos::prelude::*;
use leptos::task::spawn_local;
use send_wrapper::SendWrapper;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[component]
pub fn App() -> impl IntoView {
    let i18n = I18nService::new();
    provide_context(i18n);

    Effect::new(move |_| {
        let lang = i18n.locale().get().as_str();
        if let Some(w) = web_sys::window() {
            if let Some(doc) = w.document() {
                if let Some(root) = doc.document_element() {
                    let _ = root.set_attribute("lang", lang);
                }
            }
        }
    });

    let (ui_ready, set_ui_ready) = signal(false);
    let (eula_ok, set_eula_ok) = signal(false);

    Effect::new(move |_| {
        let stored = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(EULA_STORAGE_KEY).ok().flatten());

        set_eula_ok.set(stored.as_deref() == Some("1"));
        set_ui_ready.set(true);
    });

    Effect::new(move |_| {
        if !ui_ready.get() {
            return;
        }
        if eula_ok.get() {
            return;
        }
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let doc_click = Closure::wrap(Box::new(move |ev: web_sys::Event| {
            let Some(mouse) = ev.dyn_ref::<web_sys::MouseEvent>() else {
                return;
            };
            let Some(url) = dom_click_http_url_from_mouse_event(mouse) else {
                return;
            };
            ev.prevent_default();
            ev.stop_propagation();
            open_external(&url);
        }) as Box<dyn FnMut(_)>);
        let _ = doc.add_event_listener_with_callback_and_bool(
            "click",
            doc_click.as_ref().unchecked_ref(),
            true,
        );
        let doc_click = SendWrapper::new(doc_click);
        let doc_cleanup = doc.clone();
        on_cleanup(move || {
            let c = doc_click.take();
            let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                "click",
                c.as_ref().unchecked_ref(),
                true,
            );
        });
    });

    let accept = move |_| {
        if let Some(w) = web_sys::window() {
            if let Ok(Some(s)) = w.local_storage() {
                let _ = s.set_item(EULA_STORAGE_KEY, "1");
            }
        }
        set_eula_ok.set(true);
    };

    let decline = move |_| {
        request_app_quit();
    };

    let eula_html = Memo::new(move |_prev| {
        let loc = i18n.locale().get();
        localized_eula_html(loc)
    });

    view! {
        <Show
            when=move || ui_ready.get()
            fallback=|| view! { <div class="app-shell app-shell--boot" aria-busy="true"></div> }
        >
            <Show
                when=move || !eula_ok.get()
                fallback=move || view! { <WorkbenchShell/> }
            >
                <div class="eula-root">
                    <div class="eula-scrim" aria-hidden="true"></div>
                    <div
                        class="eula-sheet"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="eula-heading"
                    >
                        <div class="eula-scroll eula-md" inner_html=eula_html></div>

                        <footer class="eula-actions">
                            <button type="button" class="eula-btn eula-btn--ghost" on:click=decline>
                                {move || i18n.tr(I18nKey::Decline)()}
                            </button>
                            <button type="button" class="eula-btn eula-btn--primary" on:click=accept>
                                {move || i18n.tr(I18nKey::Accept)()}
                            </button>
                        </footer>
                    </div>
                </div>
            </Show>
        </Show>
    }
}

fn open_external(url: &str) {
    if crate::tauri_bridge::is_tauri_shell() {
        let owned = url.to_string();
        spawn_local(async move {
            if crate::tauri_bridge::open_external_url(&owned)
                .await
                .is_err()
            {
                open_via_dom_window(&owned);
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
