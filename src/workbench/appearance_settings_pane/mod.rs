mod theme_preview_card;

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::theme::{theme_desc_key, theme_name_key, RadiusScale, ThemeMode, FONTS, THEMES};
use crate::workbench::theme_service::{theme_count, ThemeService};
use crate::workbench::SettingsPaneHeader;

use theme_preview_card::ThemePreviewCard;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModeFilter {
    All,
    Dark,
    Light,
}

/// Roundings options paired with their label key and a fixed swatch radius
/// (visual only — independent of the global scale so the chip always reads).
fn radius_options() -> [(RadiusScale, I18nKey, &'static str); 4] {
    [
        (RadiusScale::Sharp, I18nKey::AppearanceRoundingSharp, "0"),
        (RadiusScale::Default, I18nKey::AppearanceRoundingDefault, "5px"),
        (RadiusScale::Rounded, I18nKey::AppearanceRoundingRounded, "9px"),
        (RadiusScale::Extra, I18nKey::AppearanceRoundingExtra, "15px"),
    ]
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
    let active_theme = move || {
        let id = theme_svc.active_theme_id().get();
        THEMES
            .iter()
            .find(|theme| theme.id == id)
            .copied()
            .unwrap_or(THEMES[0])
    };
    let radius_scale = theme_svc.radius_scale();
    let font_id = theme_svc.font_id();

    view! {
        <article class="appearance-pane harness-pane">
            <div class="appearance-pane__top">
                <SettingsPaneHeader
                    icon=icondata::LuSunMoon
                    title=I18nKey::AppearanceHeading
                    description=I18nKey::AppearanceDescription
                />
                <div
                    class="appearance-active-preview"
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
                    <span class="appearance-active-preview__badge">
                        {move || i18n.tr(I18nKey::AppearanceActiveBadge)()}
                    </span>
                </div>
            </div>

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

            <div class="appearance-controls">
                <section class="appearance-control">
                    <div class="appearance-control__head">
                        <h3 class="appearance-control__title">
                            {move || i18n.tr(I18nKey::AppearanceRoundingsTitle)()}
                        </h3>
                        <p class="appearance-control__desc">
                            {move || i18n.tr(I18nKey::AppearanceRoundingsDesc)()}
                        </p>
                    </div>
                    <div
                        class="appearance-seg"
                        role="group"
                        aria-label=move || i18n.tr(I18nKey::AppearanceRoundingsAria)()
                    >
                        {radius_options()
                            .into_iter()
                            .map(|(scale, key, swatch)| {
                                view! {
                                    <button
                                        type="button"
                                        class="appearance-seg-btn"
                                        class:appearance-seg-btn--active=move || {
                                            radius_scale.get() == scale
                                        }
                                        aria-pressed=move || {
                                            (radius_scale.get() == scale).to_string()
                                        }
                                        on:click=move |_| theme_svc.set_radius_scale(scale)
                                    >
                                        <span
                                            class="appearance-seg-btn__swatch"
                                            style=format!("border-radius:{swatch}")
                                            aria-hidden="true"
                                        ></span>
                                        <span class="appearance-seg-btn__label">
                                            {move || i18n.tr(key)()}
                                        </span>
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                </section>

                <section class="appearance-control">
                    <div class="appearance-control__head">
                        <h3 class="appearance-control__title">
                            {move || i18n.tr(I18nKey::AppearanceFontTitle)()}
                        </h3>
                        <p class="appearance-control__desc">
                            {move || i18n.tr(I18nKey::AppearanceFontDesc)()}
                        </p>
                    </div>
                    <div
                        class="appearance-font-grid"
                        role="group"
                        aria-label=move || i18n.tr(I18nKey::AppearanceFontAria)()
                    >
                        {FONTS
                            .iter()
                            .map(|font| {
                                view! {
                                    <button
                                        type="button"
                                        class="appearance-font-btn"
                                        class:appearance-font-btn--active=move || {
                                            font_id.get() == font.id
                                        }
                                        aria-pressed=move || (font_id.get() == font.id).to_string()
                                        style=format!("font-family:{}", font.stack)
                                        on:click=move |_| theme_svc.set_font(font.id)
                                    >
                                        <span class="appearance-font-btn__name">{font.label}</span>
                                        <span class="appearance-font-btn__sample">
                                            "AaBbCc 0123 () => {}"
                                        </span>
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                </section>
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
