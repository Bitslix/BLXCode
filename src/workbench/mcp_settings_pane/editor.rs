//! Add/edit dialog for a single MCP server.

use std::collections::BTreeMap;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{McpServer, McpTransport};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::{lines_to_map, lines_to_vec};

fn textarea_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|t| t.value())
        .unwrap_or_default()
}

fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

/// Render an env/header map back into editable `KEY<sep>VALUE` lines.
fn map_to_lines(map: &BTreeMap<String, String>, sep: &str) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}{sep}{v}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[component]
pub fn McpServerEditor(
    server: McpServer,
    on_save: Callback<McpServer>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    let id = server.id.clone();
    let name = RwSignal::new(server.name.clone());
    let description = RwSignal::new(server.description.clone());
    let enabled = RwSignal::new(server.enabled);
    // false = stdio, true = http.
    let is_http = RwSignal::new(matches!(server.transport, McpTransport::Http { .. }));

    let (init_cmd, init_args, init_env, init_url, init_headers) = match &server.transport {
        McpTransport::Stdio { command, args, env } => (
            command.clone(),
            args.join("\n"),
            map_to_lines(env, "="),
            String::new(),
            String::new(),
        ),
        McpTransport::Http { url, headers } => (
            String::new(),
            String::new(),
            String::new(),
            url.clone(),
            map_to_lines(headers, ": "),
        ),
    };
    let command = RwSignal::new(init_cmd);
    let args = RwSignal::new(init_args);
    let env = RwSignal::new(init_env);
    let url = RwSignal::new(init_url);
    let headers = RwSignal::new(init_headers);

    let save = move |_| {
        let transport = if is_http.get() {
            McpTransport::Http {
                url: url.get().trim().to_string(),
                headers: lines_to_map(&headers.get(), ':'),
            }
        } else {
            McpTransport::Stdio {
                command: command.get().trim().to_string(),
                args: lines_to_vec(&args.get()),
                env: lines_to_map(&env.get(), '='),
            }
        };
        on_save.run(McpServer {
            id: id.clone(),
            name: name.get().trim().to_string(),
            enabled: enabled.get(),
            description: description.get().trim().to_string(),
            transport,
        });
    };

    view! {
        <div class="mcp-editor-overlay" on:click=move |_| on_cancel.run(())>
            <div class="mcp-editor" on:click=|ev| ev.stop_propagation()>
                <label class="mcp-field">
                    <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpName)()}</span>
                    <input
                        class="blx-input"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(input_value(&ev))
                    />
                </label>

                <label class="mcp-field">
                    <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpDescriptionField)()}</span>
                    <input
                        class="blx-input"
                        prop:value=move || description.get()
                        on:input=move |ev| description.set(input_value(&ev))
                    />
                </label>

                <div class="mcp-field">
                    <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpTransport)()}</span>
                    <div class="mcp-transport-switch">
                        <button
                            class="blx-btn"
                            class:blx-btn--primary=move || !is_http.get()
                            class:blx-btn--ghost=move || is_http.get()
                            on:click=move |_| is_http.set(false)
                        >
                            {move || i18n.tr(I18nKey::McpTransportStdio)()}
                        </button>
                        <button
                            class="blx-btn"
                            class:blx-btn--primary=move || is_http.get()
                            class:blx-btn--ghost=move || !is_http.get()
                            on:click=move |_| is_http.set(true)
                        >
                            {move || i18n.tr(I18nKey::McpTransportHttp)()}
                        </button>
                    </div>
                </div>

                <Show
                    when=move || !is_http.get()
                    fallback=move || view! {
                        <label class="mcp-field">
                            <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpUrl)()}</span>
                            <input
                                class="blx-input"
                                prop:value=move || url.get()
                                on:input=move |ev| url.set(input_value(&ev))
                            />
                        </label>
                        <label class="mcp-field">
                            <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpHeaders)()}</span>
                            <textarea
                                class="blx-input mcp-textarea"
                                prop:value=move || headers.get()
                                on:input=move |ev| headers.set(textarea_value(&ev))
                            />
                        </label>
                    }
                >
                    <label class="mcp-field">
                        <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpCommand)()}</span>
                        <input
                            class="blx-input"
                            prop:value=move || command.get()
                            on:input=move |ev| command.set(input_value(&ev))
                        />
                    </label>
                    <label class="mcp-field">
                        <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpArgs)()}</span>
                        <textarea
                            class="blx-input mcp-textarea"
                            prop:value=move || args.get()
                            on:input=move |ev| args.set(textarea_value(&ev))
                        />
                    </label>
                    <label class="mcp-field">
                        <span class="mcp-field__label">{move || i18n.tr(I18nKey::McpEnv)()}</span>
                        <textarea
                            class="blx-input mcp-textarea"
                            prop:value=move || env.get()
                            on:input=move |ev| env.set(textarea_value(&ev))
                        />
                    </label>
                </Show>

                <label class="mcp-field mcp-field--inline">
                    <input
                        type="checkbox"
                        prop:checked=move || enabled.get()
                        on:change=move |ev| enabled.set(
                            ev.target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                .map(|i| i.checked())
                                .unwrap_or(true)
                        )
                    />
                    <span>{move || i18n.tr(I18nKey::McpEnabled)()}</span>
                </label>

                <div class="mcp-editor__actions">
                    <button class="blx-btn blx-btn--ghost" on:click=move |_| on_cancel.run(())>
                        {move || i18n.tr(I18nKey::McpCancel)()}
                    </button>
                    <button class="blx-btn blx-btn--primary" on:click=save>
                        {move || i18n.tr(I18nKey::BtnSave)()}
                    </button>
                </div>
            </div>
        </div>
    }
}
