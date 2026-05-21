use crate::tauri_bridge::{
    app_relaunch, app_version, is_tauri_shell, updater_check, updater_install_start,
    updater_poll_progress, UpdateCheckResponse, UpdateProgress,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, Copy)]
pub struct UpdateService {
    status: RwSignal<UpdateUiStatus>,
    current_version: RwSignal<String>,
    available_version: RwSignal<Option<String>>,
    notes: RwSignal<Option<String>>,
    phase: RwSignal<String>,
    progress_pct: RwSignal<Option<f64>>,
    speed_label: RwSignal<Option<String>>,
    message: RwSignal<Option<String>>,
    dialog_open: RwSignal<bool>,
    banner_visible: RwSignal<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateUiStatus {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Installing,
    Done,
    Error,
    DevUnavailable,
}

impl UpdateService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(UpdateUiStatus::Idle),
            current_version: RwSignal::new(String::new()),
            available_version: RwSignal::new(None),
            notes: RwSignal::new(None),
            phase: RwSignal::new("idle".into()),
            progress_pct: RwSignal::new(None),
            speed_label: RwSignal::new(None),
            message: RwSignal::new(None),
            dialog_open: RwSignal::new(false),
            banner_visible: RwSignal::new(false),
        }
    }

    pub fn status(&self) -> RwSignal<UpdateUiStatus> {
        self.status
    }

    pub fn current_version(&self) -> RwSignal<String> {
        self.current_version
    }

    pub fn available_version(&self) -> RwSignal<Option<String>> {
        self.available_version
    }

    pub fn notes(&self) -> RwSignal<Option<String>> {
        self.notes
    }

    pub fn progress_pct(&self) -> RwSignal<Option<f64>> {
        self.progress_pct
    }

    pub fn speed_label(&self) -> RwSignal<Option<String>> {
        self.speed_label
    }

    pub fn message(&self) -> RwSignal<Option<String>> {
        self.message
    }

    pub fn dialog_open(&self) -> RwSignal<bool> {
        self.dialog_open
    }

    pub fn banner_visible(&self) -> RwSignal<bool> {
        self.banner_visible
    }

    pub fn open_dialog(&self) {
        self.dialog_open.set(true);
        self.banner_visible.set(false);
    }

    pub fn close_dialog(&self) {
        self.dialog_open.set(false);
    }

    pub fn check_silent(&self) {
        self.check(false);
    }

    pub fn check_manual(&self) {
        self.check(true);
    }

    pub fn start_install(&self) {
        if matches!(
            self.status.get_untracked(),
            UpdateUiStatus::Downloading | UpdateUiStatus::Installing
        ) {
            return;
        }
        let service = *self;
        spawn_local(async move {
            match updater_install_start().await {
                Ok(progress) => {
                    service.apply_progress(progress);
                    service.poll_install_progress();
                }
                Err(err) => service.set_error(err),
            }
        });
    }

    pub fn relaunch(&self) {
        spawn_local(async move {
            let _ = app_relaunch().await;
        });
    }

    fn check(&self, manual: bool) {
        if !is_tauri_shell() {
            self.status.set(UpdateUiStatus::DevUnavailable);
            self.message
                .set(Some("Updater is only available in the desktop app.".into()));
            return;
        }
        let service = *self;
        self.status.set(UpdateUiStatus::Checking);
        self.message.set(None);
        spawn_local(async move {
            if let Ok(version) = app_version().await {
                service.current_version.set(version);
            }
            match updater_check().await {
                Ok(response) => service.apply_check(response, manual),
                Err(err) => service.set_error(err),
            }
        });
    }

    fn apply_check(&self, response: UpdateCheckResponse, manual: bool) {
        self.current_version.set(response.current_version);
        self.available_version.set(response.available_version);
        self.notes.set(response.notes);
        self.message.set(response.message);
        let _ = (&response.date, &response.target, &response.download_url);
        match response.status.as_str() {
            "available" => {
                self.status.set(UpdateUiStatus::Available);
                self.banner_visible.set(!manual);
                if manual {
                    self.dialog_open.set(true);
                }
            }
            "devUnavailable" => self.status.set(UpdateUiStatus::DevUnavailable),
            _ => self.status.set(UpdateUiStatus::UpToDate),
        }
    }

    fn poll_install_progress(&self) {
        let service = *self;
        spawn_local(async move {
            let mut last_bytes = 0;
            let mut last_ms = 0;
            loop {
                TimeoutFuture::new(120).await;
                match updater_poll_progress().await {
                    Ok(progress) => {
                        let busy = progress.busy;
                        service.apply_speed(&progress, &mut last_bytes, &mut last_ms);
                        service.apply_progress(progress);
                        if !busy {
                            break;
                        }
                    }
                    Err(err) => {
                        service.set_error(err);
                        break;
                    }
                }
            }
        });
    }

    fn apply_progress(&self, progress: UpdateProgress) {
        self.phase.set(progress.phase.clone());
        self.message.set(progress.error.clone());
        self.progress_pct.set(match progress.total_bytes {
            Some(total) if total > 0 => {
                Some(((progress.downloaded_bytes as f64 / total as f64) * 100.0).min(100.0))
            }
            _ => None,
        });
        self.status.set(match progress.phase.as_str() {
            "downloading" => UpdateUiStatus::Downloading,
            "installing" => UpdateUiStatus::Installing,
            "done" => UpdateUiStatus::Done,
            "error" => UpdateUiStatus::Error,
            _ => self.status.get_untracked(),
        });
    }

    fn apply_speed(&self, progress: &UpdateProgress, last_bytes: &mut u64, last_ms: &mut u64) {
        if *last_ms == 0 || progress.updated_at_ms <= *last_ms {
            *last_bytes = progress.downloaded_bytes;
            *last_ms = progress.updated_at_ms;
            return;
        }
        let delta_bytes = progress.downloaded_bytes.saturating_sub(*last_bytes);
        let delta_ms = progress.updated_at_ms - *last_ms;
        if delta_ms > 0 && delta_bytes > 0 {
            let bytes_per_sec = delta_bytes as f64 / (delta_ms as f64 / 1000.0);
            self.speed_label.set(Some(format_speed(bytes_per_sec)));
        }
        *last_bytes = progress.downloaded_bytes;
        *last_ms = progress.updated_at_ms;
    }

    fn set_error(&self, err: String) {
        self.status.set(UpdateUiStatus::Error);
        self.message.set(Some(err));
        self.banner_visible.set(false);
    }
}

fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_000_000.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1_000_000.0)
    } else {
        format!("{:.0} KB/s", bytes_per_sec / 1_000.0)
    }
}
