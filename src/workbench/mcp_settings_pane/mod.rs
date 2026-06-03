//! MCP settings pane (Settings → MCP).
//!
//! Lists the registered MCP servers with add/edit/remove and a per-server
//! connection test. Edits hit the backend registry immediately; a banner
//! reminds the user that the in-app agent only picks up changes after a session
//! reset (the reset button calls `agent_clear_conversation`).

use std::collections::BTreeMap;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_clear_conversation, mcp_list, mcp_remove, mcp_test, mcp_upsert, McpServer, McpTransport,
};
use crate::workbench::SettingsPaneHeader;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

mod editor;
use editor::McpServerEditor;

/// A blank stdio server used when the user clicks "Add server".
fn blank_server() -> McpServer {
    McpServer {
        id: String::new(),
        name: String::new(),
        enabled: true,
        description: String::new(),
        transport: McpTransport::Stdio {
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
        },
    }
}

#[component]
pub fn McpSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    let servers = RwSignal::new(Vec::<McpServer>::new());
    // `Some(server)` while the add/edit dialog is open.
    let editing = RwSignal::new(None::<McpServer>);
    let status = RwSignal::new(String::new());
    // True after any registry mutation (add/edit/remove/toggle) to remind the
    // user a session reset / app reload is needed for it to take effect.
    let needs_reload = RwSignal::new(false);

    let reload = move || {
        leptos::task::spawn_local(async move {
            match mcp_list().await {
                Ok(list) => servers.set(list),
                Err(e) => status.set(e),
            }
        });
    };
    // Initial load.
    Effect::new(move |_| reload());

    // A registry mutation happened: refresh the list and raise the reload hint.
    let changed = Callback::new(move |_: ()| {
        needs_reload.set(true);
        reload();
    });

    let on_save = move |server: McpServer| {
        leptos::task::spawn_local(async move {
            match mcp_upsert(server).await {
                Ok(_) => {
                    editing.set(None);
                    needs_reload.set(true);
                    match mcp_list().await {
                        Ok(list) => servers.set(list),
                        Err(e) => status.set(e),
                    }
                }
                Err(e) => status.set(e),
            }
        });
    };

    let reset_session = move |_| {
        leptos::task::spawn_local(async move {
            match agent_clear_conversation().await {
                Ok(()) => {
                    status.set(String::new());
                    needs_reload.set(false);
                }
                Err(e) => status.set(e),
            }
        });
    };

    view! {
        <article class="harness-pane mcp-settings-pane">
            <SettingsPaneHeader
                icon=icondata::LuPlug
                title=I18nKey::HsCatMcp
                description=I18nKey::McpDescription
            />

            // Mandatory session-reset reminder.
            <div class="mcp-reset-banner" role="note">
                <span class="mcp-reset-banner__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuInfo width="0.9rem" height="0.9rem" />
                </span>
                <span class="mcp-reset-banner__text">
                    {move || i18n.tr(I18nKey::McpResetHint)()}
                </span>
                <button class="blx-btn blx-btn--ghost mcp-reset-banner__btn" on:click=reset_session>
                    {move || i18n.tr(I18nKey::McpResetButton)()}
                </button>
            </div>

            <section class="harness-subpane">
                <div class="mcp-list-head">
                    <h4 class="harness-pane-subhead">
                        <span class="harness-pane-subhead__text">
                            {move || i18n.tr(I18nKey::McpHeading)()}
                        </span>
                    </h4>
                    <button
                        class="blx-btn blx-btn--primary"
                        on:click=move |_| editing.set(Some(blank_server()))
                    >
                        <LxIcon icon=icondata::LuPlus width="0.85rem" height="0.85rem" />
                        <span>{move || i18n.tr(I18nKey::McpAdd)()}</span>
                    </button>
                </div>

                <Show
                    when=move || !servers.get().is_empty()
                    fallback=move || view! {
                        <p class="mcp-empty">{move || i18n.tr(I18nKey::McpEmpty)()}</p>
                    }
                >
                    <ul class="mcp-server-list">
                        <For
                            each=move || servers.get()
                            key=|s| s.id.clone()
                            children=move |server: McpServer| {
                                view! {
                                    <McpServerRow
                                        server=server
                                        on_edit=Callback::new(move |s| editing.set(Some(s)))
                                        on_changed=changed
                                    />
                                }
                            }
                        />
                    </ul>
                </Show>

                <Show when=move || needs_reload.get()>
                    <p class="mcp-reload-hint" role="status">
                        {move || i18n.tr(I18nKey::McpReloadHint)()}
                    </p>
                </Show>

                <Show when=move || !status.get().is_empty()>
                    <p class="mcp-status-error">{move || status.get()}</p>
                </Show>
            </section>

            <Show when=move || editing.get().is_some()>
                <McpServerEditor
                    server=editing.get().unwrap_or_else(blank_server)
                    on_save=Callback::new(on_save)
                    on_cancel=Callback::new(move |_| editing.set(None))
                />
            </Show>
        </article>
    }
}

#[component]
fn McpServerRow(
    server: McpServer,
    on_edit: Callback<McpServer>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let test_label = RwSignal::new(None::<String>);

    let id_for_remove = server.id.clone();
    let server_for_test = server.clone();
    let server_for_edit = server.clone();
    let server_for_toggle = server.clone();

    let transport_label = server.transport.label().to_string();
    let name = server.name.clone();
    let enabled = server.enabled;

    let on_test = move |_| {
        let id = server_for_test.id.clone();
        let ok_txt = i18n.tr(I18nKey::McpTestOk)().to_string();
        let fail_txt = i18n.tr(I18nKey::McpTestFailed)().to_string();
        let tools_txt = i18n.tr(I18nKey::McpToolsSuffix)().to_string();
        let testing = i18n.tr(I18nKey::McpTesting)().to_string();
        test_label.set(Some(testing));
        leptos::task::spawn_local(async move {
            match mcp_test(id).await {
                Ok(res) if res.ok => {
                    test_label.set(Some(format!("{ok_txt} · {} {tools_txt}", res.tool_count)));
                }
                Ok(res) => {
                    test_label.set(Some(res.error.unwrap_or(fail_txt)));
                }
                Err(e) => test_label.set(Some(e)),
            }
        });
    };

    let on_remove = move |_| {
        let id = id_for_remove.clone();
        leptos::task::spawn_local(async move {
            let _ = mcp_remove(id).await;
            on_changed.run(());
        });
    };

    let on_toggle = move |_| {
        let mut s = server_for_toggle.clone();
        s.enabled = !s.enabled;
        leptos::task::spawn_local(async move {
            let _ = mcp_upsert(s).await;
            on_changed.run(());
        });
    };

    view! {
        <li class="mcp-server-row" class:mcp-server-row--off=move || !enabled>
            <div class="mcp-server-row__main">
                <span class="mcp-server-row__name">{name}</span>
                <span class="mcp-server-row__badge">{transport_label}</span>
                {move || test_label.get().map(|t| view! {
                    <span class="mcp-server-row__test">{t}</span>
                })}
            </div>
            <div class="mcp-server-row__actions">
                <button class="blx-btn blx-btn--ghost" on:click=on_toggle title=move || i18n.tr(I18nKey::McpEnabled)()>
                    <span
                        class="blx-switch"
                        class:blx-switch--on=move || enabled
                        aria-hidden="true"
                    >
                        <span class="blx-switch__thumb" />
                    </span>
                </button>
                <button class="blx-btn blx-btn--ghost" on:click=on_test>
                    {move || i18n.tr(I18nKey::McpTest)()}
                </button>
                <button
                    class="blx-btn blx-btn--ghost"
                    on:click=move |_| on_edit.run(server_for_edit.clone())
                >
                    {move || i18n.tr(I18nKey::McpEdit)()}
                </button>
                <button class="blx-btn blx-btn--ghost mcp-danger" on:click=on_remove>
                    {move || i18n.tr(I18nKey::McpRemove)()}
                </button>
            </div>
        </li>
    }
}

/// Shared helper used by the editor: split textarea lines, dropping blanks.
pub(crate) fn lines_to_vec(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Parse `KEY=VALUE` / `KEY: VALUE` lines into a map.
pub(crate) fn lines_to_map(raw: &str, sep: char) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(sep) {
            let k = k.trim();
            if !k.is_empty() {
                map.insert(k.to_string(), v.trim().to_string());
            }
        }
    }
    map
}
