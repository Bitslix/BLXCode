use super::update_service::{UpdateService, UpdateUiStatus};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn UpdateBanner() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let updates = expect_context::<UpdateService>();

    view! {
        <Show when=move || updates.banner_visible().get()>
            <div class="blx-update-banner" role="status">
                <span class="blx-update-banner__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuArrowDownToLine width="0.95rem" height="0.95rem" />
                </span>
                <span class="blx-update-banner__text">
                    {move || {
                        let version = updates.available_version().get().unwrap_or_default();
                        format!("{} {version}", i18n.tr(I18nKey::UpdateBannerTitle)())
                    }}
                </span>
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    on:click=move |_| updates.open_dialog()
                >
                    {move || i18n.tr(I18nKey::UpdateBannerAction)()}
                </button>
            </div>
        </Show>
    }
}

#[component]
pub fn UpdateDialog() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let updates = expect_context::<UpdateService>();

    view! {
        <Show when=move || updates.dialog_open().get()>
            <div class="harness-overlay harness-overlay--modal" role="presentation">
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=move |_| updates.close_dialog()
                ></button>
                <section
                    class="harness-sheet harness-sheet--update"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape"
                            && matches!(
                                updates.status().get_untracked(),
                                UpdateUiStatus::Available | UpdateUiStatus::Error
                            )
                        {
                            ev.prevent_default();
                            updates.close_dialog();
                        }
                    }
                >
                    <header class="blx-update-dialog__head">
                        <h2 class="harness-settings-title">
                            <span class="harness-settings-title__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuPackage width="1.05rem" height="1.05rem" />
                            </span>
                            <span>{move || i18n.tr(I18nKey::UpdateDialogTitle)()}</span>
                        </h2>
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| updates.close_dialog()
                        >
                            <span class="harness-btn-inline">
                                <LxIcon icon=icondata::LuX width="0.82rem" height="0.82rem" />
                                <span>{move || i18n.tr(I18nKey::BtnClose)()}</span>
                            </span>
                        </button>
                    </header>

                    <div class="blx-update-version">
                        <span>{move || updates.current_version().get()}</span>
                        <LxIcon icon=icondata::LuArrowRight width="0.9rem" height="0.9rem" />
                        <strong>{move || updates.available_version().get().unwrap_or_default()}</strong>
                    </div>

                    <Show when=move || updates.notes().get().is_some()>
                        <section class="blx-update-notes">
                            <h3>{move || i18n.tr(I18nKey::UpdateDialogNotes)()}</h3>
                            <p>{move || updates.notes().get().unwrap_or_default()}</p>
                        </section>
                    </Show>

                    <UpdateProgressBlock updates=updates />

                    <Show when=move || updates.message().get().is_some()>
                        <p class="blx-update-error">{move || updates.message().get().unwrap_or_default()}</p>
                    </Show>

                    <footer class="blx-update-actions">
                        <UpdatePrimaryButton updates=updates />
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| updates.close_dialog()
                            prop:disabled=move || matches!(
                                updates.status().get(),
                                UpdateUiStatus::Downloading | UpdateUiStatus::Installing
                            )
                        >
                            {move || i18n.tr(I18nKey::UpdateDialogLater)()}
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}

#[component]
fn UpdateProgressBlock(updates: UpdateService) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <Show when=move || matches!(
            updates.status().get(),
            UpdateUiStatus::Downloading | UpdateUiStatus::Installing | UpdateUiStatus::Done
        )>
            <div class="blx-update-progress">
                <div class="blx-update-progress__row">
                    <span>{move || match updates.status().get() {
                        UpdateUiStatus::Installing => i18n.tr(I18nKey::UpdateDialogInstalling)(),
                        UpdateUiStatus::Done => i18n.tr(I18nKey::UpdateDialogDone)(),
                        _ => i18n.tr(I18nKey::UpdateDialogDownloading)(),
                    }}</span>
                    <span>{move || updates.speed_label().get().unwrap_or_default()}</span>
                </div>
                <div class="blx-update-progress__track">
                    <div
                        class="blx-update-progress__bar"
                        style:width=move || {
                            let pct = updates.progress_pct().get().unwrap_or_else(|| {
                                if updates.status().get() == UpdateUiStatus::Done { 100.0 } else { 35.0 }
                            });
                            format!("{pct:.0}%")
                        }
                    ></div>
                </div>
            </div>
        </Show>
    }
}

#[component]
fn UpdatePrimaryButton(updates: UpdateService) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="workbench-mini-btn workbench-mini-btn--primary"
            on:click=move |_| match updates.status().get_untracked() {
                UpdateUiStatus::Done => updates.relaunch(),
                UpdateUiStatus::Error => updates.check_manual(),
                _ => updates.start_install(),
            }
            prop:disabled=move || matches!(
                updates.status().get(),
                UpdateUiStatus::Downloading | UpdateUiStatus::Installing
            )
        >
            <span class="harness-btn-inline">
                <LxIcon
                    icon=move || match updates.status().get() {
                        UpdateUiStatus::Done => icondata::LuRefreshCw,
                        UpdateUiStatus::Error => icondata::LuCircleAlert,
                        _ => icondata::LuDownload,
                    }
                    width="0.82rem"
                    height="0.82rem"
                />
                <span>{move || match updates.status().get() {
                    UpdateUiStatus::Done => i18n.tr(I18nKey::UpdateDialogRelaunch)(),
                    UpdateUiStatus::Error => i18n.tr(I18nKey::UpdateDialogRetry)(),
                    _ => i18n.tr(I18nKey::UpdateDialogInstall)(),
                }}</span>
            </span>
        </button>
    }
}
