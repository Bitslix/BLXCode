//! VS Code–style collapsible sidebar section (header + optional toolbar + body).
use crate::i18n::I18nKey;
use crate::service::I18nService;
use leptos::prelude::*;

#[component]
pub fn SidebarViewSection(
    /// Section label (uppercase styling applied in CSS).
    title: Signal<String>,
    section_id: &'static str,
    open: RwSignal<bool>,
    /// Toolbar row (pass `view! { }.into_any()` or empty `().into_any()`).
    toolbar: AnyView,
    children: Children,
) -> impl IntoView {
    let body_id = format!("{section_id}-body");

    view! {
        <section class="sidebar-view-section">
            <div class="sidebar-view-section__head">
                <button
                    type="button"
                    class="sidebar-view-section__toggle"
                    aria-expanded=move || open.get().to_string()
                    aria-controls=body_id.clone()
                    on:click=move |_| open.update(|v| *v = !*v)
                >
                    <span class="sidebar-view-section__chev" aria-hidden="true">
                        {move || if open.get() { "▾" } else { "▸" }}
                    </span>
                    <span class="sidebar-view-section__title">{move || title.get()}</span>
                </button>
                <div class="sidebar-view-section__toolbar">
                    {toolbar}
                </div>
            </div>
            <div
                id=body_id
                class="sidebar-view-section__body"
                class:sidebar-view-section__body--collapsed=move || !open.get()
                role="region"
                aria-label=move || title.get()
                aria-hidden=move || (!open.get()).to_string()
            >
                {children()}
            </div>
        </section>
    }
}

/// Icon button for section toolbars.
#[component]
pub fn SidebarSectionIconBtn(
    aria_key: I18nKey,
    #[prop(into)] on_click: Callback<()>,
    children: Children,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="sidebar-view-section__icon-btn"
            aria-label=move || i18n.tr(aria_key)()
            title=move || i18n.tr(aria_key)()
            on:click=move |_| on_click.run(())
        >
            {children()}
        </button>
    }
}
