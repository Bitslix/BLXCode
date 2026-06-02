//! Turn-end "Changed files" summary card. Renders the workspace files that
//! changed during a turn as a collapsible directory tree (reusing the shared
//! `.timeline-tree*` indentation convention), with per-file +/- line stats and
//! header actions (Collapse all / View diff). Data comes from a
//! [`TurnPart::ChangedFiles`](crate::workbench::agent_timeline::TurnPart)
//! snapshot taken via `git_status_changes` at turn end — no live polling here.

use std::collections::{BTreeMap, HashMap};

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::agent_panel::timeline::path_tail;
use crate::workbench::agent_timeline::ChangedFileEntry;
use crate::workbench::WorkbenchService;

/// A node in the changed-files directory tree.
#[derive(Clone)]
enum Node {
    Dir {
        /// Full workspace-relative dir path (stable key for open-state).
        path: String,
        /// Display label (may span several coalesced segments, e.g. `a/b`).
        name: String,
        children: Vec<Node>,
    },
    File(ChangedFileEntry),
}

#[derive(Default)]
struct DirBuild {
    dirs: BTreeMap<String, DirBuild>,
    files: Vec<ChangedFileEntry>,
}

impl DirBuild {
    fn insert(&mut self, entry: ChangedFileEntry) {
        let segments: Vec<&str> = entry.rel_path.split('/').collect();
        let Some((file_name, dirs)) = segments.split_last() else {
            return;
        };
        let _ = file_name;
        let mut node = self;
        for dir in dirs {
            node = node.dirs.entry((*dir).to_string()).or_default();
        }
        node.files.push(entry);
    }

    /// Convert into render nodes, coalescing single-child directory chains
    /// (e.g. `src-tauri` → `src` becomes one `src-tauri/src` node).
    fn into_nodes(self, prefix: &str) -> Vec<Node> {
        let mut out = Vec::new();
        for (name, mut sub) in self.dirs {
            let mut path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let mut label = name;
            // Coalesce: while this dir has exactly one child dir and no files.
            while sub.files.is_empty() && sub.dirs.len() == 1 {
                let (child_name, child) = sub.dirs.into_iter().next().expect("len == 1");
                label = format!("{label}/{child_name}");
                path = format!("{path}/{child_name}");
                sub = child;
            }
            out.push(Node::Dir {
                children: sub.into_nodes(&path),
                path,
                name: label,
            });
        }
        out.extend(self.files.into_iter().map(Node::File));
        out
    }
}

fn build_tree(files: &[ChangedFileEntry]) -> Vec<Node> {
    let mut root = DirBuild::default();
    for entry in files {
        root.insert(entry.clone());
    }
    root.into_nodes("")
}

/// Collect every directory path in the tree (for the "Collapse all" action).
fn collect_dir_paths(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        if let Node::Dir { path, children, .. } = node {
            out.push(path.clone());
            collect_dir_paths(children, out);
        }
    }
}

fn file_icon(rel_path: &str) -> icondata::Icon {
    let ext = rel_path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => icondata::LuFileCode,
        "ts" | "tsx" | "js" | "jsx" => icondata::LuFileCode,
        "css" | "scss" => icondata::LuFileCode,
        "html" | "htm" => icondata::LuFileCode,
        "json" | "toml" | "yaml" | "yml" => icondata::LuFileJson,
        "md" | "markdown" => icondata::LuFileText,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => icondata::LuFileImage,
        _ => icondata::LuFile,
    }
}

fn status_class(status: &str) -> &'static str {
    match status {
        "added" | "untracked" => "changed-files-card__file--added",
        "deleted" => "changed-files-card__file--deleted",
        "renamed" => "changed-files-card__file--renamed",
        "conflicted" => "changed-files-card__file--conflicted",
        _ => "changed-files-card__file--modified",
    }
}

#[component]
pub fn ChangedFilesCard(
    files: Vec<ChangedFileEntry>,
    wb: WorkbenchService,
    workspace_id: Option<u64>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let total = files.len();
    let total_added: u32 = files.iter().map(|f| f.added).sum();
    let total_removed: u32 = files.iter().map(|f| f.removed).sum();
    let first_path = files.first().map(|f| f.rel_path.clone());

    let nodes = build_tree(&files);
    let mut dir_paths = Vec::new();
    collect_dir_paths(&nodes, &mut dir_paths);

    // Per-dir open state; absent = open (default expanded).
    let open_state = RwSignal::new(HashMap::<String, bool>::new());
    let nodes_sv = StoredValue::new(nodes);

    let collapse_all = move |_| {
        open_state.update(|m| {
            for p in &dir_paths {
                m.insert(p.clone(), false);
            }
        });
    };

    let view_diff = move |_| {
        if let (Some(ws_id), Some(path)) = (workspace_id, first_path.clone()) {
            wb.open_center_diff_tab(ws_id, path, false);
        }
    };

    view! {
        <div class="changed-files-card">
            <div class="changed-files-card__head">
                <span class="changed-files-card__title">
                    {move || i18n.tr(I18nKey::AgChangedFilesTitle)()}
                    " ("{total}")"
                </span>
                <span class="changed-files-card__stat changed-files-card__stat--add">
                    "+"{total_added}
                </span>
                <span class="changed-files-card__stat changed-files-card__stat--del">
                    "−"{total_removed}
                </span>
                <span class="changed-files-card__spacer"></span>
                <button
                    type="button"
                    class="changed-files-card__action"
                    on:click=collapse_all
                >
                    {move || i18n.tr(I18nKey::AgChangedFilesCollapseAll)()}
                </button>
                <button
                    type="button"
                    class="changed-files-card__action"
                    on:click=view_diff
                >
                    {move || i18n.tr(I18nKey::AgChangedFilesViewDiff)()}
                </button>
            </div>
            <ul class="changed-files-card__tree timeline-tree">
                {move || nodes_sv.get_value().into_iter().map(|node| view! {
                    <NodeView node=node depth=0 open_state=open_state wb=wb workspace_id=workspace_id />
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn NodeView(
    node: Node,
    depth: usize,
    open_state: RwSignal<HashMap<String, bool>>,
    wb: WorkbenchService,
    workspace_id: Option<u64>,
) -> impl IntoView {
    let indent = format!("--timeline-depth: {depth}");
    match node {
        Node::Dir {
            path,
            name,
            children,
        } => {
            let path_for_open = path.clone();
            let is_open = Memo::new(move |_| {
                open_state.with(|m| m.get(&path_for_open).copied().unwrap_or(true))
            });
            let path_toggle = path.clone();
            let children_sv = StoredValue::new(children);
            view! {
                <li class="changed-files-card__dir" style=indent>
                    <button
                        type="button"
                        class="changed-files-card__dir-head"
                        aria-expanded=move || is_open.get().to_string()
                        on:click=move |_| {
                            let key = path_toggle.clone();
                            open_state.update(|m| {
                                let cur = m.get(&key).copied().unwrap_or(true);
                                m.insert(key, !cur);
                            });
                        }
                    >
                        <span class="changed-files-card__chevron" aria-hidden="true">
                            {move || if is_open.get() {
                                view! { <LxIcon icon=icondata::LuChevronDown width="0.82rem" height="0.82rem" /> }
                            } else {
                                view! { <LxIcon icon=icondata::LuChevronRight width="0.82rem" height="0.82rem" /> }
                            }}
                        </span>
                        <span class="changed-files-card__folder-icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuFolder width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="changed-files-card__dir-name">{name}</span>
                    </button>
                    <Show when=move || is_open.get()>
                        <ul class="changed-files-card__children timeline-tree__children">
                            {move || children_sv.get_value().into_iter().map(|child| view! {
                                <NodeView node=child depth=depth + 1 open_state=open_state wb=wb workspace_id=workspace_id />
                            }).collect_view()}
                        </ul>
                    </Show>
                </li>
            }
            .into_any()
        }
        Node::File(entry) => {
            let rel_path = entry.rel_path.clone();
            let display = path_tail(&rel_path);
            let icon = file_icon(&rel_path);
            let status_cls = status_class(&entry.status);
            let added = entry.added;
            let removed = entry.removed;
            let has_added = added > 0;
            let has_removed = removed > 0;
            let open_path = rel_path.clone();
            view! {
                <li class=format!("changed-files-card__file {status_cls}") style=indent>
                    <button
                        type="button"
                        class="changed-files-card__file-btn"
                        title=rel_path.clone()
                        on:click=move |_| {
                            if let Some(ws_id) = workspace_id {
                                wb.open_center_diff_tab(ws_id, open_path.clone(), false);
                            }
                        }
                    >
                        <span class="changed-files-card__file-icon" aria-hidden="true">
                            <LxIcon icon=icon width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="changed-files-card__file-name">{display}</span>
                        <Show when=move || has_added>
                            <span class="changed-files-card__stat changed-files-card__stat--add">"+"{added}</span>
                        </Show>
                        <Show when=move || has_removed>
                            <span class="changed-files-card__stat changed-files-card__stat--del">"−"{removed}</span>
                        </Show>
                    </button>
                </li>
            }
            .into_any()
        }
    }
}
