//! Workspace settings — memory category color presets.

use crate::workbench::state::{normalize_hex_color, MemoryColorPreset, WorkbenchService};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use js_sys::Date;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

fn input_value(ev: web_sys::Event) -> Option<String> {
    ev.target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
}

fn update_memory_preset(
    wb: WorkbenchService,
    id: &str,
    label: Option<String>,
    color: Option<String>,
) {
    let mut presets = wb.memory_color_presets().get_untracked();
    if let Some(preset) = presets.iter_mut().find(|preset| preset.id == id) {
        if let Some(label) = label {
            preset.label = label;
        }
        if let Some(color) = color {
            preset.color = normalize_hex_color(&color, "#7dd3fc");
        }
    }
    wb.set_memory_color_presets(presets);
}

#[component]
pub fn WorkspaceCategoryColorsSection(wb: WorkbenchService) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let new_label = RwSignal::new(String::new());
    let new_color = RwSignal::new("#7dd3fc".to_string());

    view! {
        <section class="harness-subpane memory-presets">
            <h4 class="harness-pane-subhead">
                <span class="harness-pane-subhead__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuPalette width="0.82rem" height="0.82rem" />
                </span>
                <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::WsSectionCategoryColors)()}</span>
            </h4>
            <ol class="memory-preset-list">
                <For
                    each=move || wb.memory_color_presets().get()
                    key=|preset| preset.id.clone()
                    children=move |preset: MemoryColorPreset| {
                        let id_for_label = preset.id.clone();
                        let id_for_color = preset.id.clone();
                        let id_for_delete = preset.id.clone();
                        view! {
                            <li class="memory-preset-row">
                                <input
                                    type="color"
                                    class="memory-preset-row__color"
                                    prop:value=preset.color.clone()
                                    on:input=move |ev| {
                                        if let Some(value) = input_value(ev) {
                                            update_memory_preset(wb, &id_for_color, None, Some(value));
                                        }
                                    }
                                />
                                <input
                                    type="text"
                                    class="workbench-plain-input memory-preset-row__name"
                                    prop:value=preset.label.clone()
                                    on:input=move |ev| {
                                        if let Some(value) = input_value(ev) {
                                            update_memory_preset(wb, &id_for_label, Some(value), None);
                                        }
                                    }
                                />
                                <button
                                    type="button"
                                    class="workbench-mini-btn memory-preset-row__delete"
                                    title=move || i18n.tr(I18nKey::WsCategoryColorsDeleteAria)()
                                    aria-label=move || i18n.tr(I18nKey::WsCategoryColorsDeleteAria)()
                                    on:click=move |_| {
                                        let mut presets = wb.memory_color_presets().get_untracked();
                                        presets.retain(|preset| preset.id != id_for_delete);
                                        wb.set_memory_color_presets(presets);
                                    }
                                >
                                    <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
                                </button>
                            </li>
                        }
                    }
                />
            </ol>
            <form
                class="memory-preset-add"
                on:submit=move |ev: web_sys::SubmitEvent| {
                    ev.prevent_default();
                    let name = new_label.get_untracked().trim().to_string();
                    if name.is_empty() {
                        return;
                    }
                    let mut presets = wb.memory_color_presets().get_untracked();
                    presets.push(MemoryColorPreset {
                        id: format!("custom-{}", Date::now() as i64),
                        label: name,
                        color: normalize_hex_color(&new_color.get_untracked(), "#7dd3fc"),
                    });
                    wb.set_memory_color_presets(presets);
                    new_label.set(String::new());
                }
            >
                <input
                    type="color"
                    class="memory-preset-row__color"
                    prop:value=move || new_color.get()
                    on:input=move |ev| {
                        if let Some(value) = input_value(ev) {
                            new_color.set(value);
                        }
                    }
                />
                <input
                    type="text"
                    class="workbench-plain-input"
                    placeholder=move || i18n.tr(I18nKey::WsCategoryColorsPresetPlaceholder)()
                    prop:value=move || new_label.get()
                    on:input=move |ev| {
                        if let Some(value) = input_value(ev) {
                            new_label.set(value);
                        }
                    }
                />
                <button type="submit" class="workbench-mini-btn workbench-mini-btn--primary">
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuPlus width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::WsCategoryColorsAdd)()}</span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    on:click=move |_| wb.reset_memory_color_presets()
                >
                    {move || i18n.tr(I18nKey::WsCategoryColorsReset)()}
                </button>
            </form>
        </section>
    }
}
