use std::collections::{BTreeSet, HashMap};

use super::types::MemoryScope;

#[derive(Debug, Clone)]
pub struct ParsedWikilink {
    pub target: String,
    pub alias: Option<String>,
    pub scope: Option<MemoryScope>,
}

pub fn parse_wikilink_inner(inner: &str) -> Option<ParsedWikilink> {
    let (target_part, alias) = match inner.find('|') {
        Some(i) => (&inner[..i], Some(inner[i + 1..].trim().to_owned())),
        None => (inner, None),
    };
    let target_part = target_part.trim();
    if target_part.is_empty() {
        return None;
    }
    let (scope, target) = if let Some(colon) = target_part.find(':') {
        if colon > 0 {
            let prefix = &target_part[..colon];
            match prefix {
                "global" => (
                    Some(MemoryScope::Global),
                    target_part[colon + 1..].trim().to_owned(),
                ),
                "workspace" => (
                    Some(MemoryScope::Workspace),
                    target_part[colon + 1..].trim().to_owned(),
                ),
                _ => (None, target_part.to_owned()),
            }
        } else {
            (None, target_part.to_owned())
        }
    } else {
        (None, target_part.to_owned())
    };
    if target.is_empty() {
        return None;
    }
    Some(ParsedWikilink {
        target,
        alias,
        scope,
    })
}

pub struct ParseResult {
    pub links: Vec<ParsedWikilink>,
    pub tags: Vec<String>,
}

pub fn parse_links_and_tags(body: &str) -> ParseResult {
    let mut links: Vec<ParsedWikilink> = Vec::new();
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let start = i + 2;
            let mut j = start;
            let mut found = false;
            while j + 1 < bytes.len() {
                if bytes[j] == b']' && bytes[j + 1] == b']' {
                    let inner = &body[start..j];
                    if let Some(link) = parse_wikilink_inner(inner) {
                        links.push(link);
                    }
                    i = j + 2;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'#' {
            let prev = if i == 0 { b'\n' } else { bytes[i - 1] };
            if prev == b'\n' || prev == b' ' || prev == b'\t' {
                let mut j = i + 1;
                while j < bytes.len() {
                    let c = bytes[j];
                    let ok = c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'/';
                    if !ok {
                        break;
                    }
                    j += 1;
                }
                if j > i + 1 && bytes[i + 1] != b' ' {
                    let tag = &body[i + 1..j];
                    if !tag.is_empty() && !tag.chars().all(|c| c.is_ascii_digit()) {
                        tags.insert(tag.to_owned());
                    }
                }
                i = j.max(i + 1);
                continue;
            }
        }
        i += 1;
    }

    ParseResult {
        links,
        tags: tags.into_iter().collect(),
    }
}

pub struct NoteLookup {
    pub by_path: HashMap<String, String>,
    pub by_basename: HashMap<String, Vec<String>>,
}

pub fn build_note_lookup(api_paths: &[String]) -> NoteLookup {
    let mut by_path: HashMap<String, String> = HashMap::new();
    let mut by_basename: HashMap<String, Vec<String>> = HashMap::new();
    for api in api_paths {
        by_path.insert(api.to_lowercase(), api.clone());
        let no_ext = strip_md_ext(api).to_ascii_lowercase();
        by_path.insert(no_ext, api.clone());
        if let Some(base) = api.split('/').last() {
            let stem = strip_md_ext(base).to_ascii_lowercase();
            by_basename.entry(stem).or_default().push(api.clone());
        }
    }
    NoteLookup {
        by_path,
        by_basename,
    }
}

pub fn resolve_link_target(
    source_scope: &MemoryScope,
    link: &ParsedWikilink,
    scope_paths: &HashMap<MemoryScope, Vec<String>>,
) -> Option<(MemoryScope, String)> {
    let target_scope = link.scope.as_ref().unwrap_or(source_scope);
    let empty = Vec::new();
    let paths = scope_paths.get(target_scope).unwrap_or(&empty);
    let lookup = build_note_lookup(paths);
    let raw = link.target.replace('\\', "/");
    if raw.is_empty() {
        return None;
    }
    let candidate = if raw.ends_with(".md") {
        raw.clone()
    } else {
        format!("{raw}.md")
    };
    if let Some(p) = lookup.by_path.get(&candidate.to_ascii_lowercase()) {
        return Some((target_scope.clone(), p.clone()));
    }
    let no_ext = strip_md_ext(&raw).to_ascii_lowercase();
    if let Some(p) = lookup.by_path.get(&no_ext) {
        return Some((target_scope.clone(), p.clone()));
    }
    if let Some(base) = raw.split('/').last() {
        let stem = strip_md_ext(base).to_ascii_lowercase();
        if let Some(matches) = lookup.by_basename.get(&stem) {
            if matches.len() == 1 {
                return Some((target_scope.clone(), matches[0].clone()));
            }
        }
    }
    None
}

pub fn rewrite_wikilinks(
    content: &str,
    old_basename: &str,
    new_basename: &str,
    old_path: &str,
    new_path: &str,
) -> (String, u32) {
    let old_pwx = strip_md_ext(old_path);
    let new_pwx = strip_md_ext(new_path);
    let mut out = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let mut i = 0usize;
    let mut count = 0u32;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = find_close_link(&content[i + 2..]) {
                let inner = &content[i + 2..i + 2 + end];
                let (target, alias) = match inner.find('|') {
                    Some(j) => (&inner[..j], Some(&inner[j + 1..])),
                    None => (inner, None),
                };
                let target_t = target.trim();
                let new_target = if target_t.eq_ignore_ascii_case(old_basename)
                    || target_t.eq_ignore_ascii_case(&old_pwx)
                {
                    if target_t.contains('/') {
                        Some(new_pwx.clone())
                    } else {
                        Some(new_basename.to_owned())
                    }
                } else {
                    None
                };
                if let Some(t) = new_target {
                    out.push_str("[[");
                    out.push_str(&t);
                    if let Some(a) = alias {
                        out.push('|');
                        out.push_str(a);
                    }
                    out.push_str("]]");
                    count += 1;
                } else {
                    out.push_str(&content[i..i + 2 + end + 2]);
                }
                i += 2 + end + 2;
                continue;
            }
        }
        let ch = content[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, count)
}

pub fn strip_md_ext(p: &str) -> String {
    if let Some(idx) = p.rfind('.') {
        if p[idx + 1..].eq_ignore_ascii_case("md") {
            return p[..idx].to_owned();
        }
    }
    p.to_owned()
}

fn find_close_link(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b']' && bytes[i + 1] == b']' {
            return Some(i);
        }
        if bytes[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}
