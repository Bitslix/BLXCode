//! Compact per-row metrics strip — `in`, `out`, `tok/s`, `ttft`, `$cost`.
//!
//! Rendered under Assistant rows, Tool rows, ModelDecision rows and
//! Subagent-tool rows. The component is intentionally side-effect free:
//! it reads a `TurnMetrics` snapshot and produces a single line.

use crate::agent_wire::TurnMetrics;
use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use leptos::prelude::*;

/// Where the bar is rendered. Currently only changes the wrapping CSS
/// class so the subagent variant can apply slightly tighter spacing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BarContext {
    /// Main chat row (Assistant / Tool / ModelDecision).
    Main,
    /// Inside a subagent card (per-tool or card header).
    Subagent,
}

#[component]
pub fn TurnMetricsBar(metrics: TurnMetrics, context: BarContext) -> impl IntoView {
    if metrics.is_empty() {
        return view! { <></> }.into_any();
    }
    let i18n = use_context::<I18nService>();
    let loc = i18n
        .as_ref()
        .map(|s| s.locale().get_untracked())
        .unwrap_or_default();
    let dash = lookup(loc, I18nKey::AgMetricsCostUnknown).to_string();

    let in_tok = metrics
        .input_tokens
        .map(fmt_compact_int)
        .unwrap_or_else(|| dash.clone());
    let out_tok = metrics
        .output_tokens
        .map(fmt_compact_int)
        .unwrap_or_else(|| dash.clone());
    let speed = match (metrics.output_tokens, metrics.elapsed_ms) {
        (Some(out), ms) if ms > 0 && out > 0 => {
            let tps = (out as f64) * 1000.0 / (ms as f64);
            format!("{tps:.1} tok/s")
        }
        _ => dash.clone(),
    };
    let ttft = metrics.ttft_ms.map(fmt_ms).unwrap_or_else(|| dash.clone());
    let cost = metrics.cost_usd.map(fmt_cost).unwrap_or_else(|| dash.clone());

    let label_in = lookup(loc, I18nKey::AgMetricsIn);
    let label_out = lookup(loc, I18nKey::AgMetricsOut);
    let label_ttft = lookup(loc, I18nKey::AgMetricsTtft);
    let tt_in = lookup(loc, I18nKey::AgMetricsTooltipIn);
    let tt_out = lookup(loc, I18nKey::AgMetricsTooltipOut);
    let tt_ttft = lookup(loc, I18nKey::AgMetricsTooltipTtft);
    let tt_speed = lookup(loc, I18nKey::AgMetricsTooltipSpeed);
    let tt_cost = lookup(loc, I18nKey::AgMetricsTooltipCost);
    let bar_aria = lookup(loc, I18nKey::AgMetricsBarAria);

    let wrap_class = match context {
        BarContext::Main => "turn-metrics-bar turn-metrics-bar--main",
        BarContext::Subagent => "turn-metrics-bar turn-metrics-bar--subagent",
    };

    view! {
        <div class=wrap_class aria-label=bar_aria>
            <span class="turn-metrics-bar__cell" title=tt_in>
                {label_in} " " <strong>{in_tok}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell" title=tt_out>
                {label_out} " " <strong>{out_tok}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell" title=tt_speed>
                <strong>{speed}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell" title=tt_ttft>
                {label_ttft} " " <strong>{ttft}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell turn-metrics-bar__cell--cost" title=tt_cost>
                <strong>{cost}</strong>
            </span>
        </div>
    }
    .into_any()
}

fn fmt_compact_int(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", (n as f64) / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", (n as f64) / 1_000.0)
    } else {
        n.to_string()
    }
}

fn fmt_ms(ms: u64) -> String {
    if ms >= 1_000 {
        format!("{:.2}s", (ms as f64) / 1_000.0)
    } else {
        format!("{ms}ms")
    }
}

pub fn fmt_cost(usd: f64) -> String {
    // < 1 cent → display in milli-USD (e.g. "0.4¢") so tiny per-row
    // numbers don't all show as "$0.00".
    if usd < 0.01 {
        format!("{:.1}¢", usd * 100.0)
    } else if usd < 1.0 {
        format!("${usd:.3}")
    } else {
        format!("${usd:.2}")
    }
}
