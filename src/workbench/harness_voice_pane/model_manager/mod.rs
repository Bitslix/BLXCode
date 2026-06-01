//! Whisper model manager UI: a filterable list of downloadable models with
//! live download progress (speed + percent), resume, delete, and "use".
//!
//! Progress arrives via Tauri events (`whisper_download_*`); the list itself
//! comes from `whisper_models_list`. Colours are theme tokens only
//! (`model_manager.css`).

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, listen_whisper_download_done, listen_whisper_download_error,
    listen_whisper_download_progress, whisper_model_cancel, whisper_model_delete,
    whisper_model_download, whisper_models_list, ModelFamily, WhisperModelView,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Family(ModelFamily),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortBy {
    Default,
    Size,
    Accuracy,
    Speed,
}

/// In-flight progress for one model.
#[derive(Clone, Copy)]
struct Prog {
    received: u64,
    total: u64,
    speed_bps: f64,
}

#[component]
pub fn ModelManager<U>(selected: Signal<Option<String>>, on_use: U) -> impl IntoView
where
    U: Fn(String) + Copy + Send + Sync + 'static,
{
    let i18n = expect_context::<I18nService>();
    let models = RwSignal::new(Vec::<WhisperModelView>::new());
    let filter = RwSignal::new(Filter::All);
    let sort_by = RwSignal::new(SortBy::Default);
    let progress = RwSignal::new(HashMap::<String, Prog>::new());
    let errors = RwSignal::new(HashMap::<String, String>::new());

    let reload = move || {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(list) = whisper_models_list().await {
                models.set(list);
            }
        });
    };
    reload();

    // Live download events. Listeners are kept for the component's lifetime and
    // dropped (unlistened) on cleanup.
    if is_tauri_shell() {
        let p = listen_whisper_download_progress(move |ev| {
            progress.update(|m| {
                m.insert(
                    ev.id.clone(),
                    Prog {
                        received: ev.received,
                        total: ev.total,
                        speed_bps: ev.speed_bps,
                    },
                );
            });
        });
        let d = listen_whisper_download_done(move |ev| {
            progress.update(|m| {
                m.remove(&ev.id);
            });
            errors.update(|m| {
                m.remove(&ev.id);
            });
            reload();
        });
        let e = listen_whisper_download_error(move |ev| {
            progress.update(|m| {
                m.remove(&ev.id);
            });
            // "cancelled" is a normal terminal state; don't surface it as error.
            if ev.message != "cancelled" {
                errors.update(|m| {
                    m.insert(ev.id.clone(), ev.message.clone());
                });
            }
            reload();
        });
        let keep = send_wrapper::SendWrapper::new((p, d, e));
        on_cleanup(move || {
            let _ = keep.take();
        });
    }

    let filtered = move || {
        let f = filter.get();
        let mut out = models
            .get()
            .into_iter()
            .filter(|m| match f {
                Filter::All => true,
                Filter::Family(fam) => m.family == fam,
            })
            .collect::<Vec<_>>();
        match sort_by.get() {
            SortBy::Default => {}
            SortBy::Size => out.sort_by_key(|m| m.size_bytes),
            SortBy::Accuracy => out.sort_by(|a, b| {
                b.accuracy_rating
                    .cmp(&a.accuracy_rating)
                    .then(a.size_bytes.cmp(&b.size_bytes))
            }),
            SortBy::Speed => out.sort_by(|a, b| {
                b.speed_rating
                    .cmp(&a.speed_rating)
                    .then(a.size_bytes.cmp(&b.size_bytes))
            }),
        }
        out
    };

    view! {
        <div class="mm">
            <div class="mm__head">
                <h6 class="mm__title">{move || i18n.tr(I18nKey::VoicePttModelsTitle)()}</h6>
                <select
                    class="mm__sort"
                    on:change=move |ev| {
                        sort_by.set(match event_target_value(&ev).as_str() {
                            "size" => SortBy::Size,
                            "accuracy" => SortBy::Accuracy,
                            "speed" => SortBy::Speed,
                            _ => SortBy::Default,
                        });
                    }
                >
                    <option value="default">"Default order"</option>
                    <option value="size">"Size"</option>
                    <option value="accuracy">"Accuracy"</option>
                    <option value="speed">"Speed"</option>
                </select>
            </div>
            <div class="mm__bar">
                <div class="mm__filters" role="group">
                    <FilterTab filter=filter want=Filter::All key=I18nKey::VoicePttModelsAll />
                    <FilterTab filter=filter want=Filter::Family(ModelFamily::Standard) key=I18nKey::VoicePttModelsStandard />
                    <FilterTab filter=filter want=Filter::Family(ModelFamily::Quantized) key=I18nKey::VoicePttModelsQuantized />
                    <FilterTab filter=filter want=Filter::Family(ModelFamily::Turbo) key=I18nKey::VoicePttModelsTurbo />
                    <FilterTab filter=filter want=Filter::Family(ModelFamily::Large) key=I18nKey::VoicePttModelsLarge />
                </div>
            </div>

            <ul class="mm__list">
                <For
                    each=filtered
                    key=|m| (m.id.clone(), m.installed, m.partial_bytes)
                    children=move |m| {
                        view! {
                            <ModelCard
                                model=m
                                progress=progress
                                errors=errors
                                selected=selected
                                on_use=on_use
                            />
                        }
                    }
                />
            </ul>
        </div>
    }
}

#[component]
fn FilterTab(filter: RwSignal<Filter>, want: Filter, key: I18nKey) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="mm__filter"
            class:mm__filter--on=move || filter.get() == want
            on:click=move |_| filter.set(want)
        >
            {move || i18n.tr(key)()}
        </button>
    }
}

#[component]
fn ModelCard<U>(
    model: WhisperModelView,
    progress: RwSignal<HashMap<String, Prog>>,
    errors: RwSignal<HashMap<String, String>>,
    selected: Signal<Option<String>>,
    on_use: U,
) -> impl IntoView
where
    U: Fn(String) + Copy + Send + Sync + 'static,
{
    let i18n = expect_context::<I18nService>();
    // `StoredValue` is `Copy`, so the per-action closures below stay `Copy` and
    // can be reused across multiple reactive views.
    let id = StoredValue::new(model.id.clone());
    let installed_path = StoredValue::new(model.installed_path.clone());

    let prog = move || progress.get().get(&id.get_value()).copied();
    let err = move || errors.get().get(&id.get_value()).cloned();
    let is_active = move || {
        selected.get().is_some_and(|s| {
            installed_path.get_value().as_deref() == Some(s.as_str()) || s == id.get_value()
        })
    };

    let download = move |_| {
        spawn_local(async move {
            let _ = whisper_model_download(id.get_value()).await;
        });
    };
    let cancel = move |_| {
        spawn_local(async move {
            let _ = whisper_model_cancel(id.get_value()).await;
        });
    };
    let delete = move |_| {
        spawn_local(async move {
            let _ = whisper_model_delete(id.get_value()).await;
        });
    };
    let use_model = move |_| on_use(id.get_value());

    let size_label = format_size(model.size_bytes);
    let lang_key = if model.multilingual {
        I18nKey::VoicePttModelMultilingual
    } else {
        I18nKey::VoicePttModelEnOnly
    };
    let installed = model.installed;
    let has_partial = model.partial_bytes.is_some();
    let label = model.label.clone();
    let best_for = model.best_for.clone();
    let speed = model.speed_rating;
    let acc = model.accuracy_rating;

    view! {
        <li class="mm-card" class:mm-card--active=is_active>
            <div class="mm-card__main">
                <div class="mm-card__head">
                    <span class="mm-card__name">{label}</span>
                    <span class="mm-card__badge">{family_label(model.family)}</span>
                    <Show when=is_active>
                        <span class="mm-card__badge mm-card__badge--active">
                            {move || i18n.tr(I18nKey::VoicePttModelActive)()}
                        </span>
                    </Show>
                </div>
                <p class="mm-card__desc">{best_for}</p>
                <div class="mm-card__meta">
                    <span>{size_label}</span>
                    <span>{move || i18n.tr(lang_key)()}</span>
                    <span class="mm-card__rating">
                        {move || i18n.tr(I18nKey::VoicePttModelSpeed)()}" "{dots(speed)}
                    </span>
                    <span class="mm-card__rating">
                        {move || i18n.tr(I18nKey::VoicePttModelAccuracy)()}" "{dots(acc)}
                    </span>
                </div>
                <Show when=move || err().is_some()>
                    <p class="mm-card__error">{move || err().unwrap_or_default()}</p>
                </Show>
            </div>

            <div class="mm-card__actions">
                {move || {
                    if let Some(p) = prog() {
                        let pct = if p.total > 0 {
                            (p.received as f64 / p.total as f64 * 100.0).clamp(0.0, 100.0)
                        } else {
                            0.0
                        };
                        view! {
                            <div class="mm-card__progress">
                                <div class="mm-progress">
                                    <div class="mm-progress__bar" style:width=move || format!("{pct:.0}%")></div>
                                </div>
                                <span class="mm-card__speed">
                                    {format!("{:.0}% · {}", pct, format_speed(p.speed_bps))}
                                </span>
                                <button type="button" class="mm-btn mm-btn--ghost" on:click=cancel.clone()>
                                    {move || i18n.tr(I18nKey::VoicePttModelCancel)()}
                                </button>
                            </div>
                        }.into_any()
                    } else if installed {
                        view! {
                            <div class="mm-card__installed">
                                <span class="mm-card__badge mm-card__badge--ok">
                                    {move || i18n.tr(I18nKey::VoicePttModelInstalled)()}
                                </span>
                                <button type="button" class="mm-btn" on:click=use_model.clone()>
                                    {move || i18n.tr(I18nKey::VoicePttModelUse)()}
                                </button>
                                <button type="button" class="mm-btn mm-btn--danger" on:click=delete.clone()>
                                    {move || i18n.tr(I18nKey::VoicePttModelDelete)()}
                                </button>
                            </div>
                        }.into_any()
                    } else if has_partial {
                        view! {
                            <div class="mm-card__installed">
                                <button type="button" class="mm-btn mm-btn--primary" on:click=download.clone()>
                                    {move || i18n.tr(I18nKey::VoicePttModelResume)()}
                                </button>
                                <button type="button" class="mm-btn mm-btn--ghost" on:click=delete.clone()>
                                    {move || i18n.tr(I18nKey::VoicePttModelDiscard)()}
                                </button>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <button type="button" class="mm-btn mm-btn--primary" on:click=download.clone()>
                                {move || i18n.tr(I18nKey::VoicePttModelDownload)()}
                            </button>
                        }.into_any()
                    }
                }}
            </div>
        </li>
    }
}

fn family_label(f: ModelFamily) -> &'static str {
    match f {
        ModelFamily::Standard => "Standard",
        ModelFamily::Quantized => "Quantized",
        ModelFamily::Turbo => "Turbo",
        ModelFamily::Large => "Large",
    }
}

fn dots(n: u8) -> String {
    let n = n.min(5) as usize;
    let mut s = String::new();
    for _ in 0..n {
        s.push('●');
    }
    for _ in n..5 {
        s.push('○');
    }
    s
}

fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0)
    } else {
        format!("{mb:.0} MB")
    }
}

fn format_speed(bps: f64) -> String {
    let mbps = bps / 1_000_000.0;
    if mbps >= 1.0 {
        format!("{mbps:.1} MB/s")
    } else {
        format!("{:.0} KB/s", bps / 1000.0)
    }
}
