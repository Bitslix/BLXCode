//! Chat-header context-window occupancy meter.
//!
//! Shows how much of the active model's context window the current
//! conversation occupies: `used / max · NN%` with a thin progress bar.
//! `used` is the latest main-agent round's prompt size
//! (`ChatUsageStats.last_round_input_tokens`); `max` is resolved by the
//! backend (`agent_active_context_window`) and supplied by the parent via a
//! shared signal so the auto-compact trigger can reuse it. When the window
//! size is unknown the meter degrades to a plain token count (no bar/%).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

/// Warn / danger thresholds (percent of context window).
const WARN_PCT: u32 = 70;
const DANGER_PCT: u32 = 85;

/// Compact token formatting: `812`, `1.2k`, `112k`, `1.0M`.
pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{}k", n / 1000)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// Integer occupancy percent (0–100+) for `used` against `max`.
pub fn occupancy_pct(used: u64, max: u64) -> u32 {
    if max == 0 {
        return 0;
    }
    ((used as f64 / max as f64) * 100.0).round() as u32
}

#[component]
pub fn ContextMeter(
    wb: WorkbenchService,
    /// Resolved max context size for the active model; `None` when unknown.
    context_length: RwSignal<Option<u64>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    // Live occupancy = newest main-agent round's prompt size.
    let used = Memo::new(move |_| {
        wb.active_id()
            .get()
            .map(|id| wb.chat_usage_for_workspace(id).last_round_input_tokens)
            .unwrap_or(0)
    });

    view! {
        <Show when=move || { used.get() > 0 }>
            {move || {
                let u = used.get();
                let aria = i18n.tr(I18nKey::AgContextWindowAria)().to_string();
                match context_length.get() {
                    Some(max) if max > 0 => {
                        let pct = occupancy_pct(u, max);
                        let fill_pct = pct.min(100);
                        let level_class = if pct >= DANGER_PCT {
                            "agent-context-meter__fill agent-context-meter__fill--danger"
                        } else if pct >= WARN_PCT {
                            "agent-context-meter__fill agent-context-meter__fill--warn"
                        } else {
                            "agent-context-meter__fill"
                        };
                        let title = i18n.tr(I18nKey::AgContextWindowLabel)().to_string();
                        view! {
                            <div class="agent-context-meter" aria-label=aria title=title>
                                <span class="agent-context-meter__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuGauge width="0.72rem" height="0.72rem" />
                                </span>
                                <div class="agent-context-meter__bar">
                                    <div class=level_class style=format!("width:{fill_pct}%") />
                                </div>
                                <span class="agent-context-meter__text">
                                    {format!("{} / {} · {}%", fmt_tokens(u), fmt_tokens(max), pct)}
                                </span>
                            </div>
                        }
                        .into_any()
                    }
                    _ => {
                        let title = i18n.tr(I18nKey::AgContextWindowUnknown)().to_string();
                        view! {
                            <div class="agent-context-meter agent-context-meter--unknown" aria-label=aria title=title>
                                <span class="agent-context-meter__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuGauge width="0.72rem" height="0.72rem" />
                                </span>
                                <span class="agent-context-meter__text">
                                    {format!("{} tok", fmt_tokens(u))}
                                </span>
                            </div>
                        }
                        .into_any()
                    }
                }
            }}
        </Show>
    }
}
