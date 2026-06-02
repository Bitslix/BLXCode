//! Lightweight toast stack (Sonner-style) for transient action feedback.

use crate::workbench::app_prefs::AppPrefsService;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::atomic::{AtomicU64, Ordering};

const TOAST_TTL_MS: u32 = 3500;
const MAX_VISIBLE: usize = 3;

static NEXT_TOAST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
    Loading,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastItem {
    pub id: u64,
    pub message: String,
    pub kind: ToastKind,
}

#[derive(Clone, Copy)]
pub struct ToastService {
    items: RwSignal<Vec<ToastItem>>,
    prefs: AppPrefsService,
}

impl ToastService {
    #[must_use]
    pub fn new(prefs: AppPrefsService) -> Self {
        Self {
            items: RwSignal::new(Vec::new()),
            prefs,
        }
    }

    pub fn success(&self, message: impl Into<String>) {
        // Read the stored prefs handle rather than `expect_context`: toasts are
        // frequently emitted from inside `spawn_local` futures where the
        // reactive owner (and thus context) is no longer present.
        if !self.prefs.success_toast_enabled().get_untracked() {
            return;
        }
        self.push_with_timeout(ToastKind::Success, message.into(), Some(TOAST_TTL_MS));
    }

    /// Error toasts are always shown (independent of the success-toast toggle).
    pub fn error(&self, message: impl Into<String>) {
        self.push_with_timeout(ToastKind::Error, message.into(), Some(TOAST_TTL_MS));
    }

    pub fn info(&self, message: impl Into<String>) {
        self.push_with_timeout(ToastKind::Info, message.into(), Some(TOAST_TTL_MS));
    }

    pub fn loading(&self, message: impl Into<String>) -> u64 {
        self.push_with_timeout(ToastKind::Loading, message.into(), None)
    }

    pub fn resolve(&self, id: u64, kind: ToastKind, message: impl Into<String>) {
        self.items.update(|list| {
            if let Some(item) = list.iter_mut().find(|item| item.id == id) {
                item.kind = kind;
                item.message = message.into();
            }
        });
        Self::schedule_remove(self.items, id, TOAST_TTL_MS);
    }

    pub fn dismiss(&self, id: u64) {
        self.items.update(|list| list.retain(|t| t.id != id));
    }

    fn push_with_timeout(&self, kind: ToastKind, message: String, timeout_ms: Option<u32>) -> u64 {
        let id = NEXT_TOAST_ID.fetch_add(1, Ordering::Relaxed);
        let item = ToastItem { id, message, kind };
        self.items.update(|list| {
            list.push(item);
            if list.len() > MAX_VISIBLE {
                let drop = list.len() - MAX_VISIBLE;
                list.drain(0..drop);
            }
        });
        if let Some(timeout_ms) = timeout_ms {
            Self::schedule_remove(self.items, id, timeout_ms);
        }
        id
    }

    fn schedule_remove(items: RwSignal<Vec<ToastItem>>, id: u64, timeout_ms: u32) {
        spawn_local(async move {
            TimeoutFuture::new(timeout_ms).await;
            items.update(|list| list.retain(|t| t.id != id));
        });
    }
}

#[component]
pub fn ToastHost() -> impl IntoView {
    let toast = expect_context::<ToastService>();
    view! {
        <div class="blx-toast-host" aria-live="polite">
            <For
                each=move || toast.items.get()
                key=|t| t.id
                children=move |t: ToastItem| {
                    let class = match t.kind {
                        ToastKind::Success => "blx-toast blx-toast--success",
                        ToastKind::Error => "blx-toast blx-toast--error",
                        ToastKind::Info => "blx-toast blx-toast--info",
                        ToastKind::Loading => "blx-toast blx-toast--loading",
                    };
                    view! {
                        <div class=class role="status">{t.message}</div>
                    }
                }
            />
        </div>
    }
}
