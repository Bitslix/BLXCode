use crate::auth::{fetch_auth_session, sign_out, AuthEnv, AuthGateState, LoginModal};
use crate::config::{AUTH_DEVICE_BEARER_KEY, EULA_STORAGE_KEY};
use crate::i18n::{localized_eula_html, I18nKey};
use crate::quit::request_app_quit;
use crate::service::{ApiService, I18nService};
use crate::workbench::WorkbenchShell;
use leptos::callback::UnsyncCallback;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn App() -> impl IntoView {
    let api_svc = ApiService::new();
    provide_context(api_svc.clone());

    let i18n = I18nService::new();
    provide_context(i18n);

    let gate = RwSignal::new(AuthGateState::CheckingSession);
    let persisted_bearer = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(AUTH_DEVICE_BEARER_KEY).ok().flatten())
        .filter(|s| !s.is_empty());
    let bearer = RwSignal::new(persisted_bearer);
    let profile = RwSignal::new(Option::<crate::auth::AuthUserBrief>::None);
    let request_logout = UnsyncCallback::new({
        let api = api_svc.clone();
        move |_| {
            let api_inner = api.clone();
            spawn_local(async move {
                let tok = bearer.get_untracked();
                let _ = sign_out(&api_inner, tok.as_deref()).await;
                bearer.set(None);
                profile.set(None);
                gate.set(AuthGateState::NeedLogin);
                if let Some(w) = web_sys::window() {
                    if let Ok(Some(s)) = w.local_storage() {
                        let _ = s.remove_item(AUTH_DEVICE_BEARER_KEY);
                    }
                }
            });
        }
    });
    provide_context(AuthEnv {
        gate,
        bearer,
        profile,
        logout: request_logout,
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
    let (eula_ok, set_eula_ok) = signal(false);

    Effect::new(move |_| {
        let stored = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(EULA_STORAGE_KEY).ok().flatten());

        set_eula_ok.set(stored.as_deref() == Some("1"));
        set_ui_ready.set(true);
    });

    Effect::new({
        let api = api_svc.clone();
        move |_| {
            if !ui_ready.get() {
                return;
            }
            if !eula_ok.get() {
                return;
            }
            spawn_local({
                let api = api.clone();
                async move {
                    gate.set(AuthGateState::CheckingSession);
                    match fetch_auth_session(&api, bearer.get_untracked().as_deref()).await {
                        Ok(Some(p)) => {
                            profile.set(Some(p));
                            gate.set(AuthGateState::LoggedIn);
                        }
                        Ok(None) => {
                            profile.set(None);
                            gate.set(AuthGateState::NeedLogin);
                        }
                        Err(_) => {
                            profile.set(None);
                            gate.set(AuthGateState::NeedLogin);
                        }
                    }
                }
            });
        }
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
                fallback=move || view! {
                    <Show
                        when=move || gate.get() == AuthGateState::LoggedIn
                        fallback=move || view! {
                            <Show
                                when=move || gate.get() == AuthGateState::NeedLogin
                                fallback=move || view! {
                                    <div class="app-shell app-shell--boot auth-gate-busy" aria-busy="true">
                                        <p class="auth-gate-busy-msg">{move || i18n.tr(I18nKey::AuthGateChecking)()}</p>
                                    </div>
                                }
                            >
                                <LoginModal/>
                            </Show>
                        }
                    >
                        <WorkbenchShell/>
                    </Show>
                }
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
