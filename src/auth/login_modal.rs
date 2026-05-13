use crate::auth::{
    open_in_new_tab, poll_device_token, promote_if_session_valid, request_device_code,
    sign_in_email, verification_url_open, AuthEnv, DevicePollOutcome, SignInEmailPayload,
};
use crate::config::AUTH_DEVICE_BEARER_KEY;
use crate::i18n::I18nKey;
use crate::service::{ApiService, I18nService};
use gloo_timers::future::TimeoutFuture;
use icondata::LuClipboardCheck;
use icondata::LuCopy;
use leptos_icons::Icon as LxIcon;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/** Länge der sichtbaren Gruppen (z. B. `XXXX-YYYY`). */
const USER_CODE_CHARS_PER_GROUP: usize = 4;

fn user_code_normalized(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

/// Gruppierung für die Anzeige (z. B. `A9S7 - PR32` aus `a9s7-pr32`).
fn user_code_display_chunks(raw: &str) -> Vec<String> {
    let clean = user_code_normalized(raw);
    if clean.is_empty() {
        return Vec::new();
    }
    let v: Vec<char> = clean.chars().collect();
    v.chunks(USER_CODE_CHARS_PER_GROUP)
        .map(|ch| ch.iter().collect::<String>())
        .collect()
}

#[must_use]
/// Zum Einfügen: `XXXX-YYYY` wie auf dem Bildschirm.
fn user_code_for_clipboard(raw: &str) -> String {
    let parts = user_code_display_chunks(raw);
    if parts.is_empty() {
        return String::new();
    }
    parts.join("-")
}

async fn write_clipboard(text: &str) -> bool {
    let Some(w) = web_sys::window() else {
        return false;
    };
    let promise = w.navigator().clipboard().write_text(text);
    JsFuture::from(promise).await.is_ok()
}

#[component]
fn AuthEmailPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let api = expect_context::<ApiService>();
    let auth = expect_context::<AuthEnv>();

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let err = RwSignal::new(Option::<String>::None);
    let busy = RwSignal::new(false);

    view! {
        <form
            class="auth-login-form"
            on:submit=move |ev: leptos::ev::SubmitEvent| {
                ev.prevent_default();
                err.set(None);
                busy.set(true);
                let api = api.clone();
                spawn_local(async move {
                    auth.bearer.set(None);
                    let body = SignInEmailPayload {
                        email: email.get_untracked().trim().to_string(),
                        password: password.get_untracked(),
                        remember_me: Some(true),
                    };
                    match sign_in_email(&api, &body, None).await {
                        Ok(()) => match promote_if_session_valid(auth, &api).await {
                            Ok(true) => {}
                            Ok(false) => err.set(Some(i18n.tr(I18nKey::AuthFail)().to_string())),
                            Err(e) => err.set(Some(e.to_string())),
                        },
                        Err(e) => err.set(Some(e.to_string())),
                    }
                    busy.set(false);
                });
            }
        >
            <label class="auth-login-field">
                <span>{move || i18n.tr(I18nKey::AuthEmailLabel)()}</span>
                <input
                    type="email"
                    autocomplete="username"
                    prop:value=move || email.get()
                    on:input=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(i) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                email.set(i.value());
                            }
                        }
                    }
                />
            </label>
            <label class="auth-login-field">
                <span>{move || i18n.tr(I18nKey::AuthPasswordLabel)()}</span>
                <input
                    type="password"
                    autocomplete="current-password"
                    prop:value=move || password.get()
                    on:input=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(i) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                password.set(i.value());
                            }
                        }
                    }
                />
            </label>
            <button
                type="submit"
                class="eula-btn eula-btn--primary auth-login-submit"
                prop:disabled=move || busy.get()
            >
                {move || i18n.tr(I18nKey::AuthSubmit)()}
            </button>
        </form>
        <Show when=move || err.get().is_some()>
            <p class="auth-login-error" role="alert">{move || err.get().unwrap_or_default()}</p>
        </Show>
    }
}

#[component]
fn AuthDevicePane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let api_stored = StoredValue::new(expect_context::<ApiService>());
    let auth = expect_context::<AuthEnv>();

    let err = RwSignal::new(Option::<String>::None);
    let busy = RwSignal::new(false);

    let device_code_vis = RwSignal::new(Option::<String>::None);
    let verification_url = RwSignal::new(Option::<String>::None);
    let polling = RwSignal::new(false);
    let device_secret = RwSignal::new(Option::<String>::None);
    let copied_feedback = RwSignal::new(false);

    view! {
        <Show when=move || device_code_vis.get().is_none()>
            <p class="auth-login-lead">{move || i18n.tr(I18nKey::AuthDeviceIntro)()}</p>
            <button
                type="button"
                class="eula-btn eula-btn--primary auth-login-submit"
                prop:disabled=move || busy.get()
                on:click=move |_| {
                    let api = api_stored.with_value(|a| a.clone());
                    let i18n = i18n;
                    err.set(None);
                    busy.set(true);
                    device_code_vis.set(None);
                    verification_url.set(None);
                    polling.set(false);
                    device_secret.set(None);
                    copied_feedback.set(false);
                    spawn_local(async move {
                        match request_device_code(&api, None, None).await {
                            Ok(dc) => {
                                let open = dc
                                    .verification_uri_complete
                                    .as_deref()
                                    .unwrap_or(dc.verification_uri.as_str());
                                let open_abs = verification_url_open(open);
                                device_secret.set(Some(dc.device_code.clone()));
                                device_code_vis.set(Some(dc.user_code));
                                verification_url.set(Some(open_abs.clone()));
                                open_in_new_tab(&open_abs);
                                polling.set(true);
                                busy.set(false);

                                let mut interval_secs = dc.interval.max(2);
                                spawn_local(async move {
                                    loop {
                                        let ms_u64 = interval_secs.saturating_mul(1000);
                                        TimeoutFuture::new(ms_u64.min(u64::from(u32::MAX)) as u32).await;
                                        let Some(secret) = device_secret.get_untracked() else {
                                            break;
                                        };
                                        match poll_device_token(&api, secret.as_str(), None).await {
                                            Ok(DevicePollOutcome::Success(t)) => {
                                                polling.set(false);
                                                auth.bearer.set(Some(t.access_token.clone()));
                                                if let Some(w) = web_sys::window() {
                                                    if let Ok(Some(s)) = w.local_storage() {
                                                        let _ = s.set_item(AUTH_DEVICE_BEARER_KEY, &t.access_token);
                                                    }
                                                }
                                                match promote_if_session_valid(auth, &api).await {
                                                    Ok(true) => break,
                                                    Ok(false) | Err(_) => {
                                                        auth.bearer.set(None);
                                                        if let Some(w) = web_sys::window() {
                                                            if let Ok(Some(s)) = w.local_storage() {
                                                                let _ = s.remove_item(AUTH_DEVICE_BEARER_KEY);
                                                            }
                                                        }
                                                        err.set(Some(
                                                            i18n.tr(I18nKey::AuthFail)().to_string(),
                                                        ));
                                                    }
                                                }
                                                break;
                                            }
                                            Ok(DevicePollOutcome::AuthorizationPending) => {}
                                            Ok(DevicePollOutcome::SlowDown) => {
                                                interval_secs = interval_secs.saturating_add(5);
                                            }
                                            Ok(DevicePollOutcome::Denied(m)) => {
                                                polling.set(false);
                                                err.set(Some(m));
                                                break;
                                            }
                                            Err(e) => {
                                                polling.set(false);
                                                err.set(Some(e.to_string()));
                                                break;
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                err.set(Some(e.to_string()));
                                busy.set(false);
                            }
                        }
                    });
                }
            >
                {move || i18n.tr(I18nKey::AuthDeviceStart)()}
            </button>
        </Show>
        <Show when=move || device_code_vis.get().is_some()>
            <div class="auth-device-code-card">
                <button
                    type="button"
                    class="auth-device-code-copy"
                    aria-label=move || i18n.tr(I18nKey::AuthDeviceCopyAria)()
                    title=move || i18n.tr(I18nKey::AuthDeviceCopyAria)()
                    on:click=move |_| {
                        let Some(raw) = device_code_vis.get_untracked() else {
                            return;
                        };
                        let text = user_code_for_clipboard(&raw);
                        if text.is_empty() {
                            return;
                        };
                        copied_feedback.set(false);
                        spawn_local(async move {
                            let ok = write_clipboard(&text).await;
                            if !ok {
                                return;
                            }
                            copied_feedback.set(true);
                            TimeoutFuture::new(1_800_u32).await;
                            copied_feedback.set(false);
                        });
                    }
                >
                    <Show
                        when=move || copied_feedback.get()
                        fallback=move || view! {
                            <LxIcon icon=LuCopy width="1.15rem" height="1.15rem" />
                        }
                    >
                        <LxIcon icon=LuClipboardCheck width="1.15rem" height="1.15rem" />
                    </Show>
                </button>
                <p class="auth-device-code-label">
                    {move || i18n.tr(I18nKey::AuthDeviceCode)()}
                </p>
                <div
                    class="auth-device-code-chunks"
                    role="status"
                    aria-live="polite"
                    aria-label=move || user_code_for_clipboard(
                        device_code_vis.get().unwrap_or_default().as_str(),
                    )
                >
                    {move || {
                        let chunks = user_code_display_chunks(
                            device_code_vis.get().unwrap_or_default().as_str(),
                        );
                        chunks
                            .into_iter()
                            .enumerate()
                            .map(|(i, chunk)| {
                                if i == 0 {
                                    view! { <span class="auth-device-code-chunk">{chunk}</span> }
                                        .into_any()
                                } else {
                                    view! {
                                        <>
                                            <span class="auth-device-code-sep" aria-hidden="true">
                                                "-"
                                            </span>
                                            <span class="auth-device-code-chunk">{chunk}</span>
                                        </>
                                    }
                                    .into_any()
                                }
                            })
                            .collect_view()
                    }}
                </div>
                <span class="workbench-visually-hidden" role="status" aria-live="polite">
                    {move || {
                        if copied_feedback.get() {
                            i18n.tr(I18nKey::AuthDeviceCopied)().to_string()
                        } else {
                            String::new()
                        }
                    }}
                </span>
            </div>
            <button
                type="button"
                class="eula-btn eula-btn--ghost auth-login-submit auth-device-open-browser"
                prop:hidden=move || verification_url.get().is_none()
                on:click=move |_| {
                    if let Some(u) = verification_url.get_untracked() {
                        open_in_new_tab(u.as_str());
                    }
                }
            >
                {move || i18n.tr(I18nKey::AuthOpenVerify)()}
            </button>
            <Show when=move || polling.get()>
                <p class="auth-login-polling">{move || i18n.tr(I18nKey::AuthPolling)()}</p>
            </Show>
        </Show>
        <Show when=move || err.get().is_some()>
            <p class="auth-login-error" role="alert">{move || err.get().unwrap_or_default()}</p>
        </Show>
    }
}

#[component]
pub fn LoginModal() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let tab = RwSignal::new(0_u8);

    let tab_cls = move |idx: u8| {
        move || {
            let mut c = String::from("eula-btn eula-btn--ghost auth-login-tab");
            if tab.get() == idx {
                c.push_str(" eula-btn--primary");
            }
            c
        }
    };

    view! {
        <div class="eula-root">
            <div class="eula-scrim" aria-hidden="true"></div>
            <div
                class="eula-sheet auth-login-sheet"
                role="dialog"
                aria-modal="true"
                aria-labelledby="auth-login-heading"
            >
                <h1 id="auth-login-heading" class="auth-login-title">
                    {move || i18n.tr(I18nKey::AuthLoginHeading)()}
                </h1>

                <div class="auth-login-tabs" role="tablist">
                    <button
                        type="button"
                        class=tab_cls(0)
                        aria-selected=move || (tab.get() == 0).to_string()
                        on:click=move |_| tab.set(0)
                    >
                        {move || i18n.tr(I18nKey::AuthTabEmail)()}
                    </button>
                    <button
                        type="button"
                        class=tab_cls(1)
                        aria-selected=move || (tab.get() == 1).to_string()
                        on:click=move |_| tab.set(1)
                    >
                        {move || i18n.tr(I18nKey::AuthTabDevice)()}
                    </button>
                </div>

                <div class="auth-login-body">
                    <Show when=move || tab.get() == 0 fallback=AuthDevicePane>
                        <AuthEmailPane/>
                    </Show>
                </div>
            </div>
        </div>
    }
}
