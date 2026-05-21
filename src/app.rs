use crate::boot_loading::{BootLoadingScreen, BootPhase};
use crate::config::{
    EULA_STORAGE_KEY, GITIGNORE_PROMPT_ANSWER_NO, GITIGNORE_PROMPT_ANSWER_YES,
    GITIGNORE_PROMPT_STORAGE_KEY, HARNESS_WORKSPACE_ROOT_KEY,
};
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
        remove_static_boot_screen();
    });

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
    let (app_boot_phase, set_app_boot_phase) = signal(BootPhase::Starting);
    let (eula_ok, set_eula_ok) = signal(false);
    let (gitignore_gate_ok, set_gitignore_gate_ok) = signal(false);
    let (gitignore_busy, set_gitignore_busy) = signal(false);

    Effect::new(move |_| {
        let stored = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(EULA_STORAGE_KEY).ok().flatten());

        set_eula_ok.set(stored.as_deref() == Some("1"));
        set_gitignore_gate_ok
            .set(gitignore_prompt_was_answered() || !crate::tauri_bridge::is_tauri_shell());
        set_app_boot_phase.set(BootPhase::OpeningWorkbench);
        set_ui_ready.set(true);
    });

    // After EULA flips to accepted, re-read storage so a prior answer is never ignored.
    Effect::new(move |_| {
        if !ui_ready.get() || !eula_ok.get() {
            return;
        }
        if gitignore_prompt_was_answered() {
            set_gitignore_gate_ok.set(true);
        }
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

    let finish_gitignore_prompt = move |answer: &'static str| {
        persist_gitignore_prompt_answer(answer);
        set_gitignore_gate_ok.set(true);
        set_gitignore_busy.set(false);
    };

    let gitignore_no = move |_| {
        if gitignore_busy.get_untracked() || gitignore_prompt_was_answered() {
            return;
        }
        finish_gitignore_prompt(GITIGNORE_PROMPT_ANSWER_NO);
    };

    let gitignore_yes = move |_| {
        if gitignore_busy.get_untracked() || gitignore_prompt_was_answered() {
            return;
        }
        set_gitignore_busy.set(true);
        // Persist immediately so the prompt is never shown again, even if append fails.
        persist_gitignore_prompt_answer(GITIGNORE_PROMPT_ANSWER_YES);
        set_gitignore_gate_ok.set(true);
        spawn_local(async move {
            if let Some(path) = resolve_gitignore_workspace_cwd().await {
                let _ = crate::tauri_bridge::gitignore_append_blxcode(&path).await;
            }
            set_gitignore_busy.set(false);
        });
    };

    let eula_html = Memo::new(move |_prev| {
        let loc = i18n.locale().get();
        localized_eula_html(loc)
    });

    let show_workbench = move || eula_ok.get() && gitignore_gate_ok.get();
    let show_gitignore = move || eula_ok.get() && !gitignore_gate_ok.get();
    let show_eula = move || !eula_ok.get();

    view! {
        <Show
            when=move || ui_ready.get()
            fallback=move || view! { <BootLoadingScreen phase=app_boot_phase.get()/> }
        >
            <Show when=show_workbench fallback=move || view! {
                <Show
                    when=show_gitignore
                    fallback=move || view! {
                        <Show when=show_eula>
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
                    }
                >
                    <div class="eula-root">
                        <div class="eula-scrim" aria-hidden="true"></div>
                        <div
                            class="eula-sheet eula-sheet--compact"
                            role="dialog"
                            aria-modal="true"
                            aria-labelledby="gitignore-prompt-title"
                        >
                            <div class="eula-scroll">
                                <h1 id="gitignore-prompt-title" class="gitignore-prompt__title">
                                    {move || i18n.tr(I18nKey::GitignorePromptTitle)()}
                                </h1>
                                <p class="gitignore-prompt__body">
                                    {move || i18n.tr(I18nKey::GitignorePromptBody)()}
                                </p>
                            </div>

                            <footer class="eula-actions">
                                <button
                                    type="button"
                                    class="eula-btn eula-btn--ghost"
                                    prop:disabled=move || gitignore_busy.get()
                                    on:click=gitignore_no
                                >
                                    {move || i18n.tr(I18nKey::GitignorePromptNo)()}
                                </button>
                                <button
                                    type="button"
                                    class="eula-btn eula-btn--primary"
                                    prop:disabled=move || gitignore_busy.get()
                                    on:click=gitignore_yes
                                >
                                    {move || i18n.tr(I18nKey::GitignorePromptYes)()}
                                </button>
                            </footer>
                        </div>
                    </div>
                </Show>
            }>
                <WorkbenchShell/>
            </Show>
        </Show>
    }
}

fn remove_static_boot_screen() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(el) = doc.get_element_by_id("blx-static-boot") {
        el.remove();
    }
}

fn gitignore_prompt_was_answered() -> bool {
    read_gitignore_prompt_answer()
        .as_deref()
        .is_some_and(|a| a == GITIGNORE_PROMPT_ANSWER_YES || a == GITIGNORE_PROMPT_ANSWER_NO)
}

fn read_gitignore_prompt_answer() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(GITIGNORE_PROMPT_STORAGE_KEY).ok().flatten())
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn persist_gitignore_prompt_answer(answer: &str) {
    if let Some(w) = web_sys::window() {
        if let Ok(Some(s)) = w.local_storage() {
            let _ = s.set_item(GITIGNORE_PROMPT_STORAGE_KEY, answer);
        }
    }
}

async fn resolve_gitignore_workspace_cwd() -> Option<String> {
    if let Some(w) = web_sys::window() {
        if let Ok(Some(s)) = w.local_storage() {
            if let Ok(Some(root)) = s.get_item(HARNESS_WORKSPACE_ROOT_KEY) {
                let t = root.trim();
                if !t.is_empty() {
                    return Some(t.to_owned());
                }
            }
        }
    }
    crate::tauri_bridge::harness_ensure_default_sandbox()
        .await
        .ok()
        .filter(|p| !p.trim().is_empty())
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
