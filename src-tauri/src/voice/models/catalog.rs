//! Static catalog of downloadable whisper.cpp models.
//!
//! Source files are the official GGML weights from the whisper.cpp repo on
//! Hugging Face (`ggerganov/whisper.cpp`). The catalog is purely declarative:
//! installed-state is computed at runtime from the models directory, never
//! stored here.
//!
//! `sha256` may be empty for an entry, in which case integrity verification is
//! skipped for that model. TODO: fill in the published checksums so every
//! download is verified.

/// Coarse grouping used by the model-manager filter tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelFamily {
    Standard,
    Quantized,
    Turbo,
    Large,
}

/// One downloadable model preset.
#[derive(Clone, Debug)]
pub struct WhisperModel {
    pub id: &'static str,
    pub label: &'static str,
    pub family: ModelFamily,
    pub multilingual: bool,
    pub size_bytes: u64,
    pub url: &'static str,
    pub sha256: &'static str,
    pub speed_rating: u8,
    pub accuracy_rating: u8,
    /// Short "best for …" hint (English source; localized in the UI layer).
    pub best_for: &'static str,
}

const fn hf(name: &'static str) -> &'static str {
    name
}

/// The full catalog. Sizes are approximate (used for the progress total
/// fallback and the size label).
pub fn catalog() -> &'static [WhisperModel] {
    &CATALOG
}

/// Look up a model by id.
pub fn find(id: &str) -> Option<&'static WhisperModel> {
    CATALOG.iter().find(|m| m.id == id)
}

const BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

// NOTE: `url` is built at runtime via `model_url()` to keep this table compact.
static CATALOG: [WhisperModel; 9] = [
    WhisperModel {
        id: "tiny",
        label: "Tiny",
        family: ModelFamily::Standard,
        multilingual: true,
        size_bytes: 77_700_000,
        url: hf("ggml-tiny.bin"),
        sha256: "",
        speed_rating: 5,
        accuracy_rating: 1,
        best_for: "Quick tests, very old hardware",
    },
    WhisperModel {
        id: "tiny-q8",
        label: "Tiny Q8",
        family: ModelFamily::Quantized,
        multilingual: true,
        size_bytes: 43_500_000,
        url: hf("ggml-tiny-q8_0.bin"),
        sha256: "",
        speed_rating: 5,
        accuracy_rating: 1,
        best_for: "Minimal RAM, embedded / low-end devices",
    },
    WhisperModel {
        id: "tiny.en",
        label: "Tiny (EN only)",
        family: ModelFamily::Standard,
        multilingual: false,
        size_bytes: 77_700_000,
        url: hf("ggml-tiny.en.bin"),
        sha256: "",
        speed_rating: 5,
        accuracy_rating: 1,
        best_for: "English-only, minimal resources",
    },
    WhisperModel {
        id: "base",
        label: "Base",
        family: ModelFamily::Standard,
        multilingual: true,
        size_bytes: 147_900_000,
        url: hf("ggml-base.bin"),
        sha256: "",
        speed_rating: 5,
        accuracy_rating: 2,
        best_for: "Daily dictation, DE + EN",
    },
    WhisperModel {
        id: "base-q8",
        label: "Base Q8",
        family: ModelFamily::Quantized,
        multilingual: true,
        size_bytes: 81_800_000,
        url: hf("ggml-base-q8_0.bin"),
        sha256: "",
        speed_rating: 5,
        accuracy_rating: 2,
        best_for: "Best size/quality ratio for Base",
    },
    WhisperModel {
        id: "small",
        label: "Small",
        family: ModelFamily::Standard,
        multilingual: true,
        size_bytes: 487_600_000,
        url: hf("ggml-small.bin"),
        sha256: "",
        speed_rating: 4,
        accuracy_rating: 3,
        best_for: "Noticeably better accuracy, still real-time on modern CPUs",
    },
    WhisperModel {
        id: "medium",
        label: "Medium",
        family: ModelFamily::Standard,
        multilingual: true,
        size_bytes: 1_530_000_000,
        url: hf("ggml-medium.bin"),
        sha256: "",
        speed_rating: 2,
        accuracy_rating: 4,
        best_for: "High accuracy, slower; strong multilingual",
    },
    WhisperModel {
        id: "large-v3-turbo",
        label: "Large v3 Turbo",
        family: ModelFamily::Turbo,
        multilingual: true,
        size_bytes: 1_620_000_000,
        url: hf("ggml-large-v3-turbo.bin"),
        sha256: "",
        speed_rating: 3,
        accuracy_rating: 5,
        best_for: "Near-large accuracy at much higher speed",
    },
    WhisperModel {
        id: "large-v3",
        label: "Large v3",
        family: ModelFamily::Large,
        multilingual: true,
        size_bytes: 3_100_000_000,
        url: hf("ggml-large-v3.bin"),
        sha256: "",
        speed_rating: 1,
        accuracy_rating: 5,
        best_for: "Best accuracy, needs strong hardware",
    },
];

/// Absolute download URL for a model.
pub fn model_url(m: &WhisperModel) -> String {
    format!("{BASE}{}", m.url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let mut seen = HashSet::new();
        for m in catalog() {
            assert!(seen.insert(m.id), "duplicate model id: {}", m.id);
        }
    }

    #[test]
    fn entries_are_well_formed() {
        for m in catalog() {
            assert!(!m.label.is_empty(), "{} missing label", m.id);
            assert!(m.url.ends_with(".bin"), "{} url not a .bin", m.id);
            assert!(model_url(m).starts_with("https://"), "{} bad url", m.id);
            assert!((1..=5).contains(&m.speed_rating), "{} speed", m.id);
            assert!((1..=5).contains(&m.accuracy_rating), "{} accuracy", m.id);
            assert!(m.size_bytes > 0, "{} size", m.id);
        }
    }

    #[test]
    fn find_works() {
        assert!(find("base").is_some());
        assert!(find("does-not-exist").is_none());
    }
}
