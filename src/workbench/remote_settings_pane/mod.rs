//! Settings → Remote: manage SSH connection presets used by remote
//! workspaces. Non-secret metadata is persisted by the backend
//! (`remote_connections.json`); passwords / key passphrases go to the OS
//! keychain via `ssh_secrets`. Secret inputs here are write-only — the pane
//! only ever learns whether a secret is *stored*, never its value.
//!
//! The pane has two modes (master/detail), mirroring the other settings tabs:
//! a **list** of saved connections rendered as cards (no secrets, only a
//! "stored" badge), and an **editor** opened by clicking a card or the
//! top-right "Add connection" button.

mod connection_card;
mod connection_editor;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, ssh_remotes_list, RemoteAuthKind, RemoteConnection, RemoteConnectionView,
    RemoteResume,
};
use crate::workbench::SettingsPaneHeader;
use connection_card::{RemoteAddCard, RemoteConnectionCard};
use connection_editor::RemoteConnectionEditor;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

// --- Shared helpers (used by the editor + card submodules) ---

pub(super) fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

pub(super) fn select_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|s| s.value())
        .unwrap_or_default()
}

pub(super) fn auth_to_str(kind: RemoteAuthKind) -> &'static str {
    match kind {
        RemoteAuthKind::Password => "password",
        RemoteAuthKind::Key => "key",
        RemoteAuthKind::Agent => "agent",
    }
}

pub(super) fn auth_from_str(s: &str) -> RemoteAuthKind {
    match s {
        "key" => RemoteAuthKind::Key,
        "agent" => RemoteAuthKind::Agent,
        _ => RemoteAuthKind::Password,
    }
}

pub(super) fn auth_label_key(kind: RemoteAuthKind) -> I18nKey {
    match kind {
        RemoteAuthKind::Password => I18nKey::RemoteAuthPassword,
        RemoteAuthKind::Key => I18nKey::RemoteAuthKey,
        RemoteAuthKind::Agent => I18nKey::RemoteAuthAgent,
    }
}

pub(super) fn resume_to_str(r: RemoteResume) -> &'static str {
    match r {
        RemoteResume::Tmux => "tmux",
        RemoteResume::KeepaliveOnly => "keepalive",
    }
}

pub(super) fn resume_from_str(s: &str) -> RemoteResume {
    match s {
        "tmux" => RemoteResume::Tmux,
        _ => RemoteResume::KeepaliveOnly,
    }
}

pub(super) fn resume_label_key(r: RemoteResume) -> I18nKey {
    match r {
        RemoteResume::Tmux => I18nKey::RemoteResumeTmux,
        RemoteResume::KeepaliveOnly => I18nKey::RemoteResumeKeepalive,
    }
}

pub(super) fn blank_view() -> RemoteConnectionView {
    RemoteConnectionView {
        connection: RemoteConnection {
            id: String::new(),
            label: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            auth_kind: RemoteAuthKind::Password,
            key_path: None,
            resume: RemoteResume::KeepaliveOnly,
            default_remote_dir: None,
        },
        has_password: false,
        has_passphrase: false,
    }
}

/// Outcome reported by the editor when it closes, so the pane knows whether
/// to refresh the list and what feedback to surface.
#[derive(Clone, Copy)]
pub(super) enum EditorOutcome {
    Cancelled,
    Saved,
    Deleted,
}

#[derive(Clone)]
struct RowModel {
    uid: u64,
    view: RemoteConnectionView,
}

#[component]
pub fn RemoteSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let rows: RwSignal<Vec<RowModel>> = RwSignal::new(Vec::new());
    let next_uid = StoredValue::new(1u64);
    let load_error: RwSignal<Option<String>> = RwSignal::new(None);
    let status_msg: RwSignal<Option<String>> = RwSignal::new(None);
    // `None` => list view; `Some(view)` => editing that connection (blank = new).
    let editing: RwSignal<Option<RemoteConnectionView>> = RwSignal::new(None);

    let alloc_uid = move || {
        let id = next_uid.get_value();
        next_uid.set_value(id + 1);
        id
    };

    let load = move || {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match ssh_remotes_list().await {
                Ok(list) => {
                    let models = list
                        .into_iter()
                        .map(|view| RowModel {
                            uid: alloc_uid(),
                            view,
                        })
                        .collect::<Vec<_>>();
                    rows.set(models);
                    load_error.set(None);
                }
                Err(err) => load_error.set(Some(err)),
            }
        });
    };

    Effect::new(move |_| {
        load();
    });

    let start_new = move || {
        status_msg.set(None);
        editing.set(Some(blank_view()));
    };
    let open_new = move |_| start_new();
    let on_add = Callback::new(move |_: ()| start_new());

    // Pad the last grid row with dashed "Add" placeholder tiles (instead of
    // empty cells). Assumes the ~3-column layout from the CSS; on narrower
    // widths the extra tiles simply wrap and stay valid "Add" affordances.
    let add_placeholder_count = move || {
        let n = rows.with(|v| v.len());
        if n == 0 {
            0
        } else {
            const GRID_COLS: usize = 3;
            let rem = n % GRID_COLS;
            if rem == 0 {
                1
            } else {
                GRID_COLS - rem
            }
        }
    };

    let on_editor_close = Callback::new(move |outcome: EditorOutcome| {
        editing.set(None);
        match outcome {
            EditorOutcome::Cancelled => {}
            EditorOutcome::Saved => {
                status_msg.set(Some(i18n.tr(I18nKey::RemoteSaved)().to_string()));
                load();
            }
            EditorOutcome::Deleted => {
                status_msg.set(None);
                load();
            }
        }
    });

    let is_editing = move || editing.with(|e| e.is_some());
    let show_list = move || is_tauri_shell() && !is_editing();

    view! {
        <article class="harness-pane remote-pane">
            <div class="remote-pane__topbar">
                <SettingsPaneHeader
                    icon=icondata::LuServer
                    title=I18nKey::RemoteHeading
                    description=I18nKey::RemoteDescription
                />
                <Show when=show_list>
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary remote-pane__add"
                        disabled=move || !is_tauri_shell()
                        on:click=open_new
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuPlus width="0.78rem" height="0.78rem" />
                            <span>{move || i18n.tr(I18nKey::RemoteAddConnection)()}</span>
                        </span>
                    </button>
                </Show>
            </div>

            <Show when=move || !is_tauri_shell()>
                <p class="harness-error">{move || i18n.tr(I18nKey::RemoteRequiresTauri)()}</p>
            </Show>

            // --- Editor (detail) view ---
            <Show when=is_editing>
                {move || {
                    editing
                        .get()
                        .map(|view| {
                            view! {
                                <RemoteConnectionEditor initial=view on_close=on_editor_close />
                            }
                        })
                }}
            </Show>

            // --- List view ---
            <Show when=show_list>
                <section class="harness-subpane remote-pane__list-card">
                    <Show when=move || load_error.with(|m| m.is_some())>
                        <p class="harness-error">{move || load_error.get().unwrap_or_default()}</p>
                    </Show>
                    <Show when=move || status_msg.with(|m| m.is_some())>
                        <p class="harness-status">{move || status_msg.get().unwrap_or_default()}</p>
                    </Show>
                    <Show when=move || rows.with(|v| v.is_empty()) && load_error.with(|m| m.is_none())>
                        <p class="harness-muted">{move || i18n.tr(I18nKey::RemoteEmpty)()}</p>
                    </Show>

                    <Show when=move || rows.with(|v| !v.is_empty())>
                        <ul class="remote-conn-list">
                            <For
                                each=move || rows.get()
                                key=|r| (r.uid, r.view.connection.id.clone())
                                children=move |row: RowModel| {
                                    let view_for_edit = row.view.clone();
                                    let on_edit = Callback::new(move |_: ()| {
                                        status_msg.set(None);
                                        editing.set(Some(view_for_edit.clone()));
                                    });
                                    view! { <RemoteConnectionCard view=row.view on_edit=on_edit /> }
                                }
                            />
                            {move || {
                                (0..add_placeholder_count())
                                    .map(|_| view! { <RemoteAddCard on_add=on_add /> })
                                    .collect_view()
                            }}
                        </ul>
                    </Show>
                </section>
            </Show>
        </article>
    }
}
