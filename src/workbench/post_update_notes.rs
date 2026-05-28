use crate::config::POST_UPDATE_NOTES_SEEN_VERSION_KEY;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    app_version, is_tauri_shell, post_update_release_notes, PostUpdateReleaseNotesItem,
    PostUpdateReleaseNotesResponse, PostUpdateReleaseNotesSection,
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[derive(Clone, Copy)]
pub struct PostUpdateNotesService {
    open: RwSignal<bool>,
    loading: RwSignal<bool>,
    checked: RwSignal<bool>,
    current_version: RwSignal<String>,
    notes: RwSignal<Option<PostUpdateReleaseNotesResponse>>,
}

impl PostUpdateNotesService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            loading: RwSignal::new(false),
            checked: RwSignal::new(false),
            current_version: RwSignal::new(String::new()),
            notes: RwSignal::new(None),
        }
    }

    pub fn open(&self) -> RwSignal<bool> {
        self.open
    }

    pub fn loading(&self) -> RwSignal<bool> {
        self.loading
    }

    pub fn notes(&self) -> RwSignal<Option<PostUpdateReleaseNotesResponse>> {
        self.notes
    }

    pub fn check_after_start(&self) {
        if self.checked.get_untracked() || !is_tauri_shell() {
            return;
        }
        self.checked.set(true);
        let service = *self;
        spawn_local(async move {
            let Ok(version) = app_version().await else {
                return;
            };
            service.current_version.set(version.clone());
            if read_seen_version().as_deref() == Some(version.as_str()) {
                return;
            }
            service.loading.set(true);
            service.open.set(true);
            match post_update_release_notes(version).await {
                Ok(notes) => service.notes.set(Some(notes)),
                Err(err) => {
                    leptos::logging::warn!("post_update_release_notes: {err}");
                    service.open.set(false);
                }
            }
            service.loading.set(false);
        });
    }

    pub fn acknowledge(&self) {
        if self.loading.get_untracked() && self.notes.get_untracked().is_none() {
            return;
        }
        let version = self
            .notes
            .get_untracked()
            .map(|notes| notes.version)
            .filter(|version| !version.trim().is_empty())
            .unwrap_or_else(|| self.current_version.get_untracked());
        if !version.trim().is_empty() {
            write_seen_version(&version);
        }
        self.open.set(false);
    }
}

#[component]
pub fn PostUpdateNotesDialog() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let notes = expect_context::<PostUpdateNotesService>();

    view! {
        <Show when=move || notes.open().get()>
            <div class="harness-overlay harness-overlay--modal" role="presentation">
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::PostUpdateGotIt)()
                    on:click=move |_| notes.acknowledge()
                ></button>
                <section
                    class="harness-sheet harness-sheet--post-update"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" {
                            ev.prevent_default();
                            notes.acknowledge();
                        }
                    }
                >
                    <Show
                        when=move || notes.notes().get().is_some()
                        fallback=move || view! {
                            <div class="post-update-loading">
                                <span class="post-update-loading__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuLoaderCircle width="1rem" height="1rem" />
                                </span>
                                <span>{move || {
                                    if notes.loading().get() {
                                        i18n.tr(I18nKey::PostUpdateLoading)()
                                    } else {
                                        i18n.tr(I18nKey::PostUpdateNoNotes)()
                                    }
                                }}</span>
                            </div>
                        }
                    >
                        {move || {
                            notes.notes().get().map(|data| view! {
                                <PostUpdateNotesContent data=data />
                            })
                        }}
                    </Show>
                    <footer class="post-update-actions">
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            on:click=move |_| notes.acknowledge()
                        >
                            <span class="harness-btn-inline">
                                <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
                                <span>{move || i18n.tr(I18nKey::PostUpdateGotIt)()}</span>
                            </span>
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}

#[component]
fn PostUpdateNotesContent(data: PostUpdateReleaseNotesResponse) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let version = data.version.clone();
    let title = data.title.clone();
    let summary = data.summary.clone();
    let sections = data.sections.clone();
    let _source = data.source.clone();

    view! {
        <header class="post-update-hero">
            <div class="post-update-hero__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuSparkles width="1.25rem" height="1.25rem" />
            </div>
            <div class="post-update-hero__copy">
                <div class="post-update-hero__eyebrow">
                    <span>{move || i18n.tr(I18nKey::PostUpdateEyebrow)()}</span>
                    <span class="post-update-version">{"v"}{version.clone()}</span>
                </div>
                <h2>{title}</h2>
                <p>{summary}</p>
            </div>
        </header>
        <div class="post-update-section-list">
            <For
                each=move || sections.clone()
                key=|section| section.title.clone()
                children=move |section| view! { <PostUpdateSection section=section /> }
            />
        </div>
    }
}

#[component]
fn PostUpdateSection(section: PostUpdateReleaseNotesSection) -> impl IntoView {
    let variant = section_variant(&section.title);
    let title = section.title.clone();
    let items = section.items.clone();

    view! {
        <section class=format!("post-update-section post-update-section--{variant}")>
            <div class="post-update-section__head">
                <span class="post-update-section__icon" aria-hidden="true">
                    <LxIcon icon=section_icon(&title) width="0.95rem" height="0.95rem" />
                </span>
                <h3>{title}</h3>
            </div>
            <ul class="post-update-items">
                <For
                    each=move || items.clone()
                    key=|item| format!("{}:{}", item.title.clone().unwrap_or_default(), item.body)
                    children=move |item| view! { <PostUpdateItem item=item /> }
                />
            </ul>
        </section>
    }
}

#[component]
fn PostUpdateItem(item: PostUpdateReleaseNotesItem) -> impl IntoView {
    let title = item.title.clone();
    let has_title = title.is_some();
    let body = item.body.clone();
    view! {
        <li class="post-update-item">
            <span class="post-update-item__dot" aria-hidden="true"></span>
            <span class="post-update-item__copy">
                <Show when=move || has_title>
                    <strong>{title.clone().unwrap_or_default()}</strong>
                </Show>
                <span>{body}</span>
            </span>
        </li>
    }
}

fn section_variant(title: &str) -> &'static str {
    match title.to_ascii_lowercase().as_str() {
        "highlights" => "highlights",
        "new" | "added" => "new",
        "improved" | "changed" => "improved",
        "fixed" | "security" => "fixed",
        "removed" => "removed",
        _ => "neutral",
    }
}

fn section_icon(title: &str) -> icondata::Icon {
    match section_variant(title) {
        "highlights" => icondata::LuSparkles,
        "new" => icondata::LuPlus,
        "improved" => icondata::LuWrench,
        "fixed" => icondata::LuShieldCheck,
        "removed" => icondata::LuTrash2,
        _ => icondata::LuPackage,
    }
}

fn read_seen_version() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|storage| {
            storage
                .get_item(POST_UPDATE_NOTES_SEEN_VERSION_KEY)
                .ok()
                .flatten()
        })
}

fn write_seen_version(version: &str) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let _ = storage.set_item(POST_UPDATE_NOTES_SEEN_VERSION_KEY, version);
}
