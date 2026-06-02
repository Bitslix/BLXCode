//! Context-window formatting helpers shared by the Agent header stats and
//! auto-compaction UI.

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
