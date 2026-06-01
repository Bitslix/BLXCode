//! Terminal title naming: render slots either as native slot numbers
//! (`#3`) or as friendly agent-style names (`Devon`) drawn from an
//! editable pool. The `slot_id` always remains the technical identity
//! (terminal_key, sessions.json, PTY routing); names are a pure
//! display/addressing layer resolved entirely client-side.

use crate::config::{TERMINAL_NAMING_MODE_KEY, TERMINAL_NAME_POOL_KEY};

/// Storage token for the "names" mode.
const MODE_NAMES: &str = "names";
/// Storage token for the "slots" (native numbers) mode.
const MODE_SLOTS: &str = "slots";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalNamingMode {
    /// Show the native `#slot_id` (default, back-compat).
    SlotNumbers,
    /// Show a friendly name from the pool / per-slot override.
    Names,
}

impl TerminalNamingMode {
    #[must_use]
    pub fn from_storage(value: Option<&str>) -> Self {
        match value {
            Some(MODE_NAMES) => Self::Names,
            _ => Self::SlotNumbers,
        }
    }

    #[must_use]
    pub const fn storage_value(self) -> &'static str {
        match self {
            Self::Names => MODE_NAMES,
            Self::SlotNumbers => MODE_SLOTS,
        }
    }
}

/// The localStorage keys are re-exported so callers (AppPrefsService) don't
/// have to import the config module twice.
pub const NAMING_MODE_KEY: &str = TERMINAL_NAMING_MODE_KEY;
pub const NAME_POOL_KEY: &str = TERMINAL_NAME_POOL_KEY;

/// Resolve the friendly name for a slot, ignoring the active mode.
///
/// Priority: a non-empty per-slot `override_name` always wins. Otherwise a
/// deterministic, collision-free name is drawn from `pool` based on the
/// slot's stable `slot_id` and the set of sibling slot ids in the same
/// workspace. Lower slot ids keep their base name on collision, so a slot's
/// auto name stays stable when *other* slots come and go. Returns `None`
/// when no override exists and the pool is empty.
#[must_use]
pub fn resolve_slot_name(
    slot_id: u64,
    override_name: Option<&str>,
    pool: &[String],
    siblings: &[u64],
) -> Option<String> {
    if let Some(name) = override_name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    auto_name(slot_id, pool, siblings)
}

/// Deterministic pool assignment via `id % len` with linear probing over the
/// ascending-sorted sibling set so names are unique within one pool length.
fn auto_name(slot_id: u64, pool: &[String], siblings: &[u64]) -> Option<String> {
    let n = pool.len();
    if n == 0 {
        return None;
    }
    let mut ids: Vec<u64> = siblings.to_vec();
    if !ids.contains(&slot_id) {
        ids.push(slot_id);
    }
    ids.sort_unstable();
    ids.dedup();

    let mut used = vec![false; n];
    for id in ids {
        let base = (id as usize) % n;
        let mut idx = base;
        for _ in 0..n {
            if !used[idx] {
                break;
            }
            idx = (idx + 1) % n;
        }
        used[idx] = true;
        if id == slot_id {
            return Some(pool[idx].clone());
        }
    }
    // Unreachable in practice (slot_id is always inserted above).
    Some(pool[(slot_id as usize) % n].clone())
}

/// The label shown in the terminal header for the given mode.
///
/// `SlotNumbers` always renders `#slot_id`. `Names` renders the resolved
/// friendly name, falling back to `#slot_id` when no name is available
/// (empty pool and no override).
#[must_use]
pub fn display_label(
    mode: TerminalNamingMode,
    slot_id: u64,
    override_name: Option<&str>,
    pool: &[String],
    siblings: &[u64],
) -> String {
    match mode {
        TerminalNamingMode::SlotNumbers => format!("#{slot_id}"),
        TerminalNamingMode::Names => resolve_slot_name(slot_id, override_name, pool, siblings)
            .unwrap_or_else(|| format!("#{slot_id}")),
    }
}

/// Parse the persisted pool (JSON string array). Falls back to the default
/// pool on missing/invalid input. Blank entries are dropped and surrounding
/// whitespace trimmed.
#[must_use]
pub fn parse_pool(raw: Option<&str>) -> Vec<String> {
    let parsed = raw
        .and_then(|r| serde_json::from_str::<Vec<String>>(r).ok())
        .map(|v| {
            v.into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        });
    match parsed {
        Some(v) if !v.is_empty() => v,
        _ => default_pool(),
    }
}

/// The built-in default name pool.
#[must_use]
pub fn default_pool() -> Vec<String> {
    crate::config::DEFAULT_TERMINAL_NAME_POOL
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// Serialize a pool back to the persisted JSON form.
#[must_use]
pub fn serialize_pool(pool: &[String]) -> String {
    serde_json::to_string(pool).unwrap_or_else(|_| "[]".to_string())
}
