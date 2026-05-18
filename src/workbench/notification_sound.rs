//! Short beep when agent tasks complete in a background terminal.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::tauri_bridge::TerminalNotification;
use crate::workbench::agent_accent::terminal_key_storage_key;
use crate::workbench::WorkbenchService;
use leptos::prelude::GetUntracked;

const NOTIFY_POLL_MS: u32 = 1000;

/// Play a brief sine beep (~150 ms) via Web Audio in the page context.
pub fn play_notification_beep() {
    let _ = js_sys::eval(
        r#"(() => {
  try {
    const Ctx = window.AudioContext || window.webkitAudioContext;
    if (!Ctx) return;
    const ctx = new Ctx();
    const o = ctx.createOscillator();
    o.frequency.value = 880;
    o.type = "sine";
    const g = ctx.createGain();
    g.gain.value = 0.08;
    o.connect(g);
    g.connect(ctx.destination);
    const t = ctx.currentTime;
    g.gain.setValueAtTime(0.08, t);
    g.gain.exponentialRampToValueAtTime(0.001, t + 0.15);
    o.start(t);
    o.stop(t + 0.16);
  } catch (_) {}
})()"#,
    );
}

/// Returns true when any terminal gained unread while not in the user's focus.
pub fn maybe_play_for_notification_delta(
    wb: WorkbenchService,
    prev_seen: &mut HashMap<String, String>,
    loaded: &HashMap<String, TerminalNotification>,
) {
    let active_storage_key = wb.active_workspace_storage_key();
    let focused_map = wb.focused_terminal_by_workspace().get_untracked();

    for (key, note) in loaded {
        if note.unread == 0 {
            continue;
        }
        let stamp = note.updated_at.as_deref().unwrap_or("");
        if prev_seen.get(key).map(String::as_str) == Some(stamp) {
            continue;
        }
        prev_seen.insert(key.clone(), stamp.to_string());

        let storage_key = terminal_key_storage_key(key);
        let ws_active = storage_key.as_deref() == active_storage_key.as_deref();
        let term_focused = storage_key
            .as_ref()
            .and_then(|sk| focused_map.get(sk))
            .map(|focused| focused == key)
            .unwrap_or(false);
        if ws_active && term_focused {
            continue;
        }
        play_notification_beep();
        return;
    }
}

/// Start polling `notifications.json` and updating workbench signals.
pub fn spawn_notification_poller(wb: WorkbenchService) {
    use crate::tauri_bridge::{
        is_tauri_shell, workbench_clear_terminal_notifications, workbench_load_notifications,
    };
    use gloo_timers::future::TimeoutFuture;
    use leptos::task::spawn_local;

    if !is_tauri_shell() {
        return;
    }

    spawn_local(async move {
        let prev_seen = Rc::new(RefCell::new(HashMap::<String, String>::new()));
        let mut first_run = true;
        loop {
            TimeoutFuture::new(NOTIFY_POLL_MS).await;
            match workbench_load_notifications().await {
                Ok(map) => {
                    if first_run {
                        // Seed `prev_seen` with everything already on disk —
                        // notifications from a previous app session must not
                        // beep, badge, or pulse titlebars on startup. External
                        // agent terminals are owned by this app process, so any
                        // unread values present before the first post-hydration
                        // poll are stale from the last run.
                        let mut prev = prev_seen.borrow_mut();
                        for (key, note) in &map {
                            prev.insert(key.clone(), note.updated_at.clone().unwrap_or_default());
                        }
                        first_run = false;
                        wb.set_notifications(HashMap::new());
                        let live_keys: HashSet<String> =
                            wb.live_terminal_keys().into_iter().collect();
                        for key in map.keys().filter(|key| live_keys.contains(*key)).cloned() {
                            spawn_local(async move {
                                let _ = workbench_clear_terminal_notifications(key).await;
                            });
                        }
                    } else {
                        let counts: HashMap<String, u32> =
                            map.iter().map(|(k, v)| (k.clone(), v.unread)).collect();
                        wb.set_notifications(counts);
                        maybe_play_for_notification_delta(wb, &mut prev_seen.borrow_mut(), &map);
                    }
                }
                Err(_) => {}
            }
        }
    });
}
