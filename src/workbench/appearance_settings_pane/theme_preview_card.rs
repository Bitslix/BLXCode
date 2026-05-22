use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::theme::{theme_desc_key, theme_name_key, AppTheme};

#[component]
pub fn ThemePreviewCard(
    theme: AppTheme,
    #[prop(into)] active: Signal<bool>,
    on_select: impl Fn(String) + 'static,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let theme_id = theme.id.to_string();
    let theme_id_for_click = theme_id.clone();
    let theme_id_for_style = theme.id;

    view! {
        <button
            type="button"
            class="theme-card"
            class:theme-card--active=move || active.get()
            role="option"
            aria-selected=move || active.get()
            aria-label=move || {
                let name = theme_name_key(theme.id)
                    .map(|k| i18n.tr(k)().to_string())
                    .unwrap_or_else(|| theme.id.to_string());
                i18n.tr(I18nKey::AppearanceThemeSelectAria)()
                    .replace("{name}", &name)
            }
            on:click=move |_| on_select(theme_id_for_click.clone())
        >
            <div
                class="theme-card__preview"
                aria-hidden="true"
                style=move || format!(
                    "--preview-sidebar:{};--preview-bg:{};--preview-accent:{};--preview-text:{};",
                    theme.preview.sidebar,
                    theme.preview.background,
                    theme.preview.accent,
                    theme.preview.text,
                )
            >
                <Show when=move || active.get()>
                    <span class="theme-card__badge">
                        {move || i18n.tr(I18nKey::AppearanceActiveBadge)()}
                    </span>
                </Show>
                <span class="theme-card__check" aria-hidden="true">
                    <LxIcon icon=icondata::LuCheck width="0.85rem" height="0.85rem" />
                </span>
            </div>
            <span class="theme-card__name">
                {move || {
                    theme_name_key(theme_id_for_style)
                        .map(|k| i18n.tr(k)().to_string())
                        .unwrap_or_else(|| theme_id_for_style.to_string())
                }}
            </span>
            <span class="theme-card__desc">
                {move || {
                    theme_desc_key(theme_id_for_style)
                        .map(|k| i18n.tr(k)().to_string())
                        .unwrap_or_default()
                }}
            </span>
        </button>
    }
}
