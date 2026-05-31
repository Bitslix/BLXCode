//! Small, dependency-free fuzzy subsequence scorer used by the file finder.
//!
//! [`fuzzy_score`] returns `None` when `query` is not a (case-insensitive)
//! subsequence of `text`, otherwise a score where higher is a better match.
//! Scoring rewards contiguous runs, matches inside the basename, and matches at
//! word boundaries; it penalizes gaps, a late first match, and long paths.

/// Score `text` against `query`. `None` ⇒ not a subsequence (filtered out).
#[must_use]
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let base_start = text.rfind(['/', '\\']).map(|i| i + 1).unwrap_or(0);

    let mut score = 0i32;
    let mut ti = 0usize;
    let mut last_match: Option<usize> = None;

    for &qc in &q {
        let mut found = None;
        while ti < t.len() {
            if t[ti] == qc {
                found = Some(ti);
                break;
            }
            ti += 1;
        }
        let pos = found?;
        match last_match {
            Some(last) if pos == last + 1 => score += 8, // contiguous run
            Some(last) => score -= ((pos - last - 1).min(10)) as i32, // gap
            None => score -= (pos.min(20)) as i32,       // earlier first match is better
        }
        if pos >= base_start {
            score += 6; // basename hit
        }
        let prev_is_boundary = pos == 0
            || matches!(
                t.get(pos - 1),
                Some('/') | Some('\\') | Some('_') | Some('-') | Some('.') | Some(' ')
            );
        if prev_is_boundary {
            score += 4;
        }
        last_match = Some(pos);
        ti = pos + 1;
    }
    // Mildly prefer shorter paths among otherwise-equal matches.
    score -= (t.len() / 16) as i32;
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn non_subsequence_is_none() {
        assert!(fuzzy_score("zzz", "src/main.rs").is_none());
        assert!(fuzzy_score("xyz", "abc").is_none());
    }

    #[test]
    fn subsequence_matches() {
        assert!(fuzzy_score("mainrs", "src/main.rs").is_some());
        assert!(fuzzy_score("srcmain", "src/main.rs").is_some());
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn basename_match_beats_dir_match() {
        // "main" in the basename should rank above "main" only in a dir name.
        let base = fuzzy_score("main", "x/main.rs").unwrap();
        let dir = fuzzy_score("main", "main/x.rs").unwrap();
        assert!(base > dir, "base={base} dir={dir}");
    }

    #[test]
    fn contiguous_beats_scattered() {
        let contiguous = fuzzy_score("abc", "abc.txt").unwrap();
        let scattered = fuzzy_score("abc", "a_b_c.txt").unwrap();
        assert!(contiguous > scattered, "cont={contiguous} scat={scattered}");
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_score("MAIN", "src/main.rs").is_some());
        assert!(fuzzy_score("main", "src/MAIN.RS").is_some());
    }
}
