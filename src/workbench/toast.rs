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
}

impl ToastService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: RwSignal::new(Vec::new()),
        }
    }

    pub fn success(&self, message: impl Into<String>) {
        let prefs = expect_context::<AppPrefsService>();
        if !prefs.success_toast_enabled().get_untracked() {
            return;
        }
        self.push(ToastKind::Success, message.into());
    }

    /// Error toasts are always shown (independent of the success-toast toggle).
    pub fn error(&self, message: impl Into<String>) {
        self.push(ToastKind::Error, message.into());
    }

    fn push(&self, kind: ToastKind, message: String) {
        let id = NEXT_TOAST_ID.fetch_add(1, Ordering::Relaxed);
        let item = ToastItem { id, message, kind };
        self.items.update(|list| {
            list.push(item);
            if list.len() > MAX_VISIBLE {
                let drop = list.len() - MAX_VISIBLE;
                list.drain(0..drop);
            }
        });
        let items = self.items;
        spawn_local(async move {
            TimeoutFuture::new(TOAST_TTL_MS).await;
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
                    };
                    view! {
                        <div class=class role="status">{t.message}</div>
                    }
                }
            />
        </div>
    }
}
