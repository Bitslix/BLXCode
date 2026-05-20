//! Sidebar git commit graph with swim-lane SVG.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    git_commit_graph, GitCommitNode, GitGraphLayout, GitGraphRow, GIT_MISSING_CODE,
};
use crate::workbench::sidebar_view_section::{SidebarSectionIconBtn, SidebarViewSection};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::HashMap;

const LANE_PITCH: f64 = 14.0;
const LANE_ORIGIN: f64 = 8.0;
const ROW_H: f64 = 22.0;
const NODE_Y: f64 = 11.0;

#[component]
pub fn GitGraphSection(git_repo_available: ReadSignal<Option<bool>>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let collapsed = wb.sidebar_collapsed();

    let graph_open = RwSignal::new(wb.active_sidebar_graph_open());
    let layout = RwSignal::new(None::<GitGraphLayout>);
    let error_kind = RwSignal::new(None::<GraphErrorKind>);
    let load_gen = RwSignal::new(0u32);

    let title = Signal::derive(move || i18n.tr(I18nKey::SbGraphTitle)().to_uppercase());

    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.workspaces().get();
        let stored = wb.active_sidebar_graph_open();
        if graph_open.get_untracked() != stored {
            graph_open.set(stored);
        }
    });

    Effect::new(move |_| {
        let open = graph_open.get();
        if open != wb.active_sidebar_graph_open() {
            wb.set_active_sidebar_graph_open(open);
        }
    });

    let reload = move || load_gen.update(|g| *g = g.wrapping_add(1));

    let last_graph_cwd = StoredValue::new(None::<String>);
    let last_load_gen = StoredValue::new(0u32);

    Effect::new(move |_| {
        let gen = load_gen.get();
        let force_reload = gen != last_load_gen.get_value();
        last_load_gen.set_value(gen);
        let _ = wb.sidebar_repo_epoch().get();
        match git_repo_available.get() {
            Some(true) => {}
            Some(false) => {
                layout.set(None);
                error_kind.set(None);
                last_graph_cwd.set_value(None);
                return;
            }
            None => return,
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let cwd_load = cwd.clone();
        let had_layout = layout.get_untracked().is_some();
        let same_cwd = last_graph_cwd.with_value(|prev| prev.as_deref() == Some(cwd.as_str()));
        if same_cwd && had_layout && !force_reload {
            return;
        }
        last_graph_cwd.set_value(Some(cwd));
        if !had_layout {
            layout.set(None);
            error_kind.set(None);
        }
        spawn_local(async move {
            match git_commit_graph(cwd_load, Some(100)).await {
                Ok(g) => {
                    layout.set(Some(g));
                    error_kind.set(None);
                }
                Err(e) if e == GIT_MISSING_CODE => {
                    layout.set(None);
                    error_kind.set(Some(GraphErrorKind::GitMissing));
                }
                Err(_) => {
                    layout.set(None);
                    error_kind.set(Some(GraphErrorKind::LoadFailed));
                }
            }
        });
    });

    let show = move || {
        !collapsed.get() && git_repo_available.get() == Some(true)
    };

    view! {
        <Show when=show>
            <SidebarViewSection
                title=title
                section_id="sb-graph"
                open=graph_open
                toolbar=view! {
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbGraphRefresh
                        on_click=Callback::new(move |_| reload())
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                }.into_any()
            >
                <GitGraphBody layout=layout error_kind=error_kind />
            </SidebarViewSection>
        </Show>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GraphErrorKind {
    GitMissing,
    LoadFailed,
}

#[component]
fn GitGraphBody(
    layout: RwSignal<Option<GitGraphLayout>>,
    error_kind: RwSignal<Option<GraphErrorKind>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <div class="git-graph">
            <Show
                when=move || error_kind.get().is_some()
                fallback=move || {
                    let Some(g) = layout.get() else {
                        return view! { <p class="sidebar-view-section__empty">"…"</p> }.into_any();
                    };
                    if g.commits.is_empty() {
                        return view! {
                            <p class="sidebar-view-section__empty">{move || i18n.tr(I18nKey::SbGraphLoadError)()}</p>
                        }
                        .into_any();
                    }
                    view! { <GitGraphList layout=g /> }.into_any()
                }
            >
                <p class="sidebar-view-section__empty">
                    {move || match error_kind.get() {
                        Some(GraphErrorKind::GitMissing) => i18n.tr(I18nKey::SbGraphGitMissing)(),
                        _ => i18n.tr(I18nKey::SbGraphLoadError)(),
                    }}
                </p>
            </Show>
        </div>
    }
}

fn lane_x(lane: usize) -> f64 {
    LANE_ORIGIN + f64::from(lane as u32) * LANE_PITCH
}

fn connector_path(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
    let mid_y = (y0 + y1) / 2.0;
    format!("M {x0} {y0} C {x0} {mid_y}, {x1} {mid_y}, {x1} {y1}")
}

#[component]
fn GitGraphList(layout: GitGraphLayout) -> impl IntoView {
    let row_by_oid: HashMap<String, GitGraphRow> = layout
        .rows
        .iter()
        .map(|r| (r.oid.clone(), r.clone()))
        .collect();
    let max_lane = layout
        .rows
        .iter()
        .flat_map(|r| {
            let mut lanes = vec![r.lane];
            if let Some(m) = r.merge_from_lane {
                lanes.push(m);
            }
            if let Some(b) = r.branch_from_lane {
                lanes.push(b);
            }
            lanes.extend(r.pass_through_lanes.iter().copied());
            lanes
        })
        .max()
        .unwrap_or(0);
    let lane_count = max_lane + 1;

    view! {
        <ul class="git-graph__list" role="list">
            <For
                each=move || layout.commits.clone()
                key=|c| c.oid.clone()
                children=move |commit: GitCommitNode| {
                    let oid = commit.oid.clone();
                    let row = row_by_oid.get(&oid).cloned();
                    view! {
                        <GitGraphRowView commit=commit row=row lane_count=lane_count />
                    }
                }
            />
        </ul>
    }
}

#[component]
fn GitGraphRowView(
    commit: GitCommitNode,
    row: Option<GitGraphRow>,
    lane_count: usize,
) -> impl IntoView {
    let lane = row.as_ref().map(|r| r.lane).unwrap_or(0);
    let color_idx = row.as_ref().map(|r| r.lane_color_index).unwrap_or(0);
    let continues_up = row.as_ref().is_some_and(|r| r.continues_up);
    let continues_down = row.as_ref().is_some_and(|r| r.continues_down);
    let merge_from = row.as_ref().and_then(|r| r.merge_from_lane);
    let branch_from = row.as_ref().and_then(|r| r.branch_from_lane);
    let pass_through = row
        .as_ref()
        .map(|r| r.pass_through_lanes.clone())
        .unwrap_or_default();
    let is_merge = merge_from.is_some();

    let svg_w = (lane_count.max(1) as f64 * LANE_PITCH + LANE_ORIGIN).max(22.0);
    let x = lane_x(lane);

    view! {
        <li class="git-graph__row">
            <svg
                class="git-graph__lanes"
                width=format!("{svg_w}px")
                height=format!("{ROW_H}px")
                aria-hidden="true"
            >
                <For
                    each=move || pass_through.clone()
                    key=|lane| *lane
                    children=move |pass_lane| {
                        let px = lane_x(pass_lane);
                        let pass_color = pass_lane % 6;
                        view! {
                            <line
                                x1=px
                                y1="0"
                                x2=px
                                y2=format!("{ROW_H}")
                                class=format!("git-graph__line git-graph__line--c{pass_color}")
                            />
                        }
                    }
                />
                {continues_up.then(|| view! {
                    <line
                        x1=x
                        y1="0"
                        x2=x
                        y2=format!("{NODE_Y}")
                        class=format!("git-graph__line git-graph__line--c{color_idx}")
                    />
                })}
                {continues_down.then(|| view! {
                    <line
                        x1=x
                        y1=format!("{NODE_Y}")
                        x2=x
                        y2=format!("{ROW_H}")
                        class=format!("git-graph__line git-graph__line--c{color_idx}")
                    />
                })}
                {branch_from.map(|from_lane| {
                    let x0 = lane_x(from_lane);
                    let from_color = from_lane % 6;
                    let d = connector_path(x0, 0.0, x, NODE_Y);
                    view! {
                        <path
                            d=d
                            class=format!("git-graph__connector git-graph__line--c{from_color}")
                            fill="none"
                            stroke-width="1.5"
                        />
                    }
                })}
                {merge_from.map(|from_lane| {
                    let x0 = lane_x(from_lane);
                    let from_color = from_lane % 6;
                    let d = connector_path(x0, ROW_H, x, NODE_Y);
                    view! {
                        <path
                            d=d
                            class=format!("git-graph__connector git-graph__line--c{from_color}")
                            fill="none"
                            stroke-width="1.5"
                        />
                    }
                })}
                <circle
                    cx=x
                    cy=format!("{NODE_Y}")
                    r=if is_merge { "4.5" } else { "3.5" }
                    class=format!(
                        "git-graph__dot git-graph__dot--c{color_idx}{}",
                        if is_merge { " git-graph__dot--merge" } else { "" }
                    )
                />
                {is_merge.then(|| view! {
                    <circle
                        cx=x
                        cy=format!("{NODE_Y}")
                        r="2"
                        class="git-graph__dot-inner"
                    />
                })}
            </svg>
            <div class="git-graph__text">
                <div class="git-graph__subject-line">
                    <span class="git-graph__subject" title=commit.subject.clone()>
                        {commit.subject.clone()}
                    </span>
                    <For
                        each=move || commit.decorations.clone()
                        key=|d| (d.kind.clone(), d.label.clone())
                        children=move |d| {
                            view! {
                                <span class="git-graph__ref">{d.label.clone()}</span>
                            }
                        }
                    />
                </div>
                <span class="git-graph__meta">
                    {commit.author.clone()}" · "{commit.rel_time.clone()}
                </span>
            </div>
        </li>
    }
}
