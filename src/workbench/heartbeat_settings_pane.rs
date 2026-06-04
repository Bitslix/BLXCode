//! HeartBeat settings pane.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    heartbeat_service_run_now, heartbeat_service_set_enabled, heartbeat_services_list,
    heartbeat_settings_get, heartbeat_settings_save, is_tauri_shell, HeartbeatServiceStatus,
    HeartbeatServiceView, HeartbeatSettings,
};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::BTreeMap;
use wasm_bindgen::JsCast;

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|input| input.checked())
}

fn number_value(ev: &web_sys::Event) -> Option<u32> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .and_then(|input| input.value().parse::<u32>().ok())
}

#[component]
pub fn HeartbeatSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let enabled = RwSignal::new(false);
    let interval = RwSignal::new(60_u32);
    let service_enabled = RwSignal::new(BTreeMap::<String, bool>::new());
    let services = RwSignal::new(Vec::<HeartbeatServiceView>::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let status = RwSignal::new(None::<String>);

    let reload = move || {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match heartbeat_settings_get().await {
                Ok(settings) => {
                    enabled.set(settings.enabled);
                    interval.set(settings.interval_minutes);
                    service_enabled.set(settings.service_enabled);
                    error.set(None);
                }
                Err(err) => error.set(Some(err)),
            }
            match heartbeat_services_list().await {
                Ok(list) => services.set(list),
                Err(err) => error.set(Some(err)),
            }
        });
    };

    Effect::new(move |_| reload());

    let save_settings = move || {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        status.set(None);
        let settings = HeartbeatSettings {
            enabled: enabled.get_untracked(),
            interval_minutes: interval.get_untracked(),
            service_enabled: service_enabled.get_untracked(),
        };
        leptos::task::spawn_local(async move {
            match heartbeat_settings_save(settings).await {
                Ok(saved) => {
                    enabled.set(saved.enabled);
                    interval.set(saved.interval_minutes);
                    service_enabled.set(saved.service_enabled);
                    status.set(Some("Saved.".into()));
                }
                Err(err) => error.set(Some(err)),
            }
            match heartbeat_services_list().await {
                Ok(list) => services.set(list),
                Err(err) => error.set(Some(err)),
            }
            busy.set(false);
        });
    };

    view! {
        <article class="harness-pane heartbeat-settings-pane">
            <header class="harness-pane-header">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuHeartPulse width="1.02rem" height="1.02rem" />
                </span>
                <div class="harness-pane-header__copy">
                    <h3 class="harness-pane-title">
                        <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::HarnessHeartBeat)()}</span>
                    </h3>
                    <p class="harness-pane-description">
                        {move || i18n.tr(I18nKey::HeartbeatIntervalDescription)()}
                    </p>
                </div>
            </header>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuTimer width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">"Schedule"</span>
                </h4>
                <label class="app-prefs-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || enabled.get()
                        on:change=move |ev| {
                            if let Some(checked) = checkbox_checked(&ev) {
                                enabled.set(checked);
                            }
                        }
                    />
                    <span class="blx-switch" class:blx-switch--on=move || enabled.get() aria-hidden="true">
                        <span class="blx-switch__thumb" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::HeartbeatEnableHeartBeat)()}</span>
                </label>
                <label class="harness-stack" style="max-width: 18rem;">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">"Interval minutes"</span>
                    </span>
                    <input
                        class="harness-input"
                        type="number"
                        min="10"
                        max="1440"
                        prop:value=move || interval.get().to_string()
                        on:input=move |ev| {
                            if let Some(value) = number_value(&ev) {
                                interval.set(value.clamp(10, 1440));
                            }
                        }
                    />
                </label>
                <div class="agent-provider-pane__actions">
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| save_settings()
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                            <span>{move || {
                                if busy.get() {
                                    i18n.tr(I18nKey::CommonSaving)().to_string()
                                } else {
                                    i18n.tr(I18nKey::BtnSave)().to_string()
                                }
                            }}</span>
                        </span>
                    </button>
                </div>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuListChecks width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">"Registered services"</span>
                </h4>
                <div class="heartbeat-service-list">
                    <For
                        each=move || services.get()
                        key=|service| service.id.clone()
                        children=move |service| {
                            let id_for_toggle = service.id.clone();
                            let id_for_run = service.id.clone();
                            view! {
                                <div class="heartbeat-service-row">
                                    <div class="heartbeat-service-row__main">
                                        <strong>{service.name.clone()}</strong>
                                        <span>{service.description.clone()}</span>
                                        <small>{format!("{} / {} / {}", service.kind, service.source, service.capabilities.join(", "))}</small>
                                    </div>
                                    <div class="heartbeat-service-row__meta">
                                        <span>{move || i18n.tr(status_label_key(service.status))()}</span>
                                        <span>{format_time(service.last_call)}</span>
                                        <span>{format_time(service.next_call)}</span>
                                        <span>{format!("skips: {}", service.skip_count)}</span>
                                        <span>{service.last_response.clone().unwrap_or_else(|| {
                                            i18n.tr(I18nKey::HeartbeatNoResponseYet)().to_string()
                                        })}</span>
                                    </div>
                                    <div class="heartbeat-service-row__actions">
                                        <label class="app-prefs-toggle">
                                            <input
                                                type="checkbox"
                                                prop:checked=service.enabled
                                                on:change=move |ev| {
                                                    let checked = checkbox_checked(&ev).unwrap_or(false);
                                                    let id = id_for_toggle.clone();
                                                    leptos::task::spawn_local(async move {
                                                        match heartbeat_service_set_enabled(id, checked).await {
                                                            Ok(list) => services.set(list),
                                                            Err(err) => error.set(Some(err)),
                                                        }
                                                    });
                                                }
                                            />
                                            <span class="blx-switch" class:blx-switch--on=service.enabled aria-hidden="true">
                                                <span class="blx-switch__thumb" />
                                            </span>
                                        </label>
                                        <button
                                            type="button"
                                            class="workbench-mini-btn"
                                            disabled=move || !is_tauri_shell()
                                            on:click=move |_| {
                                                let id = id_for_run.clone();
                                                leptos::task::spawn_local(async move {
                                                    match heartbeat_service_run_now(id).await {
                                                        Ok(list) => services.set(list),
                                                        Err(err) => error.set(Some(err)),
                                                    }
                                                });
                                            }
                                        >
                                            <span class="harness-btn-inline">
                                                <LxIcon icon=icondata::LuPlay width="0.78rem" height="0.78rem" />
                                                <span>{move || i18n.tr(I18nKey::CommonRun)()}</span>
                                            </span>
                                        </button>
                                    </div>
                                </div>
                            }
                        }
                    />
                </div>
            </section>

            <Show when=move || status.get().is_some()>
                <p class="app-prefs-hint">{move || status.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || error.get().is_some()>
                <p class="mcp-status-error">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </article>
    }
}

fn status_label_key(status: HeartbeatServiceStatus) -> I18nKey {
    match status {
        HeartbeatServiceStatus::Idle => I18nKey::CommonIdle,
        HeartbeatServiceStatus::Running => I18nKey::SwarmStatusRunning,
        HeartbeatServiceStatus::Stalled => I18nKey::HeartbeatStalled,
        HeartbeatServiceStatus::Error => I18nKey::CommonError,
        HeartbeatServiceStatus::Disabled => I18nKey::AgWebProviderNone,
    }
}

fn format_time(value: Option<u64>) -> String {
    match value {
        Some(v) => format!("{v}"),
        None => "-".into(),
    }
}
