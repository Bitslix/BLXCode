mod theme_preview_card;

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::theme::{theme_desc_key, theme_name_key, ThemeMode, THEMES};
use crate::workbench::theme_service::{theme_count, ThemeService};

use theme_preview_card::ThemePreviewCard;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModeFilter {
    All,
    Dark,
    Light,
}

#[component]
pub fn AppearanceSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let theme_svc = expect_context::<ThemeService>();
    let search_query = RwSignal::new(String::new());
    let mode_filter = RwSignal::new(ModeFilter::All);

    let filtered_themes = Memo::new(move |_| {
        let query = search_query.get().trim().to_lowercase();
        let mode = mode_filter.get();
        THEMES
            .iter()
            .copied()
            .filter(|t| match mode {
                ModeFilter::All => true,
                ModeFilter::Dark => t.mode == ThemeMode::Dark,
                ModeFilter::Light => t.mode == ThemeMode::Light,
            })
            .filter(|t| {
                if query.is_empty() {
                    return true;
                }
                let name = theme_name_key(t.id)
                    .map(|k| i18n.tr(k)().to_lowercase())
                    .unwrap_or_default();
                let desc = theme_desc_key(t.id)
                    .map(|k| i18n.tr(k)().to_lowercase())
                    .unwrap_or_default();
                name.contains(&query) || desc.contains(&query)
            })
            .collect::<Vec<_>>()
    });

    let dark_count = Memo::new(|_| THEMES.iter().filter(|t| t.mode == ThemeMode::Dark).count());
    let light_count = Memo::new(|_| THEMES.iter().filter(|t| t.mode == ThemeMode::Light).count());
    let total_count = theme_count();

    let active_theme = theme_svc.active_theme();

    view! {
        <article class="appearance-pane harness-pane">
            <header class="appearance-hero">
                <div class="appearance-hero__copy">
                    <h3 class="appearance-hero__title">
                        {move || i18n.tr(I18nKey::AppearanceHeroTitle)()}
                    </h3>
                    <p class="appearance-hero__subtitle">
                        {move || {
                            i18n.tr(I18nKey::AppearanceHeroSubtitle)()
                                .replace("{n}", &total_count.to_string())
                        }}
                    </p>
                </div>
                <div
                    class="appearance-hero__preview"
                    aria-label=move || i18n.tr(I18nKey::AppearanceActivePreviewLabel)()
                    style=move || {
                        let t = active_theme();
                        format!(
                            "--preview-sidebar:{};--preview-bg:{};--preview-accent:{};--preview-text:{};",
                            t.preview.sidebar,
                            t.preview.background,
                            t.preview.accent,
                            t.preview.text,
                        )
                    }
                >
                    <span class="appearance-hero__preview-badge">
                        {move || i18n.tr(I18nKey::AppearanceActiveBadge)()}
                    </span>
                </div>
            </header>

            <div class="appearance-toolbar">
                <label class="appearance-search">
                    <span class="appearance-search__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuSearch width="0.95rem" height="0.95rem" />
                    </span>
                    <input
                        type="search"
                        class="workbench-plain-input appearance-search__input"
                        placeholder=move || i18n.tr(I18nKey::AppearanceSearchPlaceholder)()
                        aria-label=move || i18n.tr(I18nKey::AppearanceSearchAria)()
                        prop:value=move || search_query.get()
                        on:input=move |ev| {
                            if let Some(v) = event_target_value(&ev) {
                                search_query.set(v);
                            }
                        }
                    />
                </label>
                <div
                    class="appearance-filter-group"
                    role="group"
                    aria-label=move || i18n.tr(I18nKey::AppearanceFilterAria)()
                >
                    <button
                        type="button"
                        class="appearance-filter-btn"
                        class:appearance-filter-btn--active=move || mode_filter.get() == ModeFilter::All
                        on:click=move |_| mode_filter.set(ModeFilter::All)
                    >
                        {move || {
                            format!(
                                "{} ({})",
                                i18n.tr(I18nKey::AppearanceFilterAll)(),
                                total_count
                            )
                        }}
                    </button>
                    <button
                        type="button"
                        class="appearance-filter-btn"
                        class:appearance-filter-btn--active=move || mode_filter.get() == ModeFilter::Dark
                        on:click=move |_| mode_filter.set(ModeFilter::Dark)
                    >
                        {move || {
                            format!(
                                "{} ({})",
                                i18n.tr(I18nKey::AppearanceFilterDark)(),
                                dark_count.get()
                            )
                        }}
                    </button>
                    <button
                        type="button"
                        class="appearance-filter-btn"
                        class:appearance-filter-btn--active=move || mode_filter.get() == ModeFilter::Light
                        on:click=move |_| mode_filter.set(ModeFilter::Light)
                    >
                        {move || {
                            format!(
                                "{} ({})",
                                i18n.tr(I18nKey::AppearanceFilterLight)(),
                                light_count.get()
                            )
                        }}
                    </button>
                </div>
            </div>

            <div
                class="appearance-grid"
                role="listbox"
                aria-label=move || i18n.tr(I18nKey::AppearanceThemeGridAria)()
            >
                <For
                    each=move || filtered_themes.get()
                    key=|t| t.id
                    children=move |theme| {
                        let active = Signal::derive(move || {
                            theme_svc.active_theme_id().get() == theme.id
                        });
                        view! {
                            <ThemePreviewCard
                                theme=theme
                                active=active
                                on_select=move |id: String| theme_svc.set_theme(&id)
                            />
                        }
                    }
                />
            </div>
        </article>
    }
}

fn event_target_value(ev: &leptos::ev::Event) -> Option<String> {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
}
