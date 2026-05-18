use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_session_exists, git_branch, is_tauri_shell, pty_drain_wait, pty_kill, pty_resize,
    pty_spawn_with_env, pty_write, workbench_drop_sessions, workbench_load_sessions,
    workbench_sessions_path,
};
use crate::workbench::terminal_glue::{
    terminal_create, terminal_dispose, terminal_fit, terminal_request_fit,
    terminal_set_stdin_enabled, terminal_show_fallback, terminal_size_from_js, TerminalSize,
    terminal_wait_api_ready, terminal_write_b64,
};
use gloo_timers::future::TimeoutFuture;
use leptos::callback::{Callable, Callback};
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[derive(Clone)]
struct AgentLaunchPending {
    slug: String,
    cwd: String,
    terminal_key: String,
    sid: u64,
}

#[derive(Clone, Default)]
struct CellState {
    /// xterm bootstrap finished successfully.
    bootstrapped: bool,
    /// Bootstrap async task is running.
    bootstrap_inflight: bool,
    disposed: bool,
    launch_sent: bool,
    /// True while [`spawn_agent_launch_when_ready`] is already running.
    agent_launch_inflight: bool,
    term_id: Option<f64>,
    pty_session: Option<u64>,
    /// Agent auto-launch waits here until xterm+PTY have a real size.
    agent_launch: Option<AgentLaunchPending>,
    /// Most recent terminal size reported via `blxcode-pty-resize` before
    /// the PTY existed. Flushed once `pty_session` is set so the very
    /// first agent invocation gets a correct TTY size instead of the
    /// default 80×24. Without this, the early `requestFit`/setTimeout
    /// resize events from `terminal_bootstrap.mjs::create()` are lost
    /// whenever `pty_spawn_with_env().await` hasn't resolved yet.
    pending_resize: Option<(u16, u16)>,
}

#[component]
pub fn WorkspaceTerminalCell(
    workspace_id: u64,
    cwd: String,
    grid_index: usize,
    agent_slug: String,
    title: String,
    /// Stable identifier of this terminal slot across restarts. Used as
    /// the lookup key into `sessions.json` (populated by the agent
    /// SessionStart hooks) so we can resume the precise prior agent
    /// session for this cell.
    terminal_key: String,
    is_workspace_active: Signal<bool>,
    is_slot_hidden: Signal<bool>,
    is_full_size: Signal<bool>,
    on_full_size: Callback<(), ()>,
    on_split_vertical: Callback<(), ()>,
    on_split_horizontal: Callback<(), ()>,
    on_close: Callback<(), ()>,
    /// Hides the close (`×`) button when removing this cell is a no-op
    /// (e.g. last remaining terminal in the workspace with a single pane).
    can_close: Signal<bool>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<crate::workbench::state::WorkbenchService>();
    let load_failed = RwSignal::new(false);
    let active = RwSignal::new(false);
    let node_ref = NodeRef::<html::Div>::new();
    let state: Arc<Mutex<CellState>> = Arc::new(Mutex::new(CellState::default()));
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

    // `NodeRef::on_load` can miss the first mount when the grid appears via
    // `<Show>` after wizard commit; track the ref in an Effect instead.
    Effect::new({
        let node_ref = node_ref;
        let cwd = cwd.clone();
        let agent_slug = agent_slug.clone();
        let i18n = i18n.clone();
        let state = state.clone();
        let load_failed = load_failed.clone();
        let terminal_key = terminal_key.clone();
        let wb = wb;
        move |_| {
            let _ = wb.terminal_layout_tick().get();
            if !(is_workspace_active.get() && !is_slot_hidden.get()) {
                return;
            }
            let Some(el) = node_ref.get() else {
                return;
            };
            let Ok(container) = el.dyn_into::<HtmlElement>() else {
                load_failed.set(true);
                return;
            };
            let start = {
                let mut s = state.lock().expect("cell state");
                if s.bootstrapped || s.bootstrap_inflight || s.disposed {
                    false
                } else {
                    s.bootstrap_inflight = true;
                    true
                }
            };
            if !start {
                return;
            }
            leptos::task::spawn_local(bootstrap_terminal_cell(
                container,
                cwd.clone(),
                agent_slug.clone(),
                i18n.clone(),
                state.clone(),
                load_failed.clone(),
                terminal_key.clone(),
                wb,
            ));
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
                let Some(size) = terminal_size_from_js(&detail) else {
                    return;
                };
                // PTY not yet spawned (very first terminal in a fresh
                // workspace): buffer the latest size so it can be
                // applied right after the spawn resolves.
                let Some(sid) = sid else {
                    state.lock().expect("cell").pending_resize = Some((size.rows, size.cols));
                    return;
                };
                leptos::task::spawn_local(async move {
                    let _ = pty_resize(sid, size.rows, size.cols).await;
                });
                schedule_agent_launch_retries(state.clone());
            }
        });

    let grid_ready_handle = leptos::leptos_dom::helpers::window_event_listener_untyped(
        "blxcode-ws-term-grid-ready",
        {
            let state = state.clone();
            let agent_slug = agent_slug.clone();
            move |ev| {
                if agent_slug.trim().is_empty() {
                    return;
                }
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let ws_js = js_sys::Reflect::get(
                    &detail,
                    &wasm_bindgen::JsValue::from_str("workspaceId"),
                )
                .ok()
                .and_then(|v| v.as_f64());
                if ws_js != Some(workspace_id as f64) {
                    return;
                }
                schedule_agent_launch_retries(state.clone());
                spawn_terminal_refit(state.clone(), 32, 32);
            }
        },
    );

    let resize_handle = leptos::leptos_dom::helpers::window_event_listener_untyped("resize", {
        let state = state.clone();
        move |_| {
            spawn_terminal_refit(state.clone(), 8, 32);
        }
    });

    Effect::new({
        let state = state.clone();
        let agent_slug = agent_slug.clone();
        move |_| {
            let visible = is_workspace_active.get() && !is_slot_hidden.get();
            let _ = wb.sidebar_collapsed().get();
            let _ = wb.right_collapsed().get();
            let _ = wb.right_width_px().get();
            let _ = wb.terminal_layout_tick().get();
            let _ = is_full_size.get();
            if visible {
                spawn_terminal_xterm_fit(state.clone(), 240, 50);
                spawn_terminal_refit(state.clone(), 240, 50);
                if !agent_slug.trim().is_empty() {
                    schedule_agent_launch_retries(state.clone());
                }
            }
        }
    });

    on_cleanup({
        let state = state.clone();
        let terminal_key_cleanup = terminal_key.clone();
        // Move handles into cleanup so they live until component unmount
        move || {
            drop(pty_input_handle);
            drop(pty_title_handle);
            drop(pty_resize_handle);
            drop(grid_ready_handle);
            drop(resize_handle);
            if let Ok(mut st) = state.lock() {
                st.disposed = true;
            }
            let (t, sid) = {
                let st = state.lock().expect("cell");
                (st.term_id, st.pty_session)
            };
            wb.unregister_pty_session(&terminal_key_cleanup);
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
                        if is_full_size.get() {
                            i18n.tr(I18nKey::WsTermRestoreSize)()
                        } else {
                            i18n.tr(I18nKey::WsTermFullSize)()
                        }
                    }
                    aria-label=move || {
                        if is_full_size.get() {
                            i18n.tr(I18nKey::WsTermRestoreSize)()
                        } else {
                            i18n.tr(I18nKey::WsTermFullSize)()
                        }
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
                    title=move || i18n.tr(I18nKey::WsTermSplitVerticalAria)()
                    aria-label=move || i18n.tr(I18nKey::WsTermSplitVerticalAria)()
                    on:click=move |_| on_split_vertical.run(())
                >
                    <LxIcon icon=icondata::LuPanelRight width="0.82rem" height="0.82rem" />
                </button>
                <button
                    type="button"
                    class="ws-term-cell__tool"
                    title=move || i18n.tr(I18nKey::WsTermSplitHorizontalAria)()
                    aria-label=move || i18n.tr(I18nKey::WsTermSplitHorizontalAria)()
                    on:click=move |_| on_split_horizontal.run(())
                >
                    <LxIcon icon=icondata::LuPanelBottom width="0.82rem" height="0.82rem" />
                </button>
                <Show when=move || can_close.get()>
                    <button
                        type="button"
                        class="ws-term-cell__tool ws-term-cell__tool--danger"
                        title=move || i18n.tr(I18nKey::BtnClose)()
                        aria-label=move || i18n.tr(I18nKey::WsTermCloseAria)()
                        on:click=move |_| on_close.run(())
                    >
                        <LxIcon icon=icondata::LuX width="0.86rem" height="0.86rem" />
                    </button>
                </Show>
            </div>
            <Show when=move || load_failed.get()>
                <p class="ws-term-cell__boot-fail">{move || i18n.tr(I18nKey::WsTermBootstrapFailed)()}</p>
            </Show>
            <div class="ws-term-cell__xterm" node_ref=node_ref></div>
        </div>
    }
}

async fn wait_for_container_ready(container: &HtmlElement, attempts: u32, delay_ms: u32) -> bool {
    for _ in 0..attempts {
        let rect = container.get_bounding_client_rect();
        if rect.width() > 1.0 && rect.height() > 1.0 {
            return true;
        }
        TimeoutFuture::new(delay_ms).await;
    }
    false
}

async fn bootstrap_terminal_cell(
    container: HtmlElement,
    cwd: String,
    agent_slug: String,
    i18n: I18nService,
    state: Arc<Mutex<CellState>>,
    load_failed: RwSignal<bool>,
    terminal_key: String,
    wb: crate::workbench::state::WorkbenchService,
) {
    struct BootstrapGuard(Arc<Mutex<CellState>>);
    impl Drop for BootstrapGuard {
        fn drop(&mut self) {
            if let Ok(mut st) = self.0.lock() {
                st.bootstrap_inflight = false;
            }
        }
    }
    let _bootstrap_guard = BootstrapGuard(state.clone());

    if !terminal_wait_api_ready().await {
        load_failed.set(true);
        return;
    }
    // Keep trying xterm even if the grid is still settling; agent launch retries
    // until a real resize arrives.
    let _ = wait_for_container_ready(&container, 120, 16).await;
    let tid = match terminal_create(&container) {
        Ok(v) => v,
        Err(_) => {
            load_failed.set(true);
            return;
        }
    };
    {
        let mut st = state.lock().expect("cell");
        st.term_id = Some(tid);
        st.bootstrapped = true;
    }

    spawn_terminal_xterm_fit(state.clone(), 48, 16);
    let initial_size = terminal_request_fit(tid).or_else(|| terminal_fit(tid));

    let pty_sid = if is_tauri_shell() {
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
                let pending = {
                    let mut st = state.lock().expect("cell");
                    st.pty_session = Some(sid);
                    st.pending_resize.take()
                };
                wb.register_pty_session(terminal_key.clone(), sid);
                let resize_target = pending
                    .map(|(rows, cols)| (rows, cols))
                    .or_else(|| initial_size.map(|s| (s.rows, s.cols)));
                if let Some((rows, cols)) = resize_target {
                    let _ = pty_resize(sid, rows, cols).await;
                }

                let state2 = state.clone();
                let i18n2 = i18n.clone();
                leptos::task::spawn_local(async move {
                    loop {
                        if state2.lock().expect("cell").disposed {
                            break;
                        }
                        match pty_drain_wait(sid, 65536, 250).await {
                            Ok(b64) if !b64.is_empty() => {
                                if let Some(t) = state2.lock().expect("cell").term_id {
                                    terminal_write_b64(t, &b64);
                                }
                            }
                            Ok(_) => {}
                            Err(err) => {
                                if let Some(t) = state2.lock().expect("cell").term_id {
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
                spawn_terminal_refit(state.clone(), 240, 50);

                let slug = agent_slug.trim().to_string();
                if !slug.is_empty() {
                    state.lock().expect("cell").agent_launch = Some(AgentLaunchPending {
                        slug,
                        cwd: cwd.clone(),
                        terminal_key: terminal_key.clone(),
                        sid,
                    });
                    schedule_agent_launch_retries(state.clone());
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

/// Retry agent auto-launch across layout ticks (grid mount, resize, sidebar).
fn schedule_agent_launch_retries(state: Arc<Mutex<CellState>>) {
    leptos::task::spawn_local(async move {
        for delay in [0_u32, 80, 200, 500, 1200, 2500] {
            if delay > 0 {
                TimeoutFuture::new(delay).await;
            }
            if state.lock().expect("cell").disposed {
                return;
            }
            if state.lock().expect("cell").launch_sent {
                return;
            }
            request_agent_launch(state.clone());
        }
    });
}

fn request_agent_launch(state: Arc<Mutex<CellState>>) {
    let start = {
        let mut st = state.lock().expect("cell");
        if st.disposed || st.launch_sent || st.agent_launch.is_none() || st.agent_launch_inflight {
            false
        } else {
            st.agent_launch_inflight = true;
            true
        }
    };
    if !start {
        return;
    }
    leptos::task::spawn_local(async move {
        spawn_agent_launch_when_ready(state).await;
    });
}

/// Auto-launch the agent CLI only after xterm+PTY have non-zero dimensions.
/// Plain shells tolerate the default 80×24; TUIs stay black until a real
/// resize — which is why agent slots looked "stuck" until sidebar reflow.
async fn spawn_agent_launch_when_ready(state: Arc<Mutex<CellState>>) {
    struct InflightGuard(Arc<Mutex<CellState>>);
    impl Drop for InflightGuard {
        fn drop(&mut self) {
            if let Ok(mut st) = self.0.lock() {
                st.agent_launch_inflight = false;
            }
        }
    }
    let _guard = InflightGuard(state.clone());

    if state.lock().expect("cell").disposed {
        return;
    }
    let pending = state.lock().expect("cell").agent_launch.clone();
    let Some(pending) = pending else {
        return;
    };
    if state.lock().expect("cell").launch_sent {
        return;
    }

    // Brief shell settle after PTY spawn.
    TimeoutFuture::new(200).await;

    const MAX_ROUNDS: u32 = 60;
    let mut sized = false;
    for round in 0..MAX_ROUNDS {
        if state.lock().expect("cell").disposed {
            return;
        }
        if state.lock().expect("cell").launch_sent {
            return;
        }

        if apply_buffered_or_fitted_size(&state, pending.sid).await {
            sized = true;
            break;
        }
        if refit_pty_until_ready(state.clone(), 6, 24).await {
            sized = true;
            break;
        }
        if round + 1 >= MAX_ROUNDS {
            break;
        }
        TimeoutFuture::new(50).await;
    }

    if !sized {
        let _ = pty_resize(pending.sid, 24, 80).await;
    }

    let resume_id =
        lookup_resume_session(&pending.terminal_key, &pending.slug, &pending.cwd).await;
    if state.lock().expect("cell").launch_sent || state.lock().expect("cell").disposed {
        return;
    }
    let cmd = build_launch_command(&pending.slug, resume_id.as_deref());
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(cmd.as_bytes());
    if pty_write(pending.sid, b64).await.is_ok() {
        let mut st = state.lock().expect("cell");
        st.launch_sent = true;
        st.agent_launch = None;
    }
    spawn_terminal_refit(state, 32, 32);
}

async fn apply_buffered_or_fitted_size(state: &Arc<Mutex<CellState>>, sid: u64) -> bool {
    let (term_id, pending) = {
        let st = state.lock().expect("cell");
        (st.term_id, st.pending_resize)
    };
    let Some(term_id) = term_id else {
        return false;
    };
    if let Some((rows, cols)) = pending {
        if rows > 0 && cols > 0 {
            let _ = pty_resize(sid, rows, cols).await;
            return true;
        }
    }
    if let Some(size) = terminal_request_fit(term_id).or_else(|| terminal_fit(term_id)) {
        if size.rows > 0 && size.cols > 0 {
            let _ = pty_resize(sid, size.rows, size.cols).await;
            return true;
        }
    }
    false
}

fn terminal_session_pair(state: &Arc<Mutex<CellState>>) -> Option<(f64, u64)> {
    let st = state.lock().expect("cell");
    if st.disposed {
        return None;
    }
    Some((st.term_id?, st.pty_session?))
}

fn spawn_terminal_xterm_fit(state: Arc<Mutex<CellState>>, attempts: u32, delay_ms: u32) {
    leptos::task::spawn_local(async move {
        let _ = refit_xterm_until_ready(state, attempts, delay_ms).await;
    });
}

fn spawn_terminal_refit(state: Arc<Mutex<CellState>>, attempts: u32, delay_ms: u32) {
    leptos::task::spawn_local(async move {
        let _ = refit_pty_until_ready(state, attempts, delay_ms).await;
    });
}

fn terminal_id(state: &Arc<Mutex<CellState>>) -> Option<f64> {
    let st = state.lock().expect("cell");
    if st.disposed {
        return None;
    }
    st.term_id
}

fn valid_terminal_size(size: TerminalSize) -> Option<TerminalSize> {
    if size.rows > 0 && size.cols > 0 {
        Some(size)
    } else {
        None
    }
}

async fn refit_xterm_until_ready(
    state: Arc<Mutex<CellState>>,
    attempts: u32,
    delay_ms: u32,
) -> Option<TerminalSize> {
    let mut requested_fit = false;
    for _ in 0..attempts {
        let Some(term_id) = terminal_id(&state) else {
            TimeoutFuture::new(delay_ms).await;
            continue;
        };
        let size = if requested_fit {
            terminal_fit(term_id)
        } else {
            requested_fit = true;
            terminal_request_fit(term_id).or_else(|| terminal_fit(term_id))
        };
        if let Some(size) = size.and_then(valid_terminal_size) {
            return Some(size);
        }
        TimeoutFuture::new(delay_ms).await;
    }
    None
}

async fn refit_pty_until_ready(state: Arc<Mutex<CellState>>, attempts: u32, delay_ms: u32) -> bool {
    let mut requested_fit = false;
    for _ in 0..attempts {
        let Some((term_id, sid)) = terminal_session_pair(&state) else {
            TimeoutFuture::new(delay_ms).await;
            continue;
        };
        let size = if requested_fit {
            terminal_fit(term_id)
        } else {
            requested_fit = true;
            terminal_request_fit(term_id).or_else(|| terminal_fit(term_id))
        };
        if let Some(size) = size.and_then(valid_terminal_size) {
            let _ = pty_resize(sid, size.rows, size.cols).await;
            return true;
        }
        TimeoutFuture::new(delay_ms).await;
    }
    false
}

/// Consult `sessions.json` for a prior session id matching this terminal
/// slot. Returns `None` on any error / mismatch / missing entry — the
/// caller falls back to a fresh launch.
///
/// Stale entries (a captured session_id whose on-disk transcript no
/// longer exists, e.g. an empty session that Claude never wrote) are
/// dropped from `sessions.json` so we stop trying to resume them on
/// every restart.
async fn lookup_resume_session(terminal_key: &str, agent_slug: &str, cwd: &str) -> Option<String> {
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
            &format!("[blxcode resume] {terminal_key}: no entry; have {keys:?}").into(),
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

/// Single-quote for POSIX shells: `'` → `'"'"''`.
fn shell_single_quoted_arg(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 8192 || t.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(format!("'{}'", t.replace('\'', "'\"'\"'")))
}

/// Format the shell command that auto-launches the agent CLI. With a
/// resume id we use the CLI's resume syntax (Claude: `--resume <id>`,
/// Codex: `resume <id>`); without one we just run the binary.
fn build_launch_command(slug: &str, resume_id: Option<&str>) -> String {
    if let Some(raw) = resume_id {
        if let Some(a) = shell_single_quoted_arg(raw) {
            return match slug {
                "claude" => format!("claude --resume {a}\r"),
                "codex" => format!("codex resume {a}\r"),
                "gemini" => format!("gemini --resume {a}\r"),
                "opencode" => format!("opencode --session {a}\r"),
                "cursor" => format!("cursor-agent --resume {a}\r"),
                other => format!("{other}\r"),
            };
        }
    }
    match slug {
        "cursor" => "cursor-agent\r".to_string(),
        other => format!("{other}\r"),
    }
}
