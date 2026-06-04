use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, pty_wait_output, pty_write, workbench_load_usage_snapshot,
};
use crate::workbench::state::WorkbenchService;
use base64::Engine;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::portal::Portal;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use serde_json::Value;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalUsageSnapshot {
    pub provider: String,
    pub updated_at: Option<String>,
    pub windows: Vec<TerminalUsageWindow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalUsageWindow {
    pub label: String,
    /// Used percentage, 0-100.
    pub used_percentage: f64,
    pub reset_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum UsageState {
    Idle,
    Loading,
    Ready(TerminalUsageSnapshot),
    Unavailable(String),
}

#[component]
pub fn TerminalUsageButton(terminal_key: String, agent_slug: String) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let open = RwSignal::new(false);
    let state = RwSignal::new(UsageState::Idle);
    // Render the popover in a Portal. Terminal cells apply overflow and visual
    // filters that otherwise turn fixed positioning into cell-relative layout.
    let button_ref = NodeRef::<leptos::html::Button>::new();
    let popover_style = RwSignal::new(String::new());
    let terminal_key_for_visible = terminal_key.clone();
    let agent_slug_for_visible = agent_slug.clone();
    let visible = Signal::derive(move || {
        if !is_tauri_shell() || !usage_supported_agent(&agent_slug_for_visible) {
            return false;
        }
        if agent_slug_for_visible.trim() == "claude"
            && wb
                .remote_connection_for_terminal_key(&terminal_key_for_visible)
                .is_some()
        {
            return false;
        }
        let sessions = wb.pty_sessions_signal().get();
        sessions.contains_key(&terminal_key_for_visible)
    });

    let reposition = move || {
        let Some(btn) = button_ref.get_untracked() else {
            return;
        };
        let rect = btn.get_bounding_client_rect();
        let win = web_sys::window();
        let vw = win
            .as_ref()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(rect.right());
        let vh = win
            .as_ref()
            .and_then(|w| w.inner_height().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(rect.bottom());
        // Mirror `width: min(21rem, 100vw - 2rem)` (1rem == 16px).
        let margin = 8.0;
        let width = (21.0_f64 * 16.0).min(vw - 2.0 * 16.0).max(0.0);
        // Anchor the popover's right edge to the button, then clamp.
        let mut left = rect.right() - width;
        let max_left = (vw - margin - width).max(margin);
        left = left.clamp(margin, max_left);
        let top = (rect.bottom() + margin).min((vh - margin).max(margin));
        popover_style.set(format!(
            "position: fixed; top: {top:.0}px; left: {left:.0}px; right: auto; width: {width:.0}px; max-height: calc(100vh - {:.0}px); overflow-y: auto;",
            top + margin
        ));
    };

    let close_click = window_event_listener_untyped("click", {
        let open = open;
        move |ev| {
            if !open.get_untracked() {
                return;
            }
            let inside = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|el| {
                    el.closest(".terminal-usage, .terminal-usage__popover")
                        .ok()
                        .flatten()
                })
                .is_some();
            if !inside {
                open.set(false);
            }
        }
    });
    let close_esc = window_event_listener_untyped("keydown", {
        let open = open;
        move |ev| {
            let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if ev.key() == "Escape" {
                open.set(false);
            }
        }
    });
    let reposition_resize = window_event_listener_untyped("resize", move |_| {
        if open.get_untracked() {
            reposition();
        }
    });
    on_cleanup(move || {
        close_click.remove();
        close_esc.remove();
        reposition_resize.remove();
    });

    let refresh = Callback::new({
        let terminal_key = terminal_key.clone();
        let agent_slug = agent_slug.clone();
        move |()| {
            if !visible.get_untracked() {
                return;
            }
            state.set(UsageState::Loading);
            let terminal_key = terminal_key.clone();
            let agent_slug = agent_slug.clone();
            let sessions = wb.pty_sessions_signal().get_untracked();
            let session_id = sessions.get(&terminal_key).copied();
            spawn_local(async move {
                let result = refresh_usage(i18n, &agent_slug, &terminal_key, session_id).await;
                state.set(match result {
                    Ok(snapshot) if !snapshot.windows.is_empty() => UsageState::Ready(snapshot),
                    Ok(_) => {
                        UsageState::Unavailable(i18n.tr(I18nKey::UsageUnavailable)().to_string())
                    }
                    Err(err) => UsageState::Unavailable(err),
                });
            });
        }
    });

    view! {
        <Show when=move || visible.get()>
            <div class="terminal-usage">
                <button
                    type="button"
                    node_ref=button_ref
                    class="ws-term-cell__tool terminal-usage__button"
                    class:terminal-usage__button--active=move || open.get()
                    prop:draggable=false
                    title=move || i18n.tr(I18nKey::UsageAgentUsage)()
                    aria-label=move || i18n.tr(I18nKey::UsageAgentUsage)()
                    aria-haspopup="menu"
                    aria-expanded=move || open.get().to_string()
                    on:mousedown=|ev: web_sys::MouseEvent| ev.stop_propagation()
                    on:click={
                        let refresh = refresh.clone();
                        move |ev| {
                            ev.stop_propagation();
                            let next = !open.get_untracked();
                            open.set(next);
                            if next {
                                reposition();
                                refresh.run(());
                            }
                        }
                    }
                >
                    <LxIcon icon=icondata::LuGauge width="0.82rem" height="0.82rem" />
                </button>
                <Show when=move || open.get()>
                    <Portal>
                        <div
                            class="terminal-usage__popover"
                            style=move || popover_style.get()
                            role="menu"
                            on:mousedown=|ev: web_sys::MouseEvent| ev.stop_propagation()
                            on:click=|ev: web_sys::MouseEvent| ev.stop_propagation()
                        >
                            <div class="terminal-usage__head">
                                <span>USAGE</span>
                                <button
                                    type="button"
                                    class="terminal-usage__refresh"
                                    title=move || i18n.tr(I18nKey::UsageRefreshUsage)()
                                    aria-label=move || i18n.tr(I18nKey::UsageRefreshUsage)()
                                    disabled=move || matches!(state.get(), UsageState::Loading)
                                    on:click={
                                        let refresh = refresh;
                                        move |_| refresh.run(())
                                    }
                                >
                                    <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                                </button>
                            </div>
                            {move || usage_body(state.get()).into_any()}
                        </div>
                    </Portal>
                </Show>
            </div>
        </Show>
    }
}

fn usage_body(state: UsageState) -> impl IntoView {
    match state {
        UsageState::Idle | UsageState::Loading => view! {
            <div class="terminal-usage__status">Loading usage...</div>
        }
        .into_any(),
        UsageState::Unavailable(message) => view! {
            <div class="terminal-usage__status">{message}</div>
        }
        .into_any(),
        UsageState::Ready(snapshot) => {
            let provider = snapshot.provider;
            let updated_at = snapshot.updated_at;
            let updated_at_for_when = updated_at.clone();
            let updated_at_for_text = updated_at.clone();
            let windows = snapshot.windows;
            view! {
            <div class="terminal-usage__provider">{provider}</div>
            <div class="terminal-usage__rows">
                <For
                    each=move || windows.clone()
                    key=|window| window.label.clone()
                    children=move |window| {
                        let pct = window.used_percentage.clamp(0.0, 100.0);
                        let reset_for_when = window.reset_text.clone();
                        let reset_for_text = window.reset_text.clone();
                        view! {
                            <div class="terminal-usage__row">
                                <div class="terminal-usage__row-top">
                                    <span>{window.label.clone()}</span>
                                    <span>{format!("{pct:.0}%")}</span>
                                </div>
                                <div class="terminal-usage__meter" aria-hidden="true">
                                    <span
                                        class="terminal-usage__meter-fill"
                                        style=format!("width: {pct:.1}%;")
                                    ></span>
                                </div>
                                <Show when=move || reset_for_when.is_some()>
                                    <div class="terminal-usage__reset">
                                        {reset_for_text.clone().unwrap_or_default()}
                                    </div>
                                </Show>
                            </div>
                        }
                    }
                />
            </div>
            <Show when=move || updated_at_for_when.is_some()>
                <div class="terminal-usage__updated">
                    {updated_at_for_text.clone().map(|ts| format!("Updated {ts}")).unwrap_or_default()}
                </div>
            </Show>
        }
        .into_any()
        }
    }
}

async fn refresh_usage(
    i18n: I18nService,
    agent_slug: &str,
    terminal_key: &str,
    session_id: Option<u64>,
) -> Result<TerminalUsageSnapshot, String> {
    match agent_slug.trim() {
        "claude" => {
            let raw = workbench_load_usage_snapshot(terminal_key.to_string())
                .await?
                .ok_or_else(|| {
                    i18n.tr(I18nKey::UsageUnavailableUntilClaudeUpdatesItsStatus)().to_string()
                })?;
            parse_claude_usage_snapshot(&raw)
        }
        "codex" => {
            refresh_interactive_usage(
                i18n,
                session_id
                    .ok_or_else(|| i18n.tr(I18nKey::UsageNoRunningTerminalSession)().to_string())?,
                "/status\r",
                parse_codex_usage_output,
            )
            .await
        }
        "gemini" => {
            refresh_interactive_usage(
                i18n,
                session_id
                    .ok_or_else(|| i18n.tr(I18nKey::UsageNoRunningTerminalSession)().to_string())?,
                "/stats model\r",
                parse_gemini_usage_output,
            )
            .await
        }
        _ => Err(i18n.tr(I18nKey::UsageUnavailableForThisAgent)().to_string()),
    }
}

async fn refresh_interactive_usage(
    i18n: I18nService,
    session_id: u64,
    command: &str,
    parser: fn(&str) -> Result<TerminalUsageSnapshot, String>,
) -> Result<TerminalUsageSnapshot, String> {
    let before = pty_wait_output(session_id, None, Some(1), Some(0), Some(8192), None).await?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(command.as_bytes());
    pty_write(session_id, encoded).await?;
    let after = pty_wait_output(
        session_id,
        Some(before.seq),
        Some(5000),
        Some(350),
        Some(65_536),
        None,
    )
    .await?;
    if after.timed_out && after.seq <= before.seq {
        return Err(i18n.tr(I18nKey::UsageUsageCommandTimedOut)().to_string());
    }
    parser(&after.text)
}

fn usage_supported_agent(agent_slug: &str) -> bool {
    matches!(agent_slug.trim(), "claude" | "codex" | "gemini")
}

fn parse_claude_usage_snapshot(raw: &str) -> Result<TerminalUsageSnapshot, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| format!("usage parse failed: {e}"))?;
    let payload = v
        .get("payload")
        .ok_or_else(|| "Claude usage payload missing".to_string())?;
    let limits = payload
        .get("rate_limits")
        .ok_or_else(|| "Claude rate limits missing".to_string())?;
    let mut windows = Vec::new();
    if let Some(window) = limits.get("five_hour") {
        if let Some(row) = claude_window("Session (5hr)", window) {
            windows.push(row);
        }
    }
    if let Some(window) = limits.get("seven_day") {
        if let Some(row) = claude_window("Weekly (7 day)", window) {
            windows.push(row);
        }
    }
    Ok(TerminalUsageSnapshot {
        provider: "Claude".into(),
        updated_at: v
            .get("updated_at")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        windows,
    })
}

fn claude_window(label: &str, value: &Value) -> Option<TerminalUsageWindow> {
    let pct = value.get("used_percentage")?.as_f64()?;
    let reset_text = value
        .get("resets_at")
        .and_then(|v| v.as_f64())
        .map(reset_text_from_epoch_seconds);
    Some(TerminalUsageWindow {
        label: label.into(),
        used_percentage: pct,
        reset_text,
    })
}

fn parse_codex_usage_output(raw: &str) -> Result<TerminalUsageSnapshot, String> {
    parse_text_usage_output("Codex", raw)
}

fn parse_gemini_usage_output(raw: &str) -> Result<TerminalUsageSnapshot, String> {
    parse_text_usage_output("Gemini", raw)
}

fn parse_text_usage_output(provider: &str, raw: &str) -> Result<TerminalUsageSnapshot, String> {
    let clean = strip_ansi(raw);
    let mut windows: Vec<TerminalUsageWindow> = Vec::new();
    for line in clean.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(raw_pct) = first_percent(line) else {
            continue;
        };
        let lower = line.to_ascii_lowercase();
        let label = if lower.contains("5h")
            || lower.contains("5 h")
            || lower.contains("5-hour")
            || lower.contains("5 hour")
            || lower.contains("5hr")
        {
            "Session (5hr)"
        } else if lower.contains("weekly")
            || lower.contains("week")
            || lower.contains("7d")
            || lower.contains("7 d")
            || lower.contains("7-day")
            || lower.contains("7 day")
        {
            "Weekly (7 day)"
        } else if lower.contains("context") {
            "Context"
        } else if lower.contains("quota") || lower.contains("limit") || lower.contains("usage") {
            "Usage"
        } else {
            continue;
        };
        let used = if lower.contains("remaining")
            || lower.contains("left")
            || lower.contains("available")
            || lower.contains("unused")
        {
            100.0 - raw_pct
        } else {
            raw_pct
        };
        let reset_text = reset_text_from_line(line);
        upsert_window(
            &mut windows,
            TerminalUsageWindow {
                label: label.into(),
                used_percentage: used.clamp(0.0, 100.0),
                reset_text,
            },
        );
    }
    if windows.is_empty() {
        return Err("Usage unavailable".into());
    }
    Ok(TerminalUsageSnapshot {
        provider: provider.into(),
        updated_at: None,
        windows,
    })
}

fn upsert_window(windows: &mut Vec<TerminalUsageWindow>, next: TerminalUsageWindow) {
    if let Some(existing) = windows.iter_mut().find(|w| w.label == next.label) {
        *existing = next;
    } else {
        windows.push(next);
    }
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn first_percent(line: &str) -> Option<f64> {
    let bytes = line.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] != b'%' {
            idx += 1;
            continue;
        }
        let mut start = idx;
        while start > 0 {
            let c = bytes[start - 1] as char;
            if c.is_ascii_digit() || c == '.' || c == ' ' {
                start -= 1;
            } else {
                break;
            }
        }
        let candidate = line[start..idx].trim();
        if let Ok(value) = candidate.parse::<f64>() {
            return Some(value);
        }
        idx += 1;
    }
    None
}

fn reset_text_from_line(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let pos = lower.find("resets").or_else(|| lower.find("reset"))?;
    let text = line[pos..].trim();
    if text.is_empty() {
        None
    } else {
        Some(capitalize_reset(text))
    }
}

fn capitalize_reset(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.collect::<String>()
    )
}

fn reset_text_from_epoch_seconds(seconds: f64) -> String {
    let now = current_epoch_seconds();
    let remaining = (seconds - now).max(0.0);
    if remaining < 3600.0 {
        format!("Resets in {:.0}m", (remaining / 60.0).ceil())
    } else if remaining < 86_400.0 {
        format!("Resets in {:.0}h", (remaining / 3600.0).ceil())
    } else {
        format!("Resets in {:.0}d", (remaining / 86_400.0).ceil())
    }
}

#[cfg(target_arch = "wasm32")]
fn current_epoch_seconds() -> f64 {
    js_sys::Date::now() / 1000.0
}

#[cfg(not(target_arch = "wasm32"))]
fn current_epoch_seconds() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_rate_limit_json() {
        let raw = r#"{
          "agent": "claude",
          "updated_at": "2026-06-03T10:00:00Z",
          "payload": {
            "rate_limits": {
              "five_hour": { "used_percentage": 96, "resets_at": 1917230400 },
              "seven_day": { "used_percentage": 52, "resets_at": 1917489600 }
            }
          }
        }"#;
        let parsed = parse_claude_usage_snapshot(raw).unwrap();
        assert_eq!(parsed.provider, "Claude");
        assert_eq!(parsed.windows.len(), 2);
        assert_eq!(parsed.windows[0].label, "Session (5hr)");
        assert_eq!(parsed.windows[0].used_percentage, 96.0);
    }

    #[test]
    fn parses_remaining_text_as_used_percentage() {
        let parsed = parse_codex_usage_output("5h limit: 73% left (resets 1 Apr, 18:22)").unwrap();
        assert_eq!(parsed.windows[0].label, "Session (5hr)");
        assert_eq!(parsed.windows[0].used_percentage, 27.0);
    }

    #[test]
    fn parses_used_text_and_strips_ansi() {
        let parsed =
            parse_gemini_usage_output("\u{1b}[32mWeekly quota usage 52%\u{1b}[0m resets in 3d")
                .unwrap();
        assert_eq!(parsed.windows[0].label, "Weekly (7 day)");
        assert_eq!(parsed.windows[0].used_percentage, 52.0);
        assert_eq!(
            parsed.windows[0].reset_text.as_deref(),
            Some("Resets in 3d")
        );
    }

    #[test]
    fn malformed_text_is_unavailable() {
        assert!(parse_codex_usage_output("No API calls have been made").is_err());
    }
}
