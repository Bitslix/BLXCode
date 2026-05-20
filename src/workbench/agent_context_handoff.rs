//! Shared helpers for sending BLXCode agent context to a terminal CLI session.
//!
//! Used by:
//!   * the `harness.send_agent_context` agent tool (programmatic)
//!   * the memory-graph note preview popover ("send to terminal…")
//!   * the workspace terminal titlebar ("hand off" dropdown)
//!
//! The Markdown renderer here is the single source of truth for the prompt
//! shape — keep it terminal-safe (no base64, no large file bodies).

use crate::agent_wire::{AgentContextItem, AgentContextKind};
use crate::tauri_bridge::{
    agent_export_context_images, pty_write, AgentContextExportReport, AgentContextImageInput,
};
use crate::workbench::state::{AgentImageContextStatus, WorkbenchService, WorkspaceAgentImage};
use base64::Engine;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;

#[derive(Clone, Debug, Default)]
pub struct RenderInputs {
    pub workspace_root: Option<String>,
    pub slot_id: Option<u64>,
    pub agent_slug: Option<String>,
    pub context_items: Vec<AgentContextItem>,
    pub images: Vec<RenderImageMeta>,
    pub instruction: Option<String>,
    pub include_memory: bool,
    pub include_images: bool,
    pub manifest_path: Option<String>,
    pub images_dir: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RenderImageMeta {
    #[allow(dead_code)]
    pub id: String,
    pub label: String,
    pub mime: String,
    pub size_bytes: u64,
    pub status: &'static str,
    pub exported_path: Option<String>,
}

pub fn render_agent_context_block(input: &RenderInputs) -> String {
    let mut out = String::new();
    out.push_str("⟪ BLXCode attached context for this terminal agent ⟫\n");

    out.push_str("\n## Session\n");
    if let Some(root) = input.workspace_root.as_deref().filter(|s| !s.is_empty()) {
        out.push_str(&format!("- Workspace: `{root}`\n"));
    } else {
        out.push_str("- Workspace: <not set>\n");
    }
    match (input.slot_id, input.agent_slug.as_deref()) {
        (Some(sid), Some(slug)) if !slug.is_empty() => {
            out.push_str(&format!("- Target terminal: slot {sid} (agent={slug})\n"));
        }
        (Some(sid), _) => out.push_str(&format!("- Target terminal: slot {sid}\n")),
        (None, Some(slug)) if !slug.is_empty() => {
            out.push_str(&format!("- Target terminal: agent={slug}\n"));
        }
        _ => {}
    }

    if input.include_memory {
        out.push_str("\n## Attached memory / learnings / notes\n");
        if input.context_items.is_empty() {
            out.push_str("- (none)\n");
        } else {
            for item in &input.context_items {
                let kind = match item.kind {
                    AgentContextKind::MemoryCategory => "memory category",
                    AgentContextKind::LearningCategory => "learnings category",
                    AgentContextKind::MemoryNote => "memory note",
                    AgentContextKind::LearningNote => "learning note",
                };
                out.push_str(&format!("- [{kind}] {} — {}\n", item.label, item.source));
                if !item.paths.is_empty() && item.paths.len() <= 12 {
                    for p in &item.paths {
                        out.push_str(&format!("  - `{p}`\n"));
                    }
                } else if item.paths.len() > 12 {
                    out.push_str(&format!("  - ({} paths — see manifest)\n", item.paths.len()));
                }
            }
        }
    }

    if input.include_images {
        out.push_str("\n## Attached images\n");
        if input.images.is_empty() {
            out.push_str("- (none)\n");
        } else {
            for img in &input.images {
                let exported = img
                    .exported_path
                    .as_deref()
                    .map(|p| format!(" -> `{p}`"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- {} ({}, {} bytes, status={}){}\n",
                    img.label, img.mime, img.size_bytes, img.status, exported
                ));
            }
        }
        if let Some(dir) = input.images_dir.as_deref() {
            out.push_str(&format!("- images dir: `{dir}`\n"));
        }
        if let Some(manifest) = input.manifest_path.as_deref() {
            out.push_str(&format!("- manifest: `{manifest}`\n"));
        }
    }

    if let Some(text) = input
        .instruction
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        out.push_str("\n## Instruction\n");
        out.push_str(text);
        if !text.ends_with('\n') {
            out.push('\n');
        }
    }

    out.push_str("\n⟪ end BLXCode context ⟫\n");
    out
}

/// A live terminal slot+pane in the active workspace that can receive a handoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceTerminalTarget {
    pub slot_id: u64,
    pub pane_id: u64,
    pub session_id: u64,
    pub agent_slug: String,
    pub label: String,
}

#[must_use]
pub fn list_workspace_terminal_targets(
    wb: &WorkbenchService,
    workspace_id: u64,
) -> Vec<WorkspaceTerminalTarget> {
    let sessions = wb.pty_sessions_for_workspace(workspace_id);
    let labels: std::collections::HashMap<u64, String> = wb.workspaces().with_untracked(|all| {
        let Some(w) = all.iter().find(|w| w.id == workspace_id) else {
            return std::collections::HashMap::new();
        };
        w.slot_ids
            .iter()
            .enumerate()
            .map(|(idx, sid)| {
                let agent = w.slot_agent_labels.get(idx).cloned().unwrap_or_default();
                (*sid, agent)
            })
            .collect()
    });

    sessions
        .into_iter()
        .map(|(slot_id, pane_id, session_id)| {
            let agent_slug = labels.get(&slot_id).cloned().unwrap_or_default();
            let label = if agent_slug.is_empty() {
                format!("Slot {slot_id} · shell")
            } else {
                format!("Slot {slot_id} · {agent_slug}")
            };
            WorkspaceTerminalTarget {
                slot_id,
                pane_id,
                session_id,
                agent_slug,
                label,
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct HandoffRequest {
    pub workspace_id: u64,
    pub workspace_root: Option<String>,
    pub target: WorkspaceTerminalTarget,
    /// Context items to include in the rendered block. Pass `None` to use
    /// the workspace's currently-attached agent context; pass `Some(vec)` for
    /// a one-shot selection (e.g. a specific note from the graph preview).
    pub context_items: Option<Vec<AgentContextItem>>,
    pub include_memory: bool,
    pub include_images: bool,
    pub instruction: Option<String>,
    pub submit: bool,
}

#[derive(Clone, Debug)]
pub struct HandoffOutcome {
    pub bytes_written: usize,
    pub submitted: bool,
    pub images_exported: usize,
    pub manifest_path: Option<String>,
}

/// Render + (optionally) export images + write the block into the target PTY.
/// Pure async — call from a `spawn_local`. Does NOT mark images as
/// provider-consumed (terminal handoff is separate from BLXCode-Agent state).
pub async fn perform_handoff(
    wb: WorkbenchService,
    req: HandoffRequest,
) -> Result<HandoffOutcome, String> {
    let HandoffRequest {
        workspace_id,
        workspace_root,
        target,
        context_items,
        include_memory,
        include_images,
        instruction,
        submit,
    } = req;

    let context_items = if include_memory {
        context_items.unwrap_or_else(|| wb.agent_context_for_workspace_untracked(workspace_id))
    } else {
        Vec::new()
    };

    let images_full: Vec<WorkspaceAgentImage> = if include_images {
        wb.agent_images_for_workspace_untracked(workspace_id)
    } else {
        Vec::new()
    };

    let mut export_report: Option<AgentContextExportReport> = None;
    if include_images && !images_full.is_empty() {
        if let Some(cwd) = workspace_root.clone() {
            let inputs: Vec<AgentContextImageInput> = images_full
                .iter()
                .map(|i| AgentContextImageInput {
                    id: i.item.id.clone(),
                    label: i.item.label.clone(),
                    mime: i.item.mime.clone(),
                    bytes_b64: i.item.bytes_b64.clone(),
                    size_bytes: i.item.size_bytes,
                })
                .collect();
            export_report = Some(
                agent_export_context_images(cwd, inputs)
                    .await
                    .map_err(|e| format!("image export failed: {e}"))?,
            );
        }
    }

    let images_meta: Vec<RenderImageMeta> = images_full
        .iter()
        .map(|img| {
            let exported = export_report
                .as_ref()
                .and_then(|r| r.images.iter().find(|e| e.id == img.item.id))
                .map(|e| e.path.clone());
            RenderImageMeta {
                id: img.item.id.clone(),
                label: img.item.label.clone(),
                mime: img.item.mime.clone(),
                size_bytes: img.item.size_bytes,
                status: match img.status {
                    AgentImageContextStatus::Pending => "pending",
                    AgentImageContextStatus::Read => "read",
                },
                exported_path: exported,
            }
        })
        .collect();

    let inputs = RenderInputs {
        workspace_root,
        slot_id: Some(target.slot_id),
        agent_slug: if target.agent_slug.is_empty() {
            None
        } else {
            Some(target.agent_slug.clone())
        },
        context_items,
        images: images_meta,
        instruction,
        include_memory,
        include_images,
        manifest_path: export_report.as_ref().map(|r| r.manifest_path.clone()),
        images_dir: export_report.as_ref().map(|r| r.dir.clone()),
    };

    let block = render_agent_context_block(&inputs);
    let mut payload = block.clone();
    if submit {
        payload.push('\r');
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
    pty_write(target.session_id, b64).await?;

    Ok(HandoffOutcome {
        bytes_written: block.len(),
        submitted: submit,
        images_exported: export_report.as_ref().map(|r| r.images.len()).unwrap_or(0),
        manifest_path: export_report.map(|r| r.manifest_path),
    })
}

/// Compact dropdown listing available terminals plus a separator and a
/// "Send to BLXCode agent context" item. Used by the note-preview popover and
/// the terminal titlebar button.
///
/// - `note_path` (optional): when `Some`, the per-terminal handoff sends ONLY
///   this note as a one-shot `memory_note` / `learning_note` context item,
///   and the BLXCode-Agent entry attaches that note. When `None`, the
///   terminal handoff sends the workspace's full attached context, and the
///   BLXCode-Agent entry attaches the workspace's memory category (no-op if
///   already attached — idempotent).
/// - `label` is the visible name used for the rendered/attached item.
#[component]
pub fn HandoffMenu(
    wb: WorkbenchService,
    label: Signal<String>,
    note_path: Signal<Option<String>>,
    on_close: Callback<()>,
    on_status: Callback<(bool, String)>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let targets = Memo::new(move |_| {
        // Re-run when the workspace list OR the live PTY session map changes.
        let _ = wb.workspaces().get();
        let _ = wb.pty_sessions_signal().get();
        let Some(ws_id) = wb.active_id().get() else {
            return Vec::<WorkspaceTerminalTarget>::new();
        };
        list_workspace_terminal_targets(&wb, ws_id)
    });

    let send_to = move |t: WorkspaceTerminalTarget| {
        on_close.run(());
        let Some(ws_id) = wb.active_id().get_untracked() else {
            on_status.run((false, "no active workspace".into()));
            return;
        };
        let label_str = label.get_untracked();
        let path = note_path.get_untracked();
        let context_items = path
            .as_ref()
            .map(|p| vec![note_context_item(p, &label_str)]);
        let req = HandoffRequest {
            workspace_id: ws_id,
            workspace_root: wb.default_workspace_cwd(),
            target: t.clone(),
            context_items,
            include_memory: true,
            include_images: path.is_none(),
            instruction: None,
            submit: true,
        };
        let on_status = on_status;
        let i18n_for_status = i18n;
        spawn_local(async move {
            match perform_handoff(wb, req).await {
                Ok(outcome) => on_status.run((
                    true,
                    format!(
                        "{} → {} ({} bytes{})",
                        i18n_for_status.tr(I18nKey::HandoffOkSent)(),
                        t.label,
                        outcome.bytes_written,
                        if outcome.submitted { ", submitted" } else { "" }
                    ),
                )),
                Err(e) => on_status.run((
                    false,
                    format!("{}: {e}", i18n_for_status.tr(I18nKey::HandoffFailed)()),
                )),
            }
        });
    };

    let attach_to_agent = move |_| {
        on_close.run(());
        let Some(ws_id) = wb.active_id().get_untracked() else {
            on_status.run((false, "no active workspace".into()));
            return;
        };
        let label_str = label.get_untracked();
        let path = note_path.get_untracked();
        let item = match path {
            Some(p) => note_context_item(&p, &label_str),
            None => {
                // No specific source — attach the workspace's "memory" category as
                // a coarse default. Idempotent: existing entry is upserted.
                AgentContextItem {
                    id: "memory-category:memory".into(),
                    kind: AgentContextKind::MemoryCategory,
                    label: "Memory".into(),
                    source: "memory category".into(),
                    paths: Vec::new(),
                    added_at: 0,
                }
            }
        };
        let attach_label = item.label.clone();
        wb.upsert_workspace_agent_context(ws_id, item);
        on_status.run((
            true,
            format!(
                "{} → BLXCode agent · {}",
                i18n.tr(I18nKey::HandoffOkSent)(),
                attach_label
            ),
        ));
    };

    view! {
        <div class="workbench-handoff-menu" on:click=move |ev| ev.stop_propagation()>
            <div class="workbench-handoff-menu__head">
                {move || i18n.tr(I18nKey::HandoffPickTerminal)()}
            </div>
            <Show
                when=move || !targets.get().is_empty()
                fallback=move || {
                    let msg = i18n.tr(I18nKey::HandoffNoTerminals)();
                    view! { <div class="workbench-handoff-menu__empty">{msg}</div> }
                }
            >
                <ul class="workbench-handoff-menu__list">
                    <For
                        each=move || targets.get()
                        key=|t| (t.slot_id, t.session_id)
                        children=move |t: WorkspaceTerminalTarget| {
                            let t_for_click = t.clone();
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class="workbench-handoff-menu__item"
                                        on:click=move |_| send_to(t_for_click.clone())
                                    >
                                        <LxIcon icon=icondata::LuTerminal width="0.78rem" height="0.78rem" />
                                        <span>{t.label.clone()}</span>
                                    </button>
                                </li>
                            }
                        }
                    />
                </ul>
            </Show>
            <div class="workbench-handoff-menu__separator" role="separator"></div>
            <ul class="workbench-handoff-menu__list">
                <li>
                    <button
                        type="button"
                        class="workbench-handoff-menu__item"
                        on:click=attach_to_agent
                    >
                        <LxIcon icon=icondata::LuLayers width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::HandoffToAgentContext)()}</span>
                    </button>
                </li>
            </ul>
        </div>
    }
}

/// Per-slot handoff dropdown rendered inside the terminal titlebar. The
/// button opens a `HandoffMenu` listing every live terminal in the workspace
/// plus a separator and a "Send to BLXCode agent context" entry. The
/// `note_path` is `None` here (no specific source note) — so the menu hands
/// off the workspace's attached BLXCode-Agent context to the chosen terminal.
#[component]
pub fn TerminalSlotHandoffButton(
    slot_id: u64,
    pane_id: u64,
    #[allow(unused_variables)] agent_slug: String,
    workspace_id: u64,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let status = RwSignal::new(None::<(bool, String)>);
    let open = RwSignal::new(false);
    let _ = (slot_id, pane_id, workspace_id, agent_slug);

    let icon = move || match status.get() {
        Some((true, _)) => view! {
            <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
        }
        .into_any(),
        Some((false, _)) => view! {
            <LxIcon icon=icondata::LuAlertTriangle width="0.82rem" height="0.82rem" />
        }
        .into_any(),
        None => view! {
            <LxIcon icon=icondata::LuShare2 width="0.82rem" height="0.82rem" />
        }
        .into_any(),
    };

    view! {
        <div class="workbench-handoff-anchor">
            <button
                type="button"
                class=move || {
                    let mut c = String::from("ws-term-cell__tool");
                    match status.get() {
                        Some((true, _)) => c.push_str(" ws-term-cell__tool--ok"),
                        Some((false, _)) => c.push_str(" ws-term-cell__tool--danger"),
                        None => {}
                    }
                    c
                }
                title=move || {
                    status
                        .with(|s| s.as_ref().map(|(_, msg)| msg.clone()))
                        .unwrap_or_else(|| i18n.tr(I18nKey::HandoffSendContext)().to_string())
                }
                aria-label=move || i18n.tr(I18nKey::HandoffSendContext)()
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |ev| {
                    ev.stop_propagation();
                    status.set(None);
                    open.update(|v| *v = !*v);
                }
            >
                {icon}
            </button>
            <Show when=move || open.get()>
                <HandoffMenu
                    wb=wb
                    label=Signal::derive(move || String::from("Workspace context"))
                    note_path=Signal::derive(move || None::<String>)
                    on_close=Callback::new(move |_| open.set(false))
                    on_status=Callback::new(move |s: (bool, String)| {
                        status.set(Some(s));
                        schedule_status_clear(status);
                    })
                />
            </Show>
        </div>
    }
}

fn schedule_status_clear(status: RwSignal<Option<(bool, String)>>) {
    spawn_local(async move {
        TimeoutFuture::new(2800).await;
        status.set(None);
    });
}

/// Build a one-shot `AgentContextItem` representing a single memory or
/// learnings note path. Used by the graph preview "send to terminal" button.
#[must_use]
pub fn note_context_item(path: &str, label: &str) -> AgentContextItem {
    let kind = if path.starts_with("learnings/") {
        AgentContextKind::LearningNote
    } else {
        AgentContextKind::MemoryNote
    };
    AgentContextItem {
        id: format!("memory-note:{path}"),
        kind,
        label: label.to_owned(),
        source: path.to_owned(),
        paths: vec![path.to_owned()],
        added_at: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_empty_block() {
        let inputs = RenderInputs {
            include_memory: true,
            include_images: true,
            ..Default::default()
        };
        let out = render_agent_context_block(&inputs);
        assert!(out.contains("BLXCode attached context"));
        assert!(out.contains("Workspace: <not set>"));
        assert!(out.contains("## Attached memory"));
        assert!(out.contains("## Attached images"));
    }

    #[test]
    fn renders_memory_only() {
        let inputs = RenderInputs {
            workspace_root: Some("/tmp/ws".into()),
            slot_id: Some(3),
            agent_slug: Some("codex".into()),
            include_memory: true,
            include_images: false,
            context_items: vec![note_context_item("notes/plans.md", "Plans")],
            ..Default::default()
        };
        let out = render_agent_context_block(&inputs);
        assert!(out.contains("Workspace: `/tmp/ws`"));
        assert!(out.contains("slot 3 (agent=codex)"));
        assert!(out.contains("[memory note] Plans"));
        assert!(out.contains("`notes/plans.md`"));
        assert!(!out.contains("## Attached images"));
    }

    #[test]
    fn renders_images_only_with_paths() {
        let inputs = RenderInputs {
            include_memory: false,
            include_images: true,
            images: vec![RenderImageMeta {
                id: "img-1".into(),
                label: "Cover".into(),
                mime: "image/png".into(),
                size_bytes: 42,
                status: "pending",
                exported_path: Some("/tmp/ws/.blxcode/agent-context/images/cover.png".into()),
            }],
            manifest_path: Some("/tmp/ws/.blxcode/agent-context/manifest.json".into()),
            images_dir: Some("/tmp/ws/.blxcode/agent-context".into()),
            ..Default::default()
        };
        let out = render_agent_context_block(&inputs);
        assert!(!out.contains("## Attached memory"));
        assert!(out.contains("## Attached images"));
        assert!(out.contains(
            "Cover (image/png, 42 bytes, status=pending) -> `/tmp/ws/.blxcode/agent-context/images/cover.png`"
        ));
        assert!(out.contains("manifest: `/tmp/ws/.blxcode/agent-context/manifest.json`"));
    }

    #[test]
    fn renders_mixed_with_instruction() {
        let inputs = RenderInputs {
            workspace_root: Some("/repo".into()),
            include_memory: true,
            include_images: true,
            context_items: vec![note_context_item(
                "learnings/index.md",
                "Learnings",
            )],
            images: vec![RenderImageMeta {
                id: "img".into(),
                label: "Shot".into(),
                mime: "image/png".into(),
                size_bytes: 42,
                status: "pending",
                exported_path: None,
            }],
            instruction: Some("Run /status and report".into()),
            ..Default::default()
        };
        let out = render_agent_context_block(&inputs);
        assert!(out.contains("[learning note] Learnings"));
        assert!(out.contains("Shot (image/png, 42 bytes, status=pending)"));
        assert!(out.contains("## Instruction"));
        assert!(out.contains("Run /status and report"));
        assert!(out.ends_with("⟪ end BLXCode context ⟫\n"));
    }

    #[test]
    fn note_context_item_picks_kind_by_path() {
        let mem = note_context_item("notes/foo.md", "Foo");
        assert_eq!(mem.kind, AgentContextKind::MemoryNote);
        let learning = note_context_item("learnings/bar.md", "Bar");
        assert_eq!(learning.kind, AgentContextKind::LearningNote);
    }
}
