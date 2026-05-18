//! Short beep when agent tasks complete in a background terminal.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::tauri_bridge::TerminalNotification;
use crate::workbench::agent_accent::terminal_key_workspace_id;
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
    let active_ws = wb.active_id().get_untracked();
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

        let ws_id = terminal_key_workspace_id(key);
        let ws_active = ws_id.is_some_and(|id| active_ws == Some(id));
        let term_focused = ws_id.is_some_and(|id| {
            focused_map
                .get(&id)
                .map(|focused| focused == key)
                .unwrap_or(false)
        });
        if ws_active && term_focused {
            continue;
        }
        play_notification_beep();
        return;
    }
}

/// Start polling `notifications.json` and updating workbench signals.
pub fn spawn_notification_poller(wb: WorkbenchService) {
    use crate::tauri_bridge::{is_tauri_shell, workbench_load_notifications};
    use gloo_timers::future::TimeoutFuture;
    use leptos::task::spawn_local;

    if !is_tauri_shell() {
        return;
    }

    spawn_local(async move {
        let prev_seen = Rc::new(RefCell::new(HashMap::<String, String>::new()));
        loop {
            TimeoutFuture::new(NOTIFY_POLL_MS).await;
            match workbench_load_notifications().await {
                Ok(map) => {
                    let counts: HashMap<String, u32> = map
                        .iter()
                        .map(|(k, v)| (k.clone(), v.unread))
                        .collect();
                    wb.set_notifications(counts);
                    maybe_play_for_notification_delta(
                        wb,
                        &mut prev_seen.borrow_mut(),
                        &map,
                    );
                }
                Err(_) => {}
            }
        }
    });
}
