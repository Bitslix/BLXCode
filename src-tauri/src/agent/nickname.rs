//! Agent-nickname resolution and validation.
//!
//! The user may give the agent a personal name in Settings → Agent. An empty
//! value means "use the default" ([`DEFAULT_AGENT_NICKNAME`]). Names are checked
//! against the built-in, non-user-editable [`badwords`](super::badwords) list
//! (with light leetspeak normalisation) and basic length/charset rules.

use super::badwords::BADWORDS;

/// Default agent name when the user has not set one.
pub const DEFAULT_AGENT_NICKNAME: &str = "BLXCodey";

/// Maximum length (in characters) of a user-chosen nickname.
pub const MAX_NICKNAME_LEN: usize = 32;

/// Below this compacted length a badword must match a whole token (not just a
/// substring) to avoid false positives like "class" → "ass".
const SHORT_BADWORD_LEN: usize = 3;

/// Why a nickname was rejected. [`Self::reason_code`] yields a stable string the
/// frontend maps to a localized message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NicknameError {
    TooLong,
    InvalidChars,
    BadWord,
}

impl NicknameError {
    /// Stable machine code surfaced over IPC for i18n mapping in the UI.
    #[must_use]
    pub fn reason_code(self) -> &'static str {
        match self {
            NicknameError::TooLong => "tooLong",
            NicknameError::InvalidChars => "invalidChars",
            NicknameError::BadWord => "badWord",
        }
    }
}

/// Resolve the effective agent name: the trimmed nickname, or the default when
/// blank.
#[must_use]
pub fn resolve_agent_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        DEFAULT_AGENT_NICKNAME.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Map common leetspeak digits/symbols to their letter so "4rsch" is caught.
fn deleet(c: char) -> char {
    match c {
        '4' | '@' => 'a',
        '3' => 'e',
        '1' | '!' => 'i',
        '0' => 'o',
        '5' | '$' => 's',
        '7' => 't',
        other => other,
    }
}

/// Validate a user-entered nickname. An empty/whitespace value is **allowed**
/// (it means "use the default") and returns `Ok` with the empty string, so the
/// caller stores blank rather than the literal default.
///
/// # Errors
/// Returns [`NicknameError`] when the name is too long, contains disallowed
/// characters, or matches the built-in badword list.
pub fn validate_nickname(raw: &str) -> Result<String, NicknameError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.chars().count() > MAX_NICKNAME_LEN {
        return Err(NicknameError::TooLong);
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
    {
        return Err(NicknameError::InvalidChars);
    }

    // Normalize for matching: lowercase + leetspeak.
    let normalized: String = trimmed.to_lowercase().chars().map(deleet).collect();
    // Compact form (alphanumerics only) for substring matching of longer terms.
    let compact: String = normalized.chars().filter(|c| c.is_alphanumeric()).collect();
    // Token form for whole-word matching of short terms.
    let tokens: Vec<String> = normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();

    for bw in BADWORDS {
        let bw_compact: String = bw
            .to_lowercase()
            .chars()
            .map(deleet)
            .filter(|c| c.is_alphanumeric())
            .collect();
        if bw_compact.is_empty() {
            continue;
        }
        let hit = if bw_compact.len() <= SHORT_BADWORD_LEN {
            tokens.iter().any(|t| t == &bw_compact)
        } else {
            compact.contains(&bw_compact)
        };
        if hit {
            return Err(NicknameError::BadWord);
        }
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_resolves_to_default() {
        assert_eq!(resolve_agent_name(""), DEFAULT_AGENT_NICKNAME);
        assert_eq!(resolve_agent_name("   "), DEFAULT_AGENT_NICKNAME);
        assert_eq!(resolve_agent_name("  Foo "), "Foo");
    }

    #[test]
    fn blank_is_valid_and_stored_empty() {
        assert_eq!(validate_nickname(""), Ok(String::new()));
        assert_eq!(validate_nickname("   "), Ok(String::new()));
    }

    #[test]
    fn default_name_is_clean() {
        assert_eq!(
            validate_nickname(DEFAULT_AGENT_NICKNAME),
            Ok(DEFAULT_AGENT_NICKNAME.to_string())
        );
    }

    #[test]
    fn ordinary_names_pass() {
        for name in ["Ada", "Neo_2", "Code-Buddy", "Größe"] {
            assert!(validate_nickname(name).is_ok(), "should pass: {name}");
        }
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(MAX_NICKNAME_LEN + 1);
        assert_eq!(validate_nickname(&long), Err(NicknameError::TooLong));
    }

    #[test]
    fn rejects_invalid_chars() {
        assert_eq!(validate_nickname("bad!name"), Err(NicknameError::InvalidChars));
        assert_eq!(validate_nickname("na/me"), Err(NicknameError::InvalidChars));
    }

    #[test]
    fn rejects_english_badword_substring() {
        assert_eq!(validate_nickname("asshole"), Err(NicknameError::BadWord));
        assert_eq!(validate_nickname("SuperFuck"), Err(NicknameError::BadWord));
    }

    #[test]
    fn rejects_german_badword() {
        assert_eq!(validate_nickname("arsch"), Err(NicknameError::BadWord));
        assert_eq!(validate_nickname("arschi"), Err(NicknameError::BadWord));
    }

    #[test]
    fn rejects_leetspeak_badword() {
        assert_eq!(validate_nickname("4rsch"), Err(NicknameError::BadWord));
        assert_eq!(validate_nickname("Fuck"), Err(NicknameError::BadWord));
    }

    #[test]
    fn short_badword_does_not_false_positive_as_substring() {
        // "ass" (len 3) must only match as a whole token, not inside "class".
        assert!(validate_nickname("Classy").is_ok());
    }

    #[test]
    fn reason_codes_are_stable() {
        assert_eq!(NicknameError::TooLong.reason_code(), "tooLong");
        assert_eq!(NicknameError::InvalidChars.reason_code(), "invalidChars");
        assert_eq!(NicknameError::BadWord.reason_code(), "badWord");
    }
}
