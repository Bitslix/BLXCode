//! Workbench keyboard shortcuts.
//!
//! Bindings are data-driven (see [`super::shortcut_config`]): a single
//! configurable prefix plus one binding per action, each either a direct
//! combo or a tmux-style prefix-then-key chord. The same config feeds both
//! the matching here and the on-screen display.

use super::app_prefs::AppPrefsService;
use super::browser_tab::sync_embedded_browser_layer;
use super::shortcut_config::ShortcutConfig;
use super::state::{
    BrowserEmbedSurface, HarnessUiService, RightPanelTab, SlotPaneState, WorkbenchService,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessShortcutAction {
    OpenQuickOpen,
    ToggleRightPanel,
    RightTab(RightPanelTab),
    OpenNewTerminal,
    ToggleCommandPalette,
}

pub fn dispatch_shortcut_action(
    action: HarnessShortcutAction,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) {
    match action {
        HarnessShortcutAction::OpenQuickOpen => ui.toggle_quick_open(),
        HarnessShortcutAction::ToggleRightPanel => {
            wb.toggle_right_panel();
            defer_browser_bounds(wb, embed);
        }
        HarnessShortcutAction::RightTab(tab) => {
            if wb.right_collapsed().get_untracked() {
                wb.toggle_right_panel();
            }
            wb.set_right_tab(tab);
            defer_browser_bounds(wb, embed);
        }
        HarnessShortcutAction::OpenNewTerminal => open_new_terminal(wb),
        HarnessShortcutAction::ToggleCommandPalette => ui.toggle_command_palette(),
    }
}

pub fn open_new_terminal(wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        return;
    };
    // The user may have closed the Terminals tab earlier (e.g. opened
    // Settings, then triggered the new-terminal shortcut). Restore the
    // Terminals tab before appending a slot so the new PTY actually has a
    // surface to render into.
    wb.open_center_terminals_tab(workspace_id);
    match wb.append_terminal_slot(workspace_id, None) {
        Ok(slot_id) => {
            let terminal_key = wb.workspaces().with_untracked(|list| {
                list.iter().find(|w| w.id == workspace_id).map(|w| {
                    let pane_id = SlotPaneState::default_for_slot(slot_id).pane_ids[0];
                    format!("{}:{slot_id}:{pane_id}", w.storage_key)
                })
            });
            if let Some(key) = terminal_key {
                wb.focus_terminal(key);
                wb.bump_terminal_layout();
            }
        }
        Err(e) => leptos::logging::warn!("open_new_terminal: {e}"),
    }
}

pub fn handle_harness_keydown(
    ke: &KeyboardEvent,
    prefs: AppPrefsService,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> bool {
    let blocked = ui.palette_open().get_untracked()
        || ui.settings_open().get_untracked()
        || ui.quick_open_open().get_untracked();
    // While the Terminals-close countdown is active we swallow every
    // shortcut except Escape (which dismisses the dialog) so the user
    // can't accidentally rebind keys to other workspace actions.
    let close_terminals_open = ui.close_terminals_confirm().get_untracked().is_some();
    let key = ke.key();

    if (blocked || close_terminals_open) && key.as_str() == "Escape" {
        ke.prevent_default();
        if close_terminals_open {
            ui.dismiss_close_terminals_confirm();
        }
        ui.close_command_palette();
        ui.close_settings();
        ui.close_quick_open();
        return true;
    }

    if blocked || close_terminals_open {
        return false;
    }

    let cfg = prefs.shortcut_config().get_untracked();
    handle_shortcut_keydown(ke, &cfg, ui, wb, embed)
}

fn handle_shortcut_keydown(
    ke: &KeyboardEvent,
    cfg: &ShortcutConfig,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> bool {
    // Don't steal keystrokes while the user is typing in a genuine app text
    // field. The terminal (xterm) is deliberately *not* treated as one: our
    // chords must fire there too (e.g. `Ctrl+b n` in a plain shell), which is
    // the configured default.
    if key_event_in_non_terminal_text_field(ke) {
        return false;
    }

    let key = ke.key();

    // Prefix armed: resolve the second key, cancel on Escape, or fall through
    // on an unrecognised key.
    if ui.prefix_armed().get_untracked() {
        ui.clear_prefix();
        if key.as_str() == "Escape" {
            ke.prevent_default();
            return true;
        }
        if let Some(action) = cfg.chord_match(ke) {
            ke.prevent_default();
            dispatch_shortcut_action(action.to_harness_action(), ui, wb, embed);
            return true;
        }
        return false;
    }

    // The prefix itself: arm and wait for the second key.
    if cfg.prefix.matches(ke) {
        ke.prevent_default();
        ui.arm_prefix();
        return true;
    }

    // Direct combo bindings (classic style).
    if let Some(action) = cfg.combo_match(ke) {
        ke.prevent_default();
        dispatch_shortcut_action(action.to_harness_action(), ui, wb, embed);
        return true;
    }

    if key.as_str() == "Escape" {
        ui.clear_prefix();
    }

    false
}

/// True when the event originates from a real app text input that is **not**
/// the terminal. The xterm hidden textarea lives inside `.ws-term-cell`; we
/// intentionally let chords fire there, so it is excluded.
fn key_event_in_non_terminal_text_field(ke: &KeyboardEvent) -> bool {
    let Some(target) = ke.target() else {
        return false;
    };
    let Some(el) = target.dyn_ref::<web_sys::Element>() else {
        return false;
    };
    let tag = el.tag_name();
    let is_text = tag == "INPUT"
        || tag == "TEXTAREA"
        || el
            .get_attribute("contenteditable")
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("true"));
    if !is_text {
        return false;
    }
    el.closest(".ws-term-cell").ok().flatten().is_none()
}

fn defer_browser_bounds(wb: WorkbenchService, embed: BrowserEmbedSurface) {
    leptos::task::spawn_local(async move {
        TimeoutFuture::new(48).await;
        let _ = sync_embedded_browser_layer(wb, embed).await;
    });
}
