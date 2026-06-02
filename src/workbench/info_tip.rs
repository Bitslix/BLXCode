//! App-global tooltip — the single, theme-driven hover/focus popover style.
//!
//! Visual style lives in CSS as `.blx-tooltip` (+ `__eyebrow` / `__spark` /
//! `__main` / `__hint`) and uses only theme tokens, so it follows every theme
//! automatically. [`InfoTip`] wraps an arbitrary trigger (`children`) in a
//! `.blx-tip-anchor` and reveals the popover on hover / focus-within.
//!
//! This generalises the original sidebar voice-orb tooltip; the sidebar now
//! reuses the same `.blx-tooltip` visual rules.

use leptos::prelude::*;

/// Where the popover is drawn relative to its anchor.
#[allow(dead_code)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TipPlacement {
    /// Above the trigger (default).
    #[default]
    Top,
    /// Below the trigger.
    Bottom,
}

impl TipPlacement {
    fn anchor_class(self) -> &'static str {
        match self {
            TipPlacement::Top => "blx-tip-anchor blx-tip-anchor--top",
            TipPlacement::Bottom => "blx-tip-anchor blx-tip-anchor--bottom",
        }
    }
}

/// Reusable app-global tooltip. Renders `children` as the visible trigger and
/// shows a `.blx-tooltip` popover (eyebrow + main + optional hint) on
/// hover / keyboard focus.
#[component]
pub fn InfoTip(
    /// Small uppercase eyebrow line (rendered with the accent dot).
    #[prop(into)]
    eyebrow: Signal<String>,
    /// Bold main line.
    #[prop(into)]
    main: Signal<String>,
    /// Optional muted explanation line.
    #[prop(into)]
    hint: Signal<Option<String>>,
    /// Popover placement relative to the trigger.
    #[prop(optional)]
    placement: TipPlacement,
    /// The visible trigger (icon, value, row, …).
    children: Children,
) -> impl IntoView {
    view! {
        <span class=placement.anchor_class()>
            {children()}
            <span class="blx-tooltip" role="tooltip">
                <span class="blx-tooltip__eyebrow">
                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                    {move || eyebrow.get()}
                </span>
                <span class="blx-tooltip__main">{move || main.get()}</span>
                {move || hint.get().map(|h| view! { <span class="blx-tooltip__hint">{h}</span> })}
            </span>
        </span>
    }
}
