use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

const RELEASE_NOTES_REPO: &str = "Bitslix/BLXCode";
const RELEASE_NOTES_USER_AGENT: &str = "BLXCode post-update release notes";
const UPDATE_USER_AGENT: &str = "BLXCode updater channel resolver";
const UPDATE_SETTINGS_FILE: &str = "app_update_settings.json";

#[derive(Default)]
pub struct BlxUpdaterState {
    inner: Mutex<BlxUpdaterInner>,
}

#[derive(Default)]
struct BlxUpdaterInner {
    pending_update: Option<Update>,
    progress: UpdateProgress,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettings {
    #[serde(default)]
    pub channel: UpdateChannel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsView {
    pub channel: UpdateChannel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    pub status: String,
    pub channel: UpdateChannel,
    pub current_version: String,
    pub available_version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub target: Option<String>,
    pub download_url: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: String,
    pub busy: bool,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateReleaseNotesResponse {
    pub version: String,
    pub title: String,
    pub summary: String,
    pub sections: Vec<PostUpdateReleaseNotesSection>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateReleaseNotesSection {
    pub title: String,
    pub items: Vec<PostUpdateReleaseNotesItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateReleaseNotesItem {
    pub title: Option<String>,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct GithubReleasePayload {
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseListItem {
    tag_name: String,
    draft: bool,
}

impl Default for UpdateProgress {
    fn default() -> Self {
        Self {
            phase: "idle".into(),
            busy: false,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            updated_at_ms: now_ms(),
        }
    }
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn updater_settings_get(app: AppHandle) -> Result<UpdateSettingsView, String> {
    settings_view(&load_update_settings(&app)?)
}

#[tauri::command]
pub fn updater_settings_save(
    app: AppHandle,
    state: tauri::State<'_, BlxUpdaterState>,
    patch: UpdateSettingsView,
) -> Result<UpdateSettingsView, String> {
    let previous = load_update_settings(&app)?;
    let next = UpdateSettings {
        channel: patch.channel,
    };
    save_update_settings(&app, &next)?;
    if previous.channel != next.channel {
        let mut inner = state.inner.lock().map_err(lock_err)?;
        inner.pending_update = None;
        inner.progress = UpdateProgress::default();
    }
    settings_view(&next)
}

#[tauri::command]
pub async fn updater_check(
    app: AppHandle,
    state: tauri::State<'_, BlxUpdaterState>,
) -> Result<UpdateCheckResponse, String> {
    let settings = load_update_settings(&app)?;
    let channel = settings.channel;
    if cfg!(debug_assertions) {
        let current_version = app.package_info().version.to_string();
        let mut inner = state.inner.lock().map_err(lock_err)?;
        inner.pending_update = None;
        inner.progress = UpdateProgress {
            phase: "devUnavailable".into(),
            busy: false,
            error: None,
            ..UpdateProgress::default()
        };
        return Ok(UpdateCheckResponse {
            status: "devUnavailable".into(),
            channel,
            current_version,
            available_version: None,
            notes: None,
            date: None,
            target: None,
            download_url: None,
            message: Some("Updater is disabled in development builds.".into()),
        });
    }

    let current_version = app.package_info().version.to_string();
    set_progress(&state, "checking", true, 0, None, None)?;

    let builder = app.updater_builder().on_before_exit(|| {
        eprintln!("BLXCode is exiting before installing the update");
    });
    #[cfg(target_os = "macos")]
    let builder = builder.target("darwin-universal");

    let builder = match channel {
        UpdateChannel::Stable => builder,
        UpdateChannel::Beta => {
            let Some(endpoint) = beta_update_endpoint(&current_version).await? else {
                let mut inner = state.inner.lock().map_err(lock_err)?;
                inner.pending_update = None;
                inner.progress = UpdateProgress {
                    phase: "upToDate".into(),
                    busy: false,
                    updated_at_ms: now_ms(),
                    ..UpdateProgress::default()
                };
                return Ok(UpdateCheckResponse {
                    status: "upToDate".into(),
                    channel,
                    current_version,
                    available_version: None,
                    notes: None,
                    date: None,
                    target: None,
                    download_url: None,
                    message: None,
                });
            };
            builder
                .endpoints(vec![endpoint])
                .map_err(|err| updater_error(&state, err))?
        }
    };

    let update = builder
        .build()
        .map_err(|err| updater_error(&state, err))?
        .check()
        .await
        .map_err(|err| updater_error(&state, err))?;

    match update {
        Some(update) => {
            let response = UpdateCheckResponse {
                status: "available".into(),
                channel,
                current_version,
                available_version: Some(update.version.clone()),
                notes: update.body.clone(),
                date: update.date.map(|date| date.to_string()),
                target: Some(format!("{}-{}", update.target, std::env::consts::ARCH)),
                download_url: Some(update.download_url.to_string()),
                message: None,
            };
            let mut inner = state.inner.lock().map_err(lock_err)?;
            inner.pending_update = Some(update);
            inner.progress = UpdateProgress {
                phase: "available".into(),
                busy: false,
                updated_at_ms: now_ms(),
                ..UpdateProgress::default()
            };
            Ok(response)
        }
        None => {
            let mut inner = state.inner.lock().map_err(lock_err)?;
            inner.pending_update = None;
            inner.progress = UpdateProgress {
                phase: "upToDate".into(),
                busy: false,
                updated_at_ms: now_ms(),
                ..UpdateProgress::default()
            };
            Ok(UpdateCheckResponse {
                status: "upToDate".into(),
                channel,
                current_version,
                available_version: None,
                notes: None,
                date: None,
                target: None,
                download_url: None,
                message: None,
            })
        }
    }
}

fn update_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(UPDATE_SETTINGS_FILE))
}

fn load_update_settings(app: &AppHandle) -> Result<UpdateSettings, String> {
    let path = update_settings_path(app)?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(UpdateSettings::default()),
        Ok(raw) => serde_json::from_str::<UpdateSettings>(&raw)
            .map_err(|e| format!("parse update settings {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(UpdateSettings::default()),
        Err(e) => Err(format!("read update settings {}: {e}", path.display())),
    }
}

fn save_update_settings(app: &AppHandle, settings: &UpdateSettings) -> Result<(), String> {
    let path = update_settings_path(app)?;
    atomic_write_json(&path, settings)
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid update settings path {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("create update settings dir {}: {e}", parent.display()))?;
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(value)
        .map_err(|e| format!("encode update settings {}: {e}", path.display()))?;
    {
        let mut file =
            fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        file.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))
}

fn settings_view(settings: &UpdateSettings) -> Result<UpdateSettingsView, String> {
    Ok(UpdateSettingsView {
        channel: settings.channel,
    })
}

async fn beta_update_endpoint(current_version: &str) -> Result<Option<Url>, String> {
    let current = parse_release_semver(current_version)
        .map_err(|err| format!("parse current app version {current_version:?}: {err}"))?;
    let client = reqwest::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .build()
        .map_err(|err| format!("GitHub release client: {err}"))?;
    let url = format!("https://api.github.com/repos/{RELEASE_NOTES_REPO}/releases?per_page=100");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("fetch GitHub releases: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub releases returned {}", response.status()));
    }
    let releases = response
        .json::<Vec<GithubReleaseListItem>>()
        .await
        .map_err(|err| format!("parse GitHub releases: {err}"))?;

    let Some(tag) = select_beta_release_tag(&current, releases) else {
        return Ok(None);
    };
    let endpoint =
        format!("https://github.com/{RELEASE_NOTES_REPO}/releases/download/{tag}/latest.json");
    Url::parse(&endpoint)
        .map(Some)
        .map_err(|err| format!("build beta updater endpoint for {tag}: {err}"))
}

fn parse_release_semver(version: &str) -> Result<Version, semver::Error> {
    Version::parse(version.trim().trim_start_matches('v'))
}

fn select_beta_release_tag(
    current: &Version,
    releases: Vec<GithubReleaseListItem>,
) -> Option<String> {
    let mut candidates = releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = parse_release_semver(&release.tag_name).ok()?;
            (version > *current).then_some((version, release.tag_name))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|(a, _), (b, _)| b.cmp(a));
    candidates.into_iter().next().map(|(_, tag)| tag)
}

#[tauri::command]
pub fn updater_install_start(
    app: AppHandle,
    state: tauri::State<'_, BlxUpdaterState>,
) -> Result<UpdateProgress, String> {
    let update = {
        let mut inner = state.inner.lock().map_err(lock_err)?;
        if inner.progress.busy {
            return Err("An update task is already running.".into());
        }
        let Some(update) = inner.pending_update.take() else {
            return Err("No pending update is available.".into());
        };
        inner.progress = UpdateProgress {
            phase: "downloading".into(),
            busy: true,
            updated_at_ms: now_ms(),
            ..UpdateProgress::default()
        };
        update
    };

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let app_for_progress = app_for_task.clone();
        let result = update
            .download_and_install(
                move |chunk_len, total_bytes| {
                    let state = app_for_progress.state::<BlxUpdaterState>();
                    if let Ok(mut inner) = state.inner.lock() {
                        inner.progress.phase = "downloading".into();
                        inner.progress.busy = true;
                        inner.progress.downloaded_bytes += chunk_len as u64;
                        inner.progress.total_bytes = total_bytes;
                        inner.progress.error = None;
                        inner.progress.updated_at_ms = now_ms();
                    };
                },
                move || {
                    let state = app_for_task.state::<BlxUpdaterState>();
                    if let Ok(mut inner) = state.inner.lock() {
                        inner.progress.phase = "installing".into();
                        inner.progress.busy = true;
                        inner.progress.updated_at_ms = now_ms();
                    };
                },
            )
            .await;

        let state = app.state::<BlxUpdaterState>();
        if let Ok(mut inner) = state.inner.lock() {
            match result {
                Ok(()) => {
                    inner.progress.phase = "done".into();
                    inner.progress.busy = false;
                    inner.progress.error = None;
                }
                Err(err) => {
                    inner.progress.phase = "error".into();
                    inner.progress.busy = false;
                    inner.progress.error = Some(err.to_string());
                }
            }
            inner.progress.updated_at_ms = now_ms();
        };
    });

    updater_poll_progress(state)
}

#[tauri::command]
pub fn updater_poll_progress(
    state: tauri::State<'_, BlxUpdaterState>,
) -> Result<UpdateProgress, String> {
    let inner = state.inner.lock().map_err(lock_err)?;
    Ok(inner.progress.clone())
}

#[tauri::command]
pub fn app_relaunch(app: AppHandle) -> Result<(), String> {
    app.restart();
}

#[tauri::command]
pub async fn post_update_release_notes(
    version: String,
    channel: UpdateChannel,
) -> Result<PostUpdateReleaseNotesResponse, String> {
    let version = normalize_release_version(&version);
    let tag = format!("v{version}");
    let client = reqwest::Client::builder()
        .user_agent(RELEASE_NOTES_USER_AGENT)
        .build()
        .map_err(|err| format!("release notes client: {err}"))?;

    let user_notes_url = format!(
        "https://raw.githubusercontent.com/{RELEASE_NOTES_REPO}/{tag}/docs/releases/{tag}.md"
    );
    if let Ok(markdown) = fetch_text(&client, &user_notes_url).await {
        return Ok(parse_release_notes(&version, &markdown, "userNotes"));
    }

    if channel == UpdateChannel::Beta {
        if let Some(base_version) = prerelease_base_version(&version) {
            let base_tag = format!("v{base_version}");
            let base_notes_url = format!(
                "https://raw.githubusercontent.com/{RELEASE_NOTES_REPO}/{base_tag}/docs/releases/{base_tag}.md"
            );
            if let Ok(markdown) = fetch_text(&client, &base_notes_url).await {
                return Ok(parse_release_notes(&version, &markdown, "userNotesBase"));
            }
        }
    }

    let release_url =
        format!("https://api.github.com/repos/{RELEASE_NOTES_REPO}/releases/tags/{tag}");
    if let Ok(release) = fetch_github_release(&client, &release_url).await {
        if let Some(body) = release.body.filter(|body| !body.trim().is_empty()) {
            return Ok(parse_release_notes(&version, &body, "githubRelease"));
        }
    }

    Ok(fallback_release_notes(&version))
}

fn set_progress(
    state: &tauri::State<'_, BlxUpdaterState>,
    phase: &str,
    busy: bool,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    error: Option<String>,
) -> Result<(), String> {
    let mut inner = state.inner.lock().map_err(lock_err)?;
    inner.progress = UpdateProgress {
        phase: phase.into(),
        busy,
        downloaded_bytes,
        total_bytes,
        error,
        updated_at_ms: now_ms(),
    };
    Ok(())
}

fn updater_error<E: std::fmt::Display>(
    state: &tauri::State<'_, BlxUpdaterState>,
    err: E,
) -> String {
    let message = err.to_string();
    let _ = set_progress(state, "error", false, 0, None, Some(message.clone()));
    message
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("fetch release notes: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("release notes returned {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|err| format!("read release notes: {err}"))
}

async fn fetch_github_release(
    client: &reqwest::Client,
    url: &str,
) -> Result<GithubReleasePayload, String> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("fetch GitHub release: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub release returned {}", response.status()));
    }
    response
        .json::<GithubReleasePayload>()
        .await
        .map_err(|err| format!("parse GitHub release: {err}"))
}

fn normalize_release_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn prerelease_base_version(version: &str) -> Option<&str> {
    version.split_once("-pre.").map(|(base, _)| base)
}

fn parse_release_notes(
    version: &str,
    markdown: &str,
    source: &str,
) -> PostUpdateReleaseNotesResponse {
    let (frontmatter, body) = split_frontmatter(markdown);
    let mut title = frontmatter_value(frontmatter, "title")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("BLXCode {version} is ready"));
    let summary = frontmatter_value(frontmatter, "summary")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Here are the most important changes in this update.".into());
    let sections = parse_release_sections(body);

    if title.trim().is_empty() {
        title = format!("BLXCode {version} is ready");
    }

    PostUpdateReleaseNotesResponse {
        version: version.to_string(),
        title,
        summary,
        sections: if sections.is_empty() {
            fallback_release_sections()
        } else {
            sections
        },
        source: source.into(),
    }
}

fn split_frontmatter(markdown: &str) -> (Option<&str>, &str) {
    let trimmed = markdown.trim_start();
    let Some(rest) = trimmed.strip_prefix("---") else {
        return (None, markdown);
    };
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let Some(end) = rest.find("\n---") else {
        return (None, markdown);
    };
    let frontmatter = &rest[..end];
    let body = &rest[end + "\n---".len()..];
    (Some(frontmatter), body.trim_start_matches(['\r', '\n']))
}

fn frontmatter_value(frontmatter: Option<&str>, key: &str) -> Option<String> {
    let frontmatter = frontmatter?;
    let prefix = format!("{key}:");
    frontmatter.lines().find_map(|line| {
        let line = line.trim();
        let raw = line.strip_prefix(&prefix)?.trim();
        Some(unquote(raw))
    })
}

fn parse_release_sections(markdown: &str) -> Vec<PostUpdateReleaseNotesSection> {
    let mut sections = Vec::<PostUpdateReleaseNotesSection>::new();
    let mut current_title: Option<String> = None;
    let mut current_items = Vec::<PostUpdateReleaseNotesItem>::new();
    let mut pending_paragraph = Vec::<String>::new();

    for line in markdown.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush_paragraph(&mut pending_paragraph, &mut current_items);
            continue;
        }
        if let Some(title) = line.strip_prefix("## ") {
            flush_paragraph(&mut pending_paragraph, &mut current_items);
            push_section(&mut sections, &mut current_title, &mut current_items);
            current_title = Some(clean_inline_markdown(title.trim()));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush_paragraph(&mut pending_paragraph, &mut current_items);
            current_items.push(parse_release_item(item));
            continue;
        }
        pending_paragraph.push(line.to_string());
    }

    flush_paragraph(&mut pending_paragraph, &mut current_items);
    push_section(&mut sections, &mut current_title, &mut current_items);
    sections
}

fn push_section(
    sections: &mut Vec<PostUpdateReleaseNotesSection>,
    current_title: &mut Option<String>,
    current_items: &mut Vec<PostUpdateReleaseNotesItem>,
) {
    if current_items.is_empty() {
        return;
    }
    sections.push(PostUpdateReleaseNotesSection {
        title: current_title.take().unwrap_or_else(|| "Highlights".into()),
        items: std::mem::take(current_items),
    });
}

fn flush_paragraph(
    pending_paragraph: &mut Vec<String>,
    current_items: &mut Vec<PostUpdateReleaseNotesItem>,
) {
    if pending_paragraph.is_empty() {
        return;
    }
    let body = clean_inline_markdown(&pending_paragraph.join(" "));
    if !body.is_empty() {
        current_items.push(PostUpdateReleaseNotesItem { title: None, body });
    }
    pending_paragraph.clear();
}

fn parse_release_item(raw: &str) -> PostUpdateReleaseNotesItem {
    let raw = raw.trim();
    if let Some(stripped) = raw.strip_prefix("**") {
        if let Some(end) = stripped.find("**:") {
            let title = clean_inline_markdown(&stripped[..end]);
            let body = clean_inline_markdown(stripped[end + 3..].trim());
            return PostUpdateReleaseNotesItem {
                title: Some(title),
                body,
            };
        }
    }
    PostUpdateReleaseNotesItem {
        title: None,
        body: clean_inline_markdown(raw),
    }
}

fn clean_inline_markdown(value: &str) -> String {
    value
        .replace("**", "")
        .replace('`', "")
        .replace('[', "")
        .replace(']', "")
        .trim()
        .to_string()
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn fallback_release_notes(version: &str) -> PostUpdateReleaseNotesResponse {
    PostUpdateReleaseNotesResponse {
        version: version.into(),
        title: format!("BLXCode {version} is ready"),
        summary: "This version is installed. Release notes could not be loaded right now.".into(),
        sections: fallback_release_sections(),
        source: "fallback".into(),
    }
}

fn fallback_release_sections() -> Vec<PostUpdateReleaseNotesSection> {
    vec![PostUpdateReleaseNotesSection {
        title: "Good to know".into(),
        items: vec![PostUpdateReleaseNotesItem {
            title: None,
            body: "You can keep working normally. BLXCode will try to load richer release notes next time if they were not available.".into(),
        }],
    }]
}

fn lock_err<E: std::fmt::Display>(err: E) -> String {
    format!("updater state lock poisoned: {err}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_sections() {
        let notes = parse_release_notes(
            "0.2.9",
            r#"---
title: "A smoother workspace"
summary: "Daily coding feels calmer."
---

## Highlights

- Move terminals without losing sessions.

## New

- **Terminal drag and drop**: Reorder terminal slots.
"#,
            "userNotes",
        );

        assert_eq!(notes.title, "A smoother workspace");
        assert_eq!(notes.summary, "Daily coding feels calmer.");
        assert_eq!(notes.sections.len(), 2);
        assert_eq!(notes.sections[0].title, "Highlights");
        assert_eq!(
            notes.sections[0].items[0].body,
            "Move terminals without losing sessions."
        );
        assert_eq!(
            notes.sections[1].items[0].title.as_deref(),
            Some("Terminal drag and drop")
        );
        assert_eq!(notes.sections[1].items[0].body, "Reorder terminal slots.");
    }

    #[test]
    fn falls_back_for_empty_body() {
        let notes = parse_release_notes("0.2.9", "", "userNotes");

        assert_eq!(notes.version, "0.2.9");
        assert_eq!(notes.sections.len(), 1);
        assert_eq!(notes.sections[0].title, "Good to know");
    }

    #[test]
    fn normalizes_release_version() {
        assert_eq!(normalize_release_version("v0.2.9"), "0.2.9");
        assert_eq!(normalize_release_version(" 0.2.9 "), "0.2.9");
    }

    #[test]
    fn extracts_prerelease_base_version() {
        assert_eq!(prerelease_base_version("0.6.0-pre.ed4dc"), Some("0.6.0"));
        assert_eq!(prerelease_base_version("0.6.0"), None);
    }

    #[test]
    fn update_settings_default_to_stable_channel() {
        let settings: UpdateSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.channel, UpdateChannel::Stable);
    }

    #[test]
    fn parses_prerelease_versions() {
        let version = parse_release_semver("v0.6.0-pre.ed4dc").unwrap();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 6);
        assert_eq!(version.patch, 0);
        assert_eq!(version.pre.as_str(), "pre.ed4dc");
    }

    #[test]
    fn beta_channel_picks_highest_prerelease_or_newer_final() {
        let current = parse_release_semver("0.6.0-pre.11111").unwrap();
        let tag = select_beta_release_tag(
            &current,
            vec![
                GithubReleaseListItem {
                    tag_name: "v0.6.0-pre.22222".into(),
                    draft: false,
                },
                GithubReleaseListItem {
                    tag_name: "v0.6.0".into(),
                    draft: false,
                },
                GithubReleaseListItem {
                    tag_name: "v0.7.0-pre.ed4dc".into(),
                    draft: false,
                },
                GithubReleaseListItem {
                    tag_name: "v0.8.0-pre.abcde".into(),
                    draft: true,
                },
            ],
        );

        assert_eq!(tag.as_deref(), Some("v0.7.0-pre.ed4dc"));
    }
}
