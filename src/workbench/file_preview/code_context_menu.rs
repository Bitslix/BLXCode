//! Right-click context menu for the [`CodeView`].
//!
//! Pure view layer: the parent owns the [`CodeContextMenuState`] signal
//! plus the [`CodeMenuAction`] callback that runs side effects
//! (pty_write, agent context attach, clipboard).
//!
//! The menu lists every workspace that has at least one live terminal —
//! the file preview's own workspace is moved to the front and gets a
//! "current" badge so the user can pick same-workspace targets at a
//! glance. A second section repeats the workspace grouping for the
//! "Attach to agent" action. A third clipboard section completes the
//! menu with three local-only copy variants.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::agent_context_handoff::{
    WorkspaceTerminalGroup, WorkspaceTerminalTarget,
};
use leptos::prelude::*;

/// Action emitted from a menu item. The parent component holds the
/// snippet builders + IPC plumbing and decides how to react.
#[derive(Clone, Debug)]
pub enum CodeMenuAction {
    InsertSnippetIntoTerminal {
        workspace_id: u64,
        workspace_label: String,
        target: WorkspaceTerminalTarget,
    },
    InsertEnvelopeIntoTerminal {
        workspace_id: u64,
        workspace_label: String,
        target: WorkspaceTerminalTarget,
    },
    AttachToAgent {
        workspace_id: u64,
        workspace_label: String,
    },
    CopySnippet,
    CopyRange,
    CopyRaw,
}

/// State driving the menu's visibility, anchor + content. Set to `Some`
/// to open, `None` to close.
#[derive(Clone, Debug)]
pub struct CodeContextMenuState {
    /// Viewport position of the open click (clientX, clientY).
    pub anchor_x: i32,
    pub anchor_y: i32,
    /// Currently-selected line range (1-based, inclusive). Used in copy
    /// section labels for visual feedback only.
    pub range: (u32, u32),
    /// Pre-enumerated terminals grouped by workspace.
    pub groups: Vec<WorkspaceTerminalGroup>,
    /// Workspace id of the file currently being previewed. Marked with
    /// a "current" badge in workspace group headers.
    pub preview_workspace_id: u64,
}

#[component]
pub fn CodeContextMenu(
    state: RwSignal<Option<CodeContextMenuState>>,
    on_action: Callback<CodeMenuAction>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <Show when=move || state.get().is_some()>
            {move || {
                let Some(s) = state.get() else {
                    return view! {}.into_any();
                };
                let style = format!("left: {}px; top: {}px;", s.anchor_x, s.anchor_y);
                let groups = s.groups.clone();
                let preview_ws_id = s.preview_workspace_id;
                view! {
                    <div
                        class="code-context-menu"
                        role="menu"
                        aria-label=move || i18n.tr(I18nKey::CodeViewMenuAria)()
                        style=style
                        on:mousedown=|ev| ev.stop_propagation()
                        on:click=|ev| ev.stop_propagation()
                        on:contextmenu=|ev| ev.prevent_default()
                    >
                        <SnippetTerminalSection
                            groups=groups.clone()
                            preview_ws_id=preview_ws_id
                            on_action=on_action
                            envelope=false
                        />
                        <EnvelopeTerminalSection
                            groups=groups.clone()
                            preview_ws_id=preview_ws_id
                            on_action=on_action
                        />
                        <AgentAttachSection
                            groups=groups.clone()
                            preview_ws_id=preview_ws_id
                            on_action=on_action
                        />
                        <ClipboardSection on_action=on_action />
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}

#[component]
fn SnippetTerminalSection(
    groups: Vec<WorkspaceTerminalGroup>,
    preview_ws_id: u64,
    on_action: Callback<CodeMenuAction>,
    envelope: bool,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let any_terminal = groups.iter().any(|g| !g.terminals.is_empty());
    let header_key = if envelope {
        I18nKey::CodeViewMenuSectionEnvelopeTerminal
    } else {
        I18nKey::CodeViewMenuSectionSnippetTerminal
    };
    view! {
        <div class="code-context-menu__section" role="group">
            <div class="code-context-menu__section-head">
                {move || i18n.tr(header_key)()}
            </div>
            {if !any_terminal {
                let msg = i18n.tr(I18nKey::CodeViewMenuNoTerminals)();
                view! { <div class="code-context-menu__empty">{msg}</div> }
                    .into_any()
            } else {
                view! {
                    <ul class="code-context-menu__list">
                        {groups
                            .into_iter()
                            .map(|group| {
                                let group_label_tpl =
                                    i18n.tr(I18nKey::CodeViewMenuWorkspaceGroup)();
                                let group_label = group_label_tpl
                                    .replace("{workspace}", &group.workspace_label);
                                let is_current = group.workspace_id == preview_ws_id;
                                let badge = if is_current {
                                    let badge_text = i18n
                                        .tr(I18nKey::CodeViewMenuPreviewWorkspaceBadge)();
                                    view! {
                                        <span class="code-context-menu__badge">
                                            {badge_text}
                                        </span>
                                    }
                                    .into_any()
                                } else {
                                    view! {}.into_any()
                                };
                                let ws_id = group.workspace_id;
                                let ws_label_for_actions = group.workspace_label.clone();
                                view! {
                                    <li class="code-context-menu__group">
                                        <div class="code-context-menu__group-head">
                                            <span>{group_label}</span>
                                            {badge}
                                        </div>
                                        <ul class="code-context-menu__sublist">
                                            {group
                                                .terminals
                                                .into_iter()
                                                .map(|t| {
                                                    let slot_tpl = i18n
                                                        .tr(I18nKey::CodeViewMenuTerminalSlotLabel)();
                                                    let agent = if t.agent_slug.is_empty() {
                                                        "shell".to_string()
                                                    } else {
                                                        t.agent_slug.clone()
                                                    };
                                                    let label = slot_tpl
                                                        .replace("{slot}", &t.slot_id.to_string())
                                                        .replace("{agent}", &agent);
                                                    let ws_id_for_click = ws_id;
                                                    let ws_label_for_click =
                                                        ws_label_for_actions.clone();
                                                    let target_for_click = t.clone();
                                                    let envelope_for_click = envelope;
                                                    view! {
                                                        <li>
                                                            <button
                                                                type="button"
                                                                role="menuitem"
                                                                class="code-context-menu__item"
                                                                on:click=move |_| {
                                                                    let action = if envelope_for_click {
                                                                        CodeMenuAction::InsertEnvelopeIntoTerminal {
                                                                            workspace_id: ws_id_for_click,
                                                                            workspace_label: ws_label_for_click.clone(),
                                                                            target: target_for_click.clone(),
                                                                        }
                                                                    } else {
                                                                        CodeMenuAction::InsertSnippetIntoTerminal {
                                                                            workspace_id: ws_id_for_click,
                                                                            workspace_label: ws_label_for_click.clone(),
                                                                            target: target_for_click.clone(),
                                                                        }
                                                                    };
                                                                    on_action.run(action);
                                                                }
                                                            >
                                                                {label}
                                                            </button>
                                                        </li>
                                                    }
                                                })
                                                .collect_view()}
                                        </ul>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                }
                .into_any()
            }}
        </div>
    }
}

#[component]
fn EnvelopeTerminalSection(
    groups: Vec<WorkspaceTerminalGroup>,
    preview_ws_id: u64,
    on_action: Callback<CodeMenuAction>,
) -> impl IntoView {
    // Same shape as the snippet section, but emits the envelope variant.
    view! {
        <SnippetTerminalSection
            groups=groups
            preview_ws_id=preview_ws_id
            on_action=on_action
            envelope=true
        />
    }
}

#[component]
fn AgentAttachSection(
    groups: Vec<WorkspaceTerminalGroup>,
    preview_ws_id: u64,
    on_action: Callback<CodeMenuAction>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="code-context-menu__section" role="group">
            <div class="code-context-menu__section-head">
                {move || i18n.tr(I18nKey::CodeViewMenuSectionSnippetAgent)()}
            </div>
            <ul class="code-context-menu__list">
                {groups
                    .into_iter()
                    .map(|group| {
                        let ws_id = group.workspace_id;
                        let ws_label = group.workspace_label;
                        let is_current = ws_id == preview_ws_id;
                        let label_tpl =
                            i18n.tr(I18nKey::CodeViewMenuAttachAgentLabel)();
                        let label = label_tpl.replace("{workspace}", &ws_label);
                        let badge = if is_current {
                            let badge_text = i18n
                                .tr(I18nKey::CodeViewMenuPreviewWorkspaceBadge)();
                            view! {
                                <span class="code-context-menu__badge">
                                    {badge_text}
                                </span>
                            }
                            .into_any()
                        } else {
                            view! {}.into_any()
                        };
                        let ws_label_click = ws_label.clone();
                        view! {
                            <li>
                                <button
                                    type="button"
                                    role="menuitem"
                                    class="code-context-menu__item"
                                    on:click=move |_| {
                                        on_action
                                            .run(CodeMenuAction::AttachToAgent {
                                                workspace_id: ws_id,
                                                workspace_label: ws_label_click.clone(),
                                            });
                                    }
                                >
                                    <span>{label}</span>
                                    {badge}
                                </button>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn ClipboardSection(on_action: Callback<CodeMenuAction>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="code-context-menu__section" role="group">
            <div class="code-context-menu__section-head">
                {move || i18n.tr(I18nKey::CodeViewMenuSectionClipboard)()}
            </div>
            <ul class="code-context-menu__list">
                <li>
                    <button
                        type="button"
                        role="menuitem"
                        class="code-context-menu__item"
                        on:click=move |_| on_action.run(CodeMenuAction::CopySnippet)
                    >
                        {move || i18n.tr(I18nKey::CodeViewMenuCopySnippet)()}
                    </button>
                </li>
                <li>
                    <button
                        type="button"
                        role="menuitem"
                        class="code-context-menu__item"
                        on:click=move |_| on_action.run(CodeMenuAction::CopyRange)
                    >
                        {move || i18n.tr(I18nKey::CodeViewMenuCopyRange)()}
                    </button>
                </li>
                <li>
                    <button
                        type="button"
                        role="menuitem"
                        class="code-context-menu__item"
                        on:click=move |_| on_action.run(CodeMenuAction::CopyRaw)
                    >
                        {move || i18n.tr(I18nKey::CodeViewMenuCopyRaw)()}
                    </button>
                </li>
            </ul>
        </div>
    }
}
