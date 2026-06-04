use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_onboarding_complete, agent_session_roles_list, agent_validate_nickname,
    AgentProviderSettingsView, SessionRoleView,
};
use crate::workbench::SessionRolePicker;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

fn nickname_err_key(code: &str) -> I18nKey {
    match code.strip_prefix("nickname:").unwrap_or(code) {
        "tooLong" => I18nKey::AgNicknameErrTooLong,
        "invalidChars" => I18nKey::AgNicknameErrInvalidChars,
        _ => I18nKey::AgNicknameErrBadWord,
    }
}

#[component]
pub fn AgentOnboardingDialog(
    open: RwSignal<bool>,
    settings: RwSignal<Option<AgentProviderSettingsView>>,
    on_complete: Callback<AgentProviderSettingsView>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let initialized = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let selected_role: RwSignal<Option<String>> = RwSignal::new(None);
    let roles: RwSignal<Vec<SessionRoleView>> = RwSignal::new(Vec::new());
    let nickname_error: RwSignal<Option<I18nKey>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);

    Effect::new(move |_| {
        if !open.get() {
            initialized.set(false);
            return;
        }
        if initialized.get_untracked() {
            return;
        }
        if let Some(view) = settings.get() {
            name.set(view.agent_nickname);
            selected_role.set(view.default_session_role);
            initialized.set(true);
        }
    });

    Effect::new(move |_| {
        if !open.get() || !roles.get_untracked().is_empty() {
            return;
        }
        spawn_local(async move {
            if let Ok(list) = agent_session_roles_list().await {
                roles.set(list);
            }
        });
    });

    let save_with = move |fallback_defaults: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error_msg.set(None);
        let agent_name = if fallback_defaults {
            String::new()
        } else {
            name.get_untracked()
        };
        let role = if fallback_defaults {
            None
        } else {
            selected_role.get_untracked()
        };
        spawn_local(async move {
            match agent_onboarding_complete(agent_name, role).await {
                Ok(view) => {
                    settings.set(Some(view.clone()));
                    on_complete.run(view);
                    nickname_error.set(None);
                    open.set(false);
                }
                Err(err) => {
                    if err.starts_with("nickname:") {
                        nickname_error.set(Some(nickname_err_key(&err)));
                    } else {
                        error_msg.set(Some(err));
                    }
                }
            }
            busy.set(false);
        });
    };

    view! {
        <Show when=move || open.get()>
            <div class="harness-overlay harness-overlay--modal harness-overlay--centered" role="presentation">
                <section
                    class="harness-sheet harness-sheet--agent-onboarding"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="agent-onboarding-title"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" {
                            ev.prevent_default();
                        }
                    }
                >
                    <header class="agent-onboarding__head">
                        <span class="agent-onboarding__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuBot width="1.05rem" height="1.05rem" />
                        </span>
                        <div class="agent-onboarding__title-wrap">
                            <h2 id="agent-onboarding-title" class="harness-settings-title">
                                {move || i18n.tr(I18nKey::AgentOnboardingNameYourBLXCodeAgent)()}
                            </h2>
                            <p class="harness-muted agent-onboarding__lead">
                                {move || i18n.tr(I18nKey::AgentOnboardingChooseTheNameAndDefaultRoleUsedWhen)()}
                            </p>
                        </div>
                    </header>

                    <div class="agent-onboarding__body">
                        <label class="agent-onboarding__field">
                            <span class="harness-field-label">
                                <span class="harness-field-label__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuUser width="0.82rem" height="0.82rem" />
                                </span>
                                <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgNicknameLabel)()}</span>
                            </span>
                            <input
                                class="workbench-plain-input"
                                class:agent-onboarding__input--error=move || nickname_error.get().is_some()
                                type="text"
                                maxlength="32"
                                placeholder=move || i18n.tr(I18nKey::AgNicknamePlaceholder)()
                                prop:value=move || name.get()
                                on:input=move |ev| {
                                    let val = event_target_value(&ev);
                                    name.set(val.clone());
                                    spawn_local(async move {
                                        let res = agent_validate_nickname(val.clone()).await;
                                        if name.get_untracked() == val {
                                            match res {
                                                Ok(()) => nickname_error.set(None),
                                                Err(code) => nickname_error.set(Some(nickname_err_key(&code))),
                                            }
                                        }
                                    });
                                }
                            />
                            <Show when=move || nickname_error.get().is_some()>
                                <small class="agent-onboarding__error">
                                    {move || nickname_error.get().map(|key| i18n.tr(key)()).unwrap_or_default()}
                                </small>
                            </Show>
                        </label>

                        <label class="agent-onboarding__field">
                            <span class="harness-field-label">
                                <span class="harness-field-label__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuSparkles width="0.82rem" height="0.82rem" />
                                </span>
                                <span class="harness-field-label__text">{move || i18n.tr(I18nKey::WzSessionRoleLabel)()}</span>
                            </span>
                            <SessionRolePicker
                                id="agent-onboarding-role-picker".to_string()
                                roles=Signal::derive(move || roles.get())
                                selected=Signal::derive(move || selected_role.get())
                                on_select=Callback::new(move |role| selected_role.set(role))
                            />
                        </label>
                    </div>

                    <Show when=move || error_msg.get().is_some()>
                        <p class="harness-error-text">{move || error_msg.get().unwrap_or_default()}</p>
                    </Show>

                    <footer class="agent-onboarding__actions">
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            disabled=move || busy.get()
                            on:click=move |_| save_with(true)
                        >
                            {move || i18n.tr(I18nKey::AgentOnboardingUseDefaults)()}
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            disabled=move || busy.get() || nickname_error.get().is_some()
                            on:click=move |_| save_with(false)
                        >
                            <span class="harness-btn-inline">
                                <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                                <span>{move || i18n.tr(I18nKey::BtnSave)()}</span>
                            </span>
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}
