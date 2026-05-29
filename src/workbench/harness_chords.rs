//! Workbench keyboard shortcuts (tmux prefix chords and legacy direct chords).

use super::app_prefs::{AppPrefsService, ShortcutMode};
use super::browser_tab::sync_embedded_browser_layer;
use super::state::{
    BrowserEmbedSurface, HarnessUiService, RightPanelTab, SlotPaneState, WorkbenchService,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

const PREFIX_KEYS: &[&str] = &["Ctrl", "b"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessShortcutAction {
    OpenQuickOpen,
    ToggleRightPanel,
    RightTab(RightPanelTab),
    OpenNewTerminal,
    ToggleCommandPalette,
}

#[derive(Clone, Copy)]
pub enum ShortcutKeys {
    Combo(&'static [&'static str]),
    Chord {
        prefix: &'static [&'static str],
        second: &'static str,
    },
}

impl ShortcutKeys {
    #[must_use]
    pub fn quick_open(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "o",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "O"]),
        }
    }

    #[must_use]
    pub fn side_panel(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "r",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "P"]),
        }
    }

    #[must_use]
    pub fn agent(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "a",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "Shift", "A"]),
        }
    }

    #[must_use]
    pub fn browser(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "b",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "Shift", "B"]),
        }
    }

    #[must_use]
    pub fn memory(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "m",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "Shift", "M"]),
        }
    }

    #[must_use]
    pub fn terminal(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "n",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "Shift", "N"]),
        }
    }

    #[must_use]
    pub fn command_palette(mode: ShortcutMode) -> Self {
        match mode {
            ShortcutMode::Tmux => Self::Chord {
                prefix: PREFIX_KEYS,
                second: "p",
            },
            ShortcutMode::Legacy => Self::Combo(&["Ctrl", "Shift", "P"]),
        }
    }
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

    match prefs.shortcut_mode().get_untracked() {
        ShortcutMode::Tmux => handle_tmux_keydown(ke, ui, wb, embed),
        ShortcutMode::Legacy => handle_legacy_keydown(ke, ui, wb, embed),
    }
}

fn handle_tmux_keydown(
    ke: &KeyboardEvent,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> bool {
    if key_event_in_text_field(ke) {
        return false;
    }

    if terminal_has_live_focus() {
        return false;
    }

    let key = ke.key();

    if ui.prefix_armed().get_untracked() && key.as_str() == "Escape" {
        ke.prevent_default();
        ui.clear_prefix();
        return true;
    }

    if ui.prefix_armed().get_untracked() {
        ui.clear_prefix();
        if let Some(action) = tmux_second_key_action(&key) {
            ke.prevent_default();
            dispatch_shortcut_action(action, ui, wb, embed);
            return true;
        }
        return false;
    }

    if is_prefix_keydown(ke) {
        ke.prevent_default();
        ui.arm_prefix();
        return true;
    }

    false
}

fn handle_legacy_keydown(
    ke: &KeyboardEvent,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> bool {
    let ctrl_or_meta = ke.ctrl_key() || ke.meta_key();
    let key = ke.key();

    if ctrl_or_meta && ke.shift_key() {
        match key.as_str() {
            "p" | "P" => {
                ke.prevent_default();
                dispatch_shortcut_action(
                    HarnessShortcutAction::ToggleCommandPalette,
                    ui,
                    wb,
                    embed,
                );
                return true;
            }
            "a" | "A" => {
                ke.prevent_default();
                dispatch_shortcut_action(
                    HarnessShortcutAction::RightTab(RightPanelTab::Agent),
                    ui,
                    wb,
                    embed,
                );
                return true;
            }
            "b" | "B" => {
                ke.prevent_default();
                dispatch_shortcut_action(
                    HarnessShortcutAction::RightTab(RightPanelTab::Browser),
                    ui,
                    wb,
                    embed,
                );
                return true;
            }
            "m" | "M" => {
                ke.prevent_default();
                dispatch_shortcut_action(
                    HarnessShortcutAction::RightTab(RightPanelTab::Memory),
                    ui,
                    wb,
                    embed,
                );
                return true;
            }
            "n" | "N" => {
                ke.prevent_default();
                dispatch_shortcut_action(HarnessShortcutAction::OpenNewTerminal, ui, wb, embed);
                return true;
            }
            _ => {}
        }
    }

    if ctrl_or_meta && !ke.shift_key() {
        match key.as_str() {
            "p" | "P" => {
                ke.prevent_default();
                dispatch_shortcut_action(HarnessShortcutAction::ToggleRightPanel, ui, wb, embed);
                return true;
            }
            "o" | "O" => {
                ke.prevent_default();
                dispatch_shortcut_action(HarnessShortcutAction::OpenQuickOpen, ui, wb, embed);
                return true;
            }
            _ => {}
        }
    }

    if key.as_str() == "Escape" {
        ui.clear_prefix();
    }

    false
}

fn tmux_second_key_action(key: &str) -> Option<HarnessShortcutAction> {
    match key {
        "o" | "O" => Some(HarnessShortcutAction::OpenQuickOpen),
        "r" | "R" => Some(HarnessShortcutAction::ToggleRightPanel),
        "a" | "A" => Some(HarnessShortcutAction::RightTab(RightPanelTab::Agent)),
        "b" | "B" => Some(HarnessShortcutAction::RightTab(RightPanelTab::Browser)),
        "m" | "M" => Some(HarnessShortcutAction::RightTab(RightPanelTab::Memory)),
        "n" | "N" => Some(HarnessShortcutAction::OpenNewTerminal),
        "p" | "P" => Some(HarnessShortcutAction::ToggleCommandPalette),
        _ => None,
    }
}

fn is_prefix_keydown(ke: &KeyboardEvent) -> bool {
    ke.ctrl_key()
        && !ke.shift_key()
        && !ke.alt_key()
        && !ke.meta_key()
        && matches!(ke.key().as_str(), "b" | "B")
}

fn key_event_in_text_field(ke: &KeyboardEvent) -> bool {
    let Some(target) = ke.target() else {
        return false;
    };
    let Some(el) = target.dyn_ref::<web_sys::Element>() else {
        return false;
    };
    let tag = el.tag_name();
    if tag == "INPUT" || tag == "TEXTAREA" {
        return true;
    }
    el.get_attribute("contenteditable")
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// True when keyboard focus *currently* sits inside a workspace terminal
/// (xterm) cell.
///
/// We must read the live DOM focus here, not the sticky
/// `focused_terminal_by_workspace` map: that map remembers the last-focused
/// terminal per workspace for notification/accent purposes and is never
/// cleared on blur. Consulting it would permanently disable tmux chords once
/// any terminal was ever focused — notably on Windows (WebView2), where the
/// xterm textarea grabs focus as soon as the terminal grid mounts.
fn terminal_has_live_focus() -> bool {
    let Some(active) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
    else {
        return false;
    };
    active
        .closest(".ws-term-cell")
        .ok()
        .flatten()
        .is_some()
}

fn defer_browser_bounds(wb: WorkbenchService, embed: BrowserEmbedSurface) {
    leptos::task::spawn_local(async move {
        TimeoutFuture::new(48).await;
        let _ = sync_embedded_browser_layer(wb, embed).await;
    });
}
