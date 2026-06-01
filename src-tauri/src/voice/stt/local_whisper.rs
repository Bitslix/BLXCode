//! Local whisper.cpp speech-to-text via the optional `whisper-rs` binding.
//!
//! The model is loaded **once** and kept warm in [`WhisperEngine`] (a process-
//! wide Tauri state). Inference runs on a blocking thread via the caller's
//! `spawn_blocking`; this module never blocks the async runtime itself.
//!
//! The whole heavy path is gated behind the `local-whisper` cargo feature so
//! cloud-only builds need no C/C++ toolchain. When the feature is off, the
//! engine compiles to a stub that returns a clear "not available" error.

use std::sync::Mutex;

use crate::voice::settings::WhisperQuality;

/// Warm whisper model holder. Reloads only when the selected model path
/// changes; otherwise reuses the in-memory context for every utterance.
///
/// `inner` is only read on the `local-whisper` path; the stub build keeps the
/// field so the public type is identical across builds.
#[derive(Default)]
pub struct WhisperEngine {
    #[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
    inner: Mutex<EngineInner>,
}

#[derive(Default)]
struct EngineInner {
    /// Path of the currently loaded model, if any.
    #[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
    loaded_path: Option<String>,
    #[cfg(feature = "local-whisper")]
    ctx: Option<whisper_rs::WhisperContext>,
}

impl WhisperEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// True when `path` is already loaded and warm.
    #[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
    pub fn is_loaded(&self, path: &str) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.loaded_path.clone())
            .as_deref()
            == Some(path)
    }
}

#[cfg(feature = "local-whisper")]
mod imp {
    use super::*;
    use whisper_rs::{
        FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    };

    impl WhisperEngine {
        /// Load `path` into memory if it isn't the currently warm model.
        pub fn ensure_loaded(&self, path: &str) -> Result<(), String> {
            if !std::path::Path::new(path).exists() {
                return Err(format!("Whisper-Modell nicht gefunden: {path}"));
            }
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| "whisper engine poisoned".to_string())?;
            if guard.loaded_path.as_deref() == Some(path) && guard.ctx.is_some() {
                return Ok(());
            }
            let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
                .map_err(|e| format!("Whisper-Modell konnte nicht geladen werden: {e}"))?;
            guard.ctx = Some(ctx);
            guard.loaded_path = Some(path.to_owned());
            Ok(())
        }

        /// Transcribe a mono 16 kHz f32 buffer with the warm model. Blocking;
        /// call from within `spawn_blocking`.
        pub fn transcribe_pcm(
            &self,
            pcm: &[f32],
            quality: WhisperQuality,
            language: Option<&str>,
        ) -> Result<String, String> {
            let guard = self
                .inner
                .lock()
                .map_err(|_| "whisper engine poisoned".to_string())?;
            let ctx = guard
                .ctx
                .as_ref()
                .ok_or_else(|| "Whisper-Modell ist nicht geladen.".to_string())?;
            let mut state = ctx
                .create_state()
                .map_err(|e| format!("whisper state: {e}"))?;

            let mut params = FullParams::new(sampling_for(quality));
            params.set_n_threads(threads_for(quality));
            params.set_translate(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            if let Some(lang) = language {
                if !lang.is_empty() {
                    params.set_language(Some(lang));
                }
            }

            state
                .full(params, pcm)
                .map_err(|e| format!("whisper transcribe: {e}"))?;

            let n = state
                .full_n_segments()
                .map_err(|e| format!("whisper segments: {e}"))?;
            let mut out = String::new();
            for i in 0..n {
                if let Ok(seg) = state.full_get_segment_text(i) {
                    out.push_str(&seg);
                }
            }
            Ok(out.trim().to_string())
        }
    }

    fn sampling_for(q: WhisperQuality) -> SamplingStrategy {
        match q {
            WhisperQuality::Fast => SamplingStrategy::Greedy { best_of: 1 },
            WhisperQuality::Balanced => SamplingStrategy::Greedy { best_of: 2 },
            WhisperQuality::Best => SamplingStrategy::BeamSearch {
                beam_size: 5,
                patience: -1.0,
            },
        }
    }

    fn threads_for(q: WhisperQuality) -> i32 {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        match q {
            WhisperQuality::Fast => (cores / 2).max(1),
            WhisperQuality::Balanced => (cores - 1).max(1),
            WhisperQuality::Best => cores.max(1),
        }
    }
}

#[cfg(not(feature = "local-whisper"))]
impl WhisperEngine {
    pub fn ensure_loaded(&self, _path: &str) -> Result<(), String> {
        Err(local_disabled())
    }

    pub fn transcribe_pcm(
        &self,
        _pcm: &[f32],
        _quality: WhisperQuality,
        _language: Option<&str>,
    ) -> Result<String, String> {
        Err(local_disabled())
    }
}

#[cfg(not(feature = "local-whisper"))]
fn local_disabled() -> String {
    "Lokales Whisper ist in diesem Build nicht aktiviert (Feature `local-whisper`).".to_string()
}
