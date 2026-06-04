//! A single saved SSH connection rendered as a clickable card in the list
//! view. Shows only non-secret metadata plus a "stored / not set" badge for
//! the relevant secret. The endpoint (`user@host:port`) and the remote
//! directory are **masked by default** — an eye toggle reveals them — since a
//! settings screen may be shared or screen-captured. Clicking the card (but
//! not the eye button) opens the editor.

use super::{auth_label_key, resume_label_key};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{RemoteAuthKind, RemoteConnectionView};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

/// Fixed-width placeholder shown instead of the real value while masked, so
/// neither the value nor its length leaks.
const MASK: &str = "••••••••••••";

/// Dashed "Add connection" placeholder tile that fills otherwise-empty grid
/// cells. Clicking it opens a fresh connection editor.
#[component]
pub fn RemoteAddCard(on_add: Callback<()>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <li class="remote-conn-card-item">
            <button
                type="button"
                class="remote-conn-card remote-conn-card--add"
                aria-label=move || i18n.tr(I18nKey::RemoteAddConnection)()
                on:click=move |_| on_add.run(())
            >
                <span class="remote-conn-card__add-inner">
                    <LxIcon icon=icondata::LuPlus width="0.9rem" height="0.9rem" />
                    <span>{move || i18n.tr(I18nKey::RemoteAddConnection)()}</span>
                </span>
            </button>
        </li>
    }
}

#[component]
pub fn RemoteConnectionCard(view: RemoteConnectionView, on_edit: Callback<()>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let conn = view.connection.clone();

    let title = if conn.label.trim().is_empty() {
        conn.host.clone()
    } else {
        conn.label.clone()
    };
    // `user@host:port`, skipping any empty parts gracefully.
    let endpoint = {
        let host_port = if conn.host.is_empty() {
            String::new()
        } else {
            format!("{}:{}", conn.host, conn.port)
        };
        if conn.username.is_empty() {
            host_port
        } else if host_port.is_empty() {
            conn.username.clone()
        } else {
            format!("{}@{}", conn.username, host_port)
        }
    };

    let auth_key = auth_label_key(conn.auth_kind);
    let resume_key = resume_label_key(conn.resume);
    let remote_dir = conn.default_remote_dir.clone();

    // Secret indicator: passwords use `has_password`, key auth uses
    // `has_passphrase`. SSH-agent auth carries no stored secret.
    let secret_stored = match conn.auth_kind {
        RemoteAuthKind::Password => Some(view.has_password),
        RemoteAuthKind::Key => Some(view.has_passphrase),
        RemoteAuthKind::Agent => None,
    };

    // Reveal is per-card and defaults to masked.
    let revealed = RwSignal::new(false);

    let endpoint_has = !endpoint.is_empty();
    let endpoint_shown = {
        let endpoint = endpoint.clone();
        move || {
            if revealed.get() {
                endpoint.clone()
            } else {
                MASK.to_string()
            }
        }
    };
    let dir_value = remote_dir.clone().unwrap_or_default();
    let dir_has = remote_dir.is_some();
    let dir_shown = move || {
        if revealed.get() {
            dir_value.clone()
        } else {
            MASK.to_string()
        }
    };

    let open = move || on_edit.run(());

    view! {
        <li class="remote-conn-card-item">
            <div
                class="settings-field-card remote-conn-card"
                role="button"
                tabindex="0"
                title=move || i18n.tr(I18nKey::RemoteCardEditHint)()
                aria-label=move || i18n.tr(I18nKey::RemoteEditConnection)()
                on:click=move |_| open()
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    let key = ev.key();
                    if key == "Enter" || key == " " {
                        ev.prevent_default();
                        open();
                    }
                }
            >
                <div class="remote-conn-card__head">
                    <span class="remote-conn-card__title">
                        <span class="remote-conn-card__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuServer width="0.85rem" height="0.85rem" />
                        </span>
                        <span class="remote-conn-card__name">{title}</span>
                    </span>
                    <span class="remote-conn-card__head-actions">
                        <button
                            type="button"
                            class="remote-conn-card__reveal"
                            aria-pressed=move || revealed.get().to_string()
                            title=move || if revealed.get() {
                                i18n.tr(I18nKey::RemoteHideDetails)()
                            } else {
                                i18n.tr(I18nKey::RemoteRevealDetails)()
                            }
                            aria-label=move || if revealed.get() {
                                i18n.tr(I18nKey::RemoteHideDetails)()
                            } else {
                                i18n.tr(I18nKey::RemoteRevealDetails)()
                            }
                            on:click=move |ev: web_sys::MouseEvent| {
                                ev.stop_propagation();
                                revealed.update(|r| *r = !*r);
                            }
                        >
                            {move || if revealed.get() {
                                view! { <LxIcon icon=icondata::LuEyeOff width="0.9rem" height="0.9rem" /> }
                            } else {
                                view! { <LxIcon icon=icondata::LuEye width="0.9rem" height="0.9rem" /> }
                            }}
                        </button>
                        <span class="remote-conn-card__chevron" aria-hidden="true">
                            <LxIcon icon=icondata::LuChevronRight width="0.9rem" height="0.9rem" />
                        </span>
                    </span>
                </div>

                <Show when=move || endpoint_has>
                    <code
                        class="remote-conn-card__endpoint"
                        class:remote-conn-card__masked=move || !revealed.get()
                    >
                        {endpoint_shown.clone()}
                    </code>
                </Show>

                <div class="remote-conn-card__meta">
                    <span class="remote-conn-card__badge">{move || i18n.tr(auth_key)()}</span>
                    <span class="remote-conn-card__badge">{move || i18n.tr(resume_key)()}</span>
                    {move || {
                        secret_stored
                            .map(|stored| {
                                let label = if stored {
                                    i18n.tr(I18nKey::RemoteSecretStored)()
                                } else {
                                    i18n.tr(I18nKey::RemoteSecretNotSet)()
                                };
                                view! {
                                    <span
                                        class="remote-conn-card__badge"
                                        class:remote-conn-card__badge--muted=move || !stored
                                    >
                                        {label}
                                    </span>
                                }
                            })
                    }}
                </div>

                <Show when=move || dir_has>
                    <code
                        class="remote-conn-card__dir"
                        class:remote-conn-card__masked=move || !revealed.get()
                    >
                        {dir_shown.clone()}
                    </code>
                </Show>
            </div>
        </li>
    }
}
