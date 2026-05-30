//! Settings pane for viewing and rebinding keyboard shortcuts.
//!
//! Lets the user pick a preset (Tmux / Classic), rebind the shared prefix key,
//! and rebind or reset each individual action. All colours/spacing come from
//! theme tokens (see `shortcuts_settings_pane.css`) so every theme is honoured.

use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use web_sys::KeyboardEvent;

use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::workbench::app_prefs::{AppPrefsService, ShortcutMode};
use crate::workbench::shortcut_config::{Binding, KeyChord, ShortcutAction};
use crate::workbench::state::HarnessUiService;

/// What a running key capture is targeting.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureTarget {
    Prefix,
    Action(ShortcutAction),
}

fn action_icon(action: ShortcutAction) -> icondata::Icon {
    match action {
        ShortcutAction::QuickOpen => icondata::LuFolderSearch,
        ShortcutAction::SidePanel => icondata::LuPanelRight,
        ShortcutAction::Agent => icondata::LuSparkles,
        ShortcutAction::Browser => icondata::LuGlobe,
        ShortcutAction::Memory => icondata::LuLayers,
        ShortcutAction::Terminal => icondata::LuTerminal,
        ShortcutAction::CommandPalette => icondata::LuCommand,
    }
}

#[component]
pub fn ShortcutsSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let ui = expect_context::<HarnessUiService>();

    let capturing: RwSignal<Option<CaptureTarget>> = RwSignal::new(None);
    let capture_ref = NodeRef::<html::Button>::new();

    // Move keyboard focus onto the capture catcher as soon as a capture
    // begins, so it receives the next keydown.
    Effect::new(move |_| {
        if capturing.get().is_some() {
            if let Some(el) = capture_ref.get() {
                let _ = el.focus();
            }
        }
    });

    let on_capture_key = move |ev: KeyboardEvent| {
        // Intercept before the global harness handler (this element is deeper
        // in the DOM, so its bubble-phase handler runs first).
        ev.stop_propagation();
        ev.prevent_default();
        let key = ev.key();
        if matches!(key.as_str(), "Control" | "Shift" | "Alt" | "Meta") {
            return; // wait for a non-modifier key
        }
        if key == "Escape" {
            capturing.set(None);
            return;
        }
        let Some(target) = capturing.get_untracked() else {
            return;
        };
        let Some(chord) = KeyChord::from_event(&ev) else {
            return;
        };
        match target {
            CaptureTarget::Prefix => prefs.set_shortcut_prefix(chord),
            CaptureTarget::Action(action) => {
                // Keep the existing binding *style*: rebinding a tmux chord
                // sets its second key; rebinding a direct combo sets the combo.
                let binding = match prefs.shortcut_config().get_untracked().binding(action) {
                    Binding::Chord { .. } => Binding::Chord { second: chord.key },
                    Binding::Combo(_) => Binding::Combo(chord),
                };
                prefs.set_shortcut_binding(action, binding);
            }
        }
        ui.clear_prefix();
        capturing.set(None);
    };

    view! {
        <article class="harness-pane shortcuts-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuKeyboard width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">
                    {move || i18n.tr(I18nKey::ShortcutsHeading)()}
                </span>
            </h3>

            // Preset selector (moved here from the App pane).
            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuLayers width="0.82rem" height="0.82rem" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::ShortcutsPresetHeading)()}</span>
                </h4>
                <div class="app-prefs-toggle-grid">
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="shortcut-preset"
                                prop:checked=move || prefs.shortcut_mode().get() == ShortcutMode::Tmux
                                on:change=move |_| {
                                    prefs.apply_shortcut_preset(ShortcutMode::Tmux);
                                    ui.clear_prefix();
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::AppShortcutModeTmux)()}</span>
                        </label>
                    </div>
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="shortcut-preset"
                                prop:checked=move || prefs.shortcut_mode().get() == ShortcutMode::Legacy
                                on:change=move |_| {
                                    prefs.apply_shortcut_preset(ShortcutMode::Legacy);
                                    ui.clear_prefix();
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::AppShortcutModeLegacy)()}</span>
                        </label>
                    </div>
                </div>
                <p class="app-prefs-hint">{move || i18n.tr(I18nKey::ShortcutsHint)()}</p>
            </section>

            // Prefix key.
            <section class="harness-subpane">
                <div class="shortcuts-pane__row">
                    <span class="shortcuts-pane__label">
                        {move || i18n.tr(I18nKey::ShortcutsPrefixLabel)()}
                    </span>
                    <span class="shortcuts-pane__keys">
                        <kbd class="workbench-kbd">
                            {move || prefs.shortcut_config().get().prefix.parts().join(" + ")}
                        </kbd>
                    </span>
                    <button
                        type="button"
                        class="shortcuts-pane__btn"
                        on:click=move |_| capturing.set(Some(CaptureTarget::Prefix))
                    >
                        <LxIcon icon=icondata::LuPencil width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::ShortcutsRebind)()}</span>
                    </button>
                </div>
            </section>

            // Per-action bindings.
            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuKeyboard width="0.82rem" height="0.82rem" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::ShortcutsBindingsHeading)()}</span>
                </h4>
                <ul class="shortcuts-pane__list">
                    {ShortcutAction::ALL
                        .into_iter()
                        .map(|action| view! { <ActionRow action=action capturing=capturing /> })
                        .collect_view()}
                </ul>
                <button
                    type="button"
                    class="shortcuts-pane__btn shortcuts-pane__btn--reset-all"
                    on:click=move |_| {
                        prefs.apply_shortcut_preset(prefs.shortcut_mode().get_untracked());
                        ui.clear_prefix();
                    }
                >
                    <LxIcon icon=icondata::LuRotateCcw width="0.78rem" height="0.78rem" />
                    <span>{move || i18n.tr(I18nKey::ShortcutsResetAll)()}</span>
                </button>
            </section>

            // Capture catcher: always present, only visible while a capture is
            // running. Focused on demand; swallows the keystroke.
            <button
                type="button"
                node_ref=capture_ref
                class="shortcuts-pane__capture"
                class:shortcuts-pane__capture--active=move || capturing.get().is_some()
                on:keydown=on_capture_key
                on:blur=move |_| capturing.set(None)
            >
                {move || i18n.tr(I18nKey::ShortcutsCapturePrompt)()}
            </button>
        </article>
    }
}

#[component]
fn ActionRow(action: ShortcutAction, capturing: RwSignal<Option<CaptureTarget>>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let ui = expect_context::<HarnessUiService>();
    let label = action.label_key();
    let icon = action_icon(action);

    let keys_text = move || {
        let cfg = prefs.shortcut_config().get();
        let then = lookup(i18n.locale().get(), I18nKey::WsKwThen);
        cfg.binding(action).display(&cfg.prefix, then)
    };
    let has_conflict = move || !prefs.shortcut_config().get().conflicts(action).is_empty();

    view! {
        <li class="shortcuts-pane__row">
            <span class="shortcuts-pane__lead">
                <span class="shortcuts-pane__icon" aria-hidden="true">
                    <LxIcon icon=icon width="0.82rem" height="0.82rem" />
                </span>
                <span class="shortcuts-pane__label">{move || i18n.tr(label)()}</span>
            </span>
            <span class="shortcuts-pane__keys">
                <kbd class="workbench-kbd">{keys_text}</kbd>
                <Show when=has_conflict>
                    <span
                        class="shortcuts-pane__conflict"
                        title=move || i18n.tr(I18nKey::ShortcutsConflict)()
                        aria-label=move || i18n.tr(I18nKey::ShortcutsConflict)()
                    >
                        <LxIcon icon=icondata::LuTriangleAlert width="0.82rem" height="0.82rem" />
                    </span>
                </Show>
            </span>
            <span class="shortcuts-pane__actions">
                <button
                    type="button"
                    class="shortcuts-pane__btn"
                    on:click=move |_| capturing.set(Some(CaptureTarget::Action(action)))
                >
                    <LxIcon icon=icondata::LuPencil width="0.78rem" height="0.78rem" />
                    <span>{move || i18n.tr(I18nKey::ShortcutsRebind)()}</span>
                </button>
                <button
                    type="button"
                    class="shortcuts-pane__btn shortcuts-pane__btn--icon"
                    title=move || i18n.tr(I18nKey::ShortcutsResetOne)()
                    aria-label=move || i18n.tr(I18nKey::ShortcutsResetOne)()
                    on:click=move |_| {
                        prefs.reset_shortcut_binding(action);
                        ui.clear_prefix();
                    }
                >
                    <LxIcon icon=icondata::LuRotateCcw width="0.78rem" height="0.78rem" />
                </button>
            </span>
        </li>
    }
}
