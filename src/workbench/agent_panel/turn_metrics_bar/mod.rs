//! Compact per-row metrics strip — `in`, `out`, `tok/s`, `ttft`, `$cost`.
//!
//! Rendered under Assistant rows, Tool rows, ModelDecision rows and
//! Subagent-tool rows. The component is intentionally side-effect free:
//! it reads a `TurnMetrics` snapshot and produces a single line.
//!
//! i18n strings are still English literals here — they get swapped to
//! `I18nKey::AgMetrics*` by the `i18n-keys` task that follows.

use crate::agent_wire::TurnMetrics;
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

    let in_tok = metrics
        .input_tokens
        .map(fmt_compact_int)
        .unwrap_or_else(em_dash);
    let out_tok = metrics
        .output_tokens
        .map(fmt_compact_int)
        .unwrap_or_else(em_dash);
    let speed = match (metrics.output_tokens, metrics.elapsed_ms) {
        (Some(out), ms) if ms > 0 && out > 0 => {
            let tps = (out as f64) * 1000.0 / (ms as f64);
            format!("{tps:.1} tok/s")
        }
        _ => em_dash(),
    };
    let ttft = metrics.ttft_ms.map(fmt_ms).unwrap_or_else(em_dash);
    let cost = metrics.cost_usd.map(fmt_cost).unwrap_or_else(em_dash);

    let wrap_class = match context {
        BarContext::Main => "turn-metrics-bar turn-metrics-bar--main",
        BarContext::Subagent => "turn-metrics-bar turn-metrics-bar--subagent",
    };

    view! {
        <div class=wrap_class aria-label="Turn metrics">
            <span class="turn-metrics-bar__cell" title="Input / prompt tokens">
                "in " <strong>{in_tok}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell" title="Output / completion tokens">
                "out " <strong>{out_tok}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell" title="Output decode speed">
                <strong>{speed}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell" title="Time to first token">
                "ttft " <strong>{ttft}</strong>
            </span>
            <span class="turn-metrics-bar__sep">"·"</span>
            <span class="turn-metrics-bar__cell turn-metrics-bar__cell--cost" title="Resolved USD cost">
                <strong>{cost}</strong>
            </span>
        </div>
    }
    .into_any()
}

fn em_dash() -> String {
    "—".to_string()
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
