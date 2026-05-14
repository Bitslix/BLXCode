use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_session_exists, git_branch, is_tauri_shell, pty_drain, pty_kill, pty_resize,
    pty_spawn_with_env, pty_write, workbench_drop_sessions, workbench_load_sessions,
    workbench_sessions_path,
};
use crate::workbench::terminal_glue::{
    terminal_api_ready, terminal_create, terminal_dispose, terminal_fit,
    terminal_set_stdin_enabled, terminal_show_fallback, terminal_size_from_js, terminal_write_b64,
};
use gloo_timers::future::TimeoutFuture;
use leptos::callback::{Callable, Callback};
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[derive(Clone, Default)]
struct CellState {
    started: bool,
    disposed: bool,
    term_id: Option<f64>,
    pty_session: Option<u64>,
}

#[component]
pub fn WorkspaceTerminalCell(
    cwd: String,
    grid_index: usize,
    agent_slug: String,
    title: String,
    /// Stable identifier of this terminal slot across restarts. Used as
    /// the lookup key into `sessions.json` (populated by the agent
    /// SessionStart hooks) so we can resume the precise prior agent
    /// session for this cell.
    terminal_key: String,
    is_full_size: Signal<bool>,
    on_full_size: Callback<(), ()>,
    on_split_vertical: Callback<(), ()>,
    on_split_horizontal: Callback<(), ()>,
    on_close: Callback<(), ()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let load_failed = RwSignal::new(false);
    let active = RwSignal::new(false);
    let node_ref = NodeRef::<html::Div>::new();
    let state: Arc<Mutex<CellState>> = Arc::new(Mutex::new(CellState::default()));
    let initial_pty_output_seen = Arc::new(AtomicBool::new(false));
    let branch = RwSignal::new(None::<String>);
    let initial_title = title.clone();
    let dynamic_title = RwSignal::new(initial_title);

    if is_tauri_shell() {
        let cwd_for_branch = cwd.clone();
        leptos::task::spawn_local(async move {
            if let Ok(Some(name)) = git_branch(cwd_for_branch).await {
                branch.set(Some(name));
            }
        });
    }

    let agent_slug_memo = agent_slug.clone();
    let agent_label = Memo::new({
        let i18n = i18n;
        move |_| {
            let _track = i18n.locale().get();
            let s = agent_slug_memo.trim();
            if s.is_empty() {
                return String::new();
            }
            match s {
                "claude" => i18n.tr(I18nKey::WzAgentClaude)().to_string(),
                "codex" => i18n.tr(I18nKey::WzAgentCodex)().to_string(),
                "gemini" => i18n.tr(I18nKey::WzAgentGemini)().to_string(),
                "opencode" => i18n.tr(I18nKey::WzAgentOpencode)().to_string(),
                "cursor" => i18n.tr(I18nKey::WzAgentCursor)().to_string(),
                _ => s.to_string(),
            }
        }
    });

    Effect::new({
        let cwd = cwd.clone();
        let agent_slug = agent_slug.clone();
        let i18n = i18n.clone();
        let state = state.clone();
        let node_ref = node_ref.clone();
        let load_failed = load_failed.clone();
        let initial_pty_output_seen = initial_pty_output_seen.clone();
        let terminal_key = terminal_key.clone();
        move |_| {
            let Some(el) = node_ref.get() else {
                return;
            };
            let Ok(container) = el.dyn_into::<HtmlElement>() else {
                return;
            };
            {
                let mut s = state.lock().expect("cell state");
                if s.started {
                    return;
                }
                s.started = true;
            }

            leptos::task::spawn_local({
                let cwd = cwd.clone();
                let agent_slug = agent_slug.clone();
                let i18n = i18n.clone();
                let state = state.clone();
                let load_failed = load_failed.clone();
                let initial_pty_output_seen = initial_pty_output_seen.clone();
                let terminal_key = terminal_key.clone();
                async move {
                    // Wait for xterm.js to load (up to 6 s)
                    for _ in 0..120u32 {
                        if terminal_api_ready() {
                            break;
                        }
                        TimeoutFuture::new(50).await;
                    }
                    if !terminal_api_ready() {
                        load_failed.set(true);
                        return;
                    }
                    let tid = match terminal_create(&container) {
                        Ok(v) => v,
                        Err(_) => {
                            load_failed.set(true);
                            return;
                        }
                    };
                    state.lock().expect("cell").term_id = Some(tid);

                    // Allow one browser frame so CSS layout is settled before fit()
                    TimeoutFuture::new(50).await;
                    let initial_size = terminal_fit(tid);

                    let pty_sid = if is_tauri_shell() {
                        // Phase 2 resume plumbing: inject env so the agent's
                        // SessionStart hook can record session_id -> this
                        // terminal slot. The sessions.json path is also
                        // exposed via env (cross-platform path discovery).
                        let sessions_path = workbench_sessions_path().await.ok();
                        let mut env: Vec<(String, String)> = Vec::new();
                        env.push(("BLX_TERMINAL_KEY".into(), terminal_key.clone()));
                        if !agent_slug.trim().is_empty() {
                            env.push(("BLX_AGENT_SLUG".into(), agent_slug.trim().into()));
                        }
                        if let Some(p) = sessions_path.as_ref() {
                            env.push(("BLX_SESSIONS_PATH".into(), p.clone()));
                        }
                        match pty_spawn_with_env(cwd.clone(), env).await {
                            Ok(sid) => {
                                terminal_set_stdin_enabled(tid, true);
                                state.lock().expect("cell").pty_session = Some(sid);
                                if let Some(size) = initial_size {
                                    let _ = pty_resize(sid, size.rows, size.cols).await;
                                }

                                let state2 = state.clone();
                                let i18n2 = i18n.clone();
                                let initial_pty_output_seen2 = initial_pty_output_seen.clone();
                                leptos::task::spawn_local(async move {
                                    loop {
                                        TimeoutFuture::new(35).await;
                                        if state2.lock().expect("cell").disposed {
                                            break;
                                        }
                                        match pty_drain(sid, 65536).await {
                                            Ok(b64) if !b64.is_empty() => {
                                                initial_pty_output_seen2
                                                    .store(true, Ordering::Relaxed);
                                                if let Some(t) =
                                                    state2.lock().expect("cell").term_id
                                                {
                                                    terminal_write_b64(t, &b64);
                                                }
                                            }
                                            Ok(_) => {}
                                            Err(err) => {
                                                if let Some(t) =
                                                    state2.lock().expect("cell").term_id
                                                {
                                                    let msg = format!(
                                                        "{}\n{}",
                                                        i18n2.tr(I18nKey::WsPtySpawnFailed)(),
                                                        err
                                                    );
                                                    terminal_show_fallback(t, &msg);
                                                }
                                                break;
                                            }
                                        }
                                    }
                                });

                                // Auto-launch agent command after shell init.
                                // If sessions.json (written by the SessionStart
                                // hook on a previous run) has a session_id for
                                // this terminal_key, resume it; otherwise start
                                // a fresh session.
                                let slug = agent_slug.trim().to_string();
                                if !slug.is_empty() {
                                    let resume_id =
                                        lookup_resume_session(&terminal_key, &slug, &cwd).await;
                                    for _ in 0..30u8 {
                                        if initial_pty_output_seen.load(Ordering::Relaxed) {
                                            break;
                                        }
                                        TimeoutFuture::new(25).await;
                                    }
                                    TimeoutFuture::new(50).await;
                                    let cmd = build_launch_command(&slug, resume_id.as_deref());
                                    use base64::Engine;
                                    let b64 = base64::engine::general_purpose::STANDARD
                                        .encode(cmd.as_bytes());
                                    let _ = pty_write(sid, b64).await;
                                }
                                Some(sid)
                            }
                            Err(err) => {
                                let msg = format!(
                                    "{}\n{}\n{}",
                                    i18n.tr(I18nKey::WsPtySpawnFailed)(),
                                    i18n.tr(I18nKey::WsPtyNoDesktop)(),
                                    err
                                );
                                terminal_show_fallback(tid, &msg);
                                None
                            }
                        }
                    } else {
                        terminal_show_fallback(tid, i18n.tr(I18nKey::WsPtyNoDesktop)());
                        None
                    };
                    if pty_sid.is_none() {
                        state.lock().expect("cell").pty_session = None;
                    }
                }
            });
        }
    });

    // Bug fix: create handles BEFORE on_cleanup, then move them INTO the cleanup closure
    // so they live for the full component lifetime (not dropped at end of component fn).
    let pty_input_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("blxcode-pty-input", {
            let state = state.clone();
            let i18n = i18n.clone();
            move |ev| {
                let (term_id, sid) = {
                    let st = state.lock().expect("cell");
                    (st.term_id, st.pty_session)
                };
                let Some(term_id) = term_id else {
                    return;
                };
                let Some(sid) = sid else {
                    return;
                };
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let term_js =
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("termId"))
                        .ok()
                        .and_then(|v| v.as_f64());
                if term_js != Some(term_id) {
                    return;
                }
                let data = js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("data"))
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();
                if data.is_empty() {
                    return;
                }
                leptos::task::spawn_local({
                    let data = data.into_bytes();
                    let i18n = i18n.clone();
                    let state = state.clone();
                    async move {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                        if let Err(err) = pty_write(sid, b64).await {
                            if let Some(t) = state.lock().expect("cell").term_id {
                                let msg =
                                    format!("{}\n{}", i18n.tr(I18nKey::WsPtySpawnFailed)(), err);
                                terminal_show_fallback(t, &msg);
                            }
                        }
                    }
                });
            }
        });

    let pty_title_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("blxcode-pty-title", {
            let state = state.clone();
            move |ev| {
                let term_id = state.lock().expect("cell").term_id;
                let Some(term_id) = term_id else {
                    return;
                };
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let term_js =
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("termId"))
                        .ok()
                        .and_then(|v| v.as_f64());
                if term_js != Some(term_id) {
                    return;
                }
                let new_title =
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("title"))
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default();
                let trimmed = new_title.trim();
                if !trimmed.is_empty() {
                    dynamic_title.set(trimmed.to_string());
                }
            }
        });

    let pty_resize_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("blxcode-pty-resize", {
            let state = state.clone();
            move |ev| {
                let (term_id, sid) = {
                    let st = state.lock().expect("cell");
                    (st.term_id, st.pty_session)
                };
                let Some(term_id) = term_id else {
                    return;
                };
                let Some(sid) = sid else {
                    return;
                };
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let term_js =
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("termId"))
                        .ok()
                        .and_then(|v| v.as_f64());
                if term_js != Some(term_id) {
                    return;
                }
                if let Some(size) = terminal_size_from_js(&detail) {
                    leptos::task::spawn_local(async move {
                        let _ = pty_resize(sid, size.rows, size.cols).await;
                    });
                }
            }
        });

    let resize_handle = leptos::leptos_dom::helpers::window_event_listener_untyped("resize", {
        let state = state.clone();
        move |_| {
            let term_id = state.lock().expect("cell").term_id;
            let sid = state.lock().expect("cell").pty_session;
            if let (Some(t), Some(sid)) = (term_id, sid) {
                if let Some(size) = terminal_fit(t) {
                    leptos::task::spawn_local(async move {
                        let _ = pty_resize(sid, size.rows, size.cols).await;
                    });
                }
            }
        }
    });

    on_cleanup({
        let state = state.clone();
        // Move handles into cleanup so they live until component unmount
        move || {
            drop(pty_input_handle);
            drop(pty_title_handle);
            drop(pty_resize_handle);
            drop(resize_handle);
            if let Ok(mut st) = state.lock() {
                st.disposed = true;
            }
            let (t, sid) = {
                let st = state.lock().expect("cell");
                (st.term_id, st.pty_session)
            };
            if let Some(t) = t {
                terminal_dispose(t);
            }
            if let Some(sid) = sid {
                leptos::task::spawn_local(async move {
                    let _ = pty_kill(sid).await;
                });
            }
        }
    });

    view! {
        <div
            class=move || {
                let mut class = String::from("ws-term-cell");
                if active.get() {
                    class.push_str(" ws-term-cell--active");
                }
                class
            }
            role="region"
            aria-label=move || format!("{} {}", i18n.tr(I18nKey::WsTermSlot)(), grid_index + 1)
            on:mousedown=move |_| active.set(true)
            on:focusin=move |_| active.set(true)
            on:focusout=move |_| active.set(false)
        >
            <div class="ws-term-cell__head">
                <span class="ws-term-cell__title">{move || dynamic_title.get()}</span>
                <Show when=move || branch.with(|b| b.is_some())>
                    <span class="ws-term-cell__branch">
                        <LxIcon icon=icondata::LuGitBranch width="0.72rem" height="0.72rem" />
                        <span>{move || branch.get().unwrap_or_default()}</span>
                    </span>
                </Show>
                <Show when=move || !agent_label.get().is_empty()>
                    <span class="ws-term-cell__badge">{move || agent_label.get()}</span>
                </Show>
                <button
                    type="button"
                    class="ws-term-cell__tool"
                    title=move || {
                        if is_full_size.get() { "Restore size" } else { "Full size" }
                    }
                    aria-label=move || {
                        if is_full_size.get() { "Restore terminal size" } else { "Full size terminal" }
                    }
                    on:click=move |_| on_full_size.run(())
                >
                    {move || {
                        if is_full_size.get() {
                            view! { <LxIcon icon=icondata::LuMinimize2 width="0.78rem" height="0.78rem" /> }.into_any()
                        } else {
                            view! { <LxIcon icon=icondata::LuMaximize2 width="0.78rem" height="0.78rem" /> }.into_any()
                        }
                    }}
                </button>
                <button
                    type="button"
                    class="ws-term-cell__tool"
                    title="Split vertical"
                    aria-label="Split terminal vertical"
                    on:click=move |_| on_split_vertical.run(())
                >
                    <LxIcon icon=icondata::LuPanelRight width="0.82rem" height="0.82rem" />
                </button>
                <button
                    type="button"
                    class="ws-term-cell__tool"
                    title="Split horizontal"
                    aria-label="Split terminal horizontal"
                    on:click=move |_| on_split_horizontal.run(())
                >
                    <LxIcon icon=icondata::LuPanelBottom width="0.82rem" height="0.82rem" />
                </button>
                <button
                    type="button"
                    class="ws-term-cell__tool ws-term-cell__tool--danger"
                    title="Close"
                    aria-label="Close terminal"
                    on:click=move |_| on_close.run(())
                >
                    <LxIcon icon=icondata::LuX width="0.86rem" height="0.86rem" />
                </button>
            </div>
            <Show when=move || load_failed.get()>
                <p class="ws-term-cell__boot-fail">{move || i18n.tr(I18nKey::WsTermBootstrapFailed)()}</p>
            </Show>
            <div class="ws-term-cell__xterm" node_ref=node_ref></div>
        </div>
    }
}

/// Consult `sessions.json` for a prior session id matching this terminal
/// slot. Returns `None` on any error / mismatch / missing entry — the
/// caller falls back to a fresh launch.
///
/// Stale entries (a captured session_id whose on-disk transcript no
/// longer exists, e.g. an empty session that Claude never wrote) are
/// dropped from `sessions.json` so we stop trying to resume them on
/// every restart.
async fn lookup_resume_session(
    terminal_key: &str,
    agent_slug: &str,
    cwd: &str,
) -> Option<String> {
    let raw = match workbench_load_sessions().await {
        Ok(Some(s)) => s,
        Ok(None) => {
            web_sys::console::log_1(
                &format!("[blxcode resume] {terminal_key}: no sessions.json").into(),
            );
            return None;
        }
        Err(e) => {
            web_sys::console::log_1(
                &format!("[blxcode resume] {terminal_key}: load err {e}").into(),
            );
            return None;
        }
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            web_sys::console::log_1(
                &format!("[blxcode resume] {terminal_key}: parse err {e}").into(),
            );
            return None;
        }
    };
    let Some(entry) = parsed.get("terminals").and_then(|t| t.get(terminal_key)) else {
        let keys: Vec<String> = parsed
            .get("terminals")
            .and_then(|t| t.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        web_sys::console::log_1(
            &format!(
                "[blxcode resume] {terminal_key}: no entry; have {keys:?}"
            )
            .into(),
        );
        return None;
    };
    let stored_agent = entry.get("agent").and_then(|v| v.as_str()).unwrap_or("");
    if stored_agent != agent_slug {
        web_sys::console::log_1(
            &format!(
                "[blxcode resume] {terminal_key}: agent mismatch stored={stored_agent} slot={agent_slug}"
            )
            .into(),
        );
        return None;
    }
    let id = entry
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    // Validate against on-disk transcript. Captured-but-never-used
    // sessions don't have a JSONL file, so `claude --resume <id>` would
    // bail with "No conversation found with session ID …". Drop those.
    match agent_session_exists(agent_slug.to_string(), cwd.to_string(), id.clone()).await {
        Ok(true) => {
            web_sys::console::log_1(
                &format!("[blxcode resume] {terminal_key}: resume_id={id} (validated)").into(),
            );
            Some(id)
        }
        Ok(false) => {
            web_sys::console::log_1(
                &format!(
                    "[blxcode resume] {terminal_key}: session {id} has no transcript on disk — dropping"
                )
                .into(),
            );
            let key = terminal_key.to_string();
            leptos::task::spawn_local(async move {
                let _ = workbench_drop_sessions(key).await;
            });
            None
        }
        Err(e) => {
            web_sys::console::log_1(
                &format!("[blxcode resume] {terminal_key}: validate err {e}").into(),
            );
            None
        }
    }
}

/// Format the shell command that auto-launches the agent CLI. With a
/// resume id we use the CLI's resume syntax (Claude: `--resume <id>`,
/// Codex: `resume <id>`); without one we just run the binary.
fn build_launch_command(slug: &str, resume_id: Option<&str>) -> String {
    match (slug, resume_id) {
        ("claude", Some(id)) => format!("claude --resume {id}\r"),
        ("codex", Some(id)) => format!("codex resume {id}\r"),
        ("gemini", Some(id)) => format!("gemini --resume {id}\r"),
        ("opencode", Some(id)) => format!("opencode --session {id}\r"),
        // Cursor ships as the `cursor-agent` binary; `--resume <chatId>`
        // re-attaches the prior conversation.
        ("cursor", Some(id)) => format!("cursor-agent --resume {id}\r"),
        ("cursor", None) => "cursor-agent\r".to_string(),
        (other, _) => format!("{other}\r"),
    }
}
