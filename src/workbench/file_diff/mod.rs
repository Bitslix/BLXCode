//! Center-tab diff viewer (inline). Loads `git diff [--cached]` for one
//! file and renders the unified-diff text with line classifiers
//! (`@@`, `+`, `-`). Side-by-side view is intentionally out-of-scope.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{git_file_diff, GIT_MISSING_CODE};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffErrorKind {
    GitMissing,
    LoadFailed,
}

#[component]
pub fn FileDiffDock(workspace_id: u64, rel_path: String, staged: bool) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let diff_text = RwSignal::new(None::<String>);
    let error_kind = RwSignal::new(None::<DiffErrorKind>);

    let rel_for_load = rel_path.clone();
    Effect::new(move |_| {
        let _ = wb.workspaces().get();
        let _ = wb.sidebar_repo_epoch().get();
        let cwd = wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.cwd.clone())
        });
        let Some(cwd) = cwd.filter(|c| !c.trim().is_empty()) else {
            return;
        };
        let rel = rel_for_load.clone();
        spawn_local(async move {
            match git_file_diff(cwd, rel, staged).await {
                Ok(text) => {
                    diff_text.set(Some(text));
                    error_kind.set(None);
                }
                Err(e) if e == GIT_MISSING_CODE => {
                    diff_text.set(None);
                    error_kind.set(Some(DiffErrorKind::GitMissing));
                }
                Err(_) => {
                    diff_text.set(None);
                    error_kind.set(Some(DiffErrorKind::LoadFailed));
                }
            }
        });
    });

    view! {
        <article class="file-diff-view">
            <header class="file-diff-view__head">
                <span class="file-diff-view__path" title=rel_path.clone()>
                    {rel_path.clone()}
                </span>
                <span class=move || {
                    let mut c = String::from("file-diff-view__badge");
                    if staged {
                        c.push_str(" file-diff-view__badge--staged");
                    } else {
                        c.push_str(" file-diff-view__badge--unstaged");
                    }
                    c
                }>
                    {if staged { "staged" } else { "unstaged" }}
                </span>
            </header>
            <div class="file-diff-view__body">
                <Show
                    when=move || error_kind.get().is_some()
                    fallback=move || {
                        let Some(text) = diff_text.get() else {
                            return view! {
                                <p class="file-diff-view__empty">"…"</p>
                            }
                            .into_any();
                        };
                        if text.trim().is_empty() {
                            return view! {
                                <p class="file-diff-view__empty">
                                    {move || i18n.tr(I18nKey::SbDiffViewerEmpty)()}
                                </p>
                            }
                            .into_any();
                        }
                        view! { <DiffLines text=text /> }.into_any()
                    }
                >
                    <p class="file-diff-view__empty">
                        {move || match error_kind.get() {
                            Some(DiffErrorKind::GitMissing) => i18n.tr(I18nKey::SbDiffGitMissing)(),
                            _ => i18n.tr(I18nKey::SbDiffViewerLoadError)(),
                        }}
                    </p>
                </Show>
            </div>
        </article>
    }
}

#[component]
fn DiffLines(text: String) -> impl IntoView {
    let lines: Vec<(usize, String, &'static str)> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let kind = classify_line(line);
            (i, line.to_string(), kind)
        })
        .collect();

    view! {
        <pre class="file-diff-view__pre" role="region">
            <For
                each=move || lines.clone()
                key=|(i, _, _)| *i
                children=move |(_, line, kind)| {
                    let class = format!("file-diff-view__line file-diff-view__line--{kind}");
                    view! {
                        <span class=class>{line}{"\n"}</span>
                    }
                }
            />
        </pre>
    }
}

fn classify_line(line: &str) -> &'static str {
    if line.starts_with("@@") {
        "hunk"
    } else if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") {
        "header"
    } else if line.starts_with('+') {
        "add"
    } else if line.starts_with('-') {
        "del"
    } else {
        "ctx"
    }
}
