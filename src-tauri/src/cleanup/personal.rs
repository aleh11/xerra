//! Personal vocabulary: dictionary terms, "heard as -> write as"
//! replacements and whole-utterance snippets.

use super::{Replacement, Snippet};
use ::regex::Regex;
use std::collections::HashMap;

const FILLERS: [&str; 7] = ["um", "uh", "er", "erm", "hmm", "mm", "ah"];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Collapse internal whitespace and lowercase, for keying `heard` phrases.
fn phrase_key(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Regex source for one phrase: words joined by flexible whitespace, with
/// word boundaries only where the edge character is a word character.
fn phrase_pattern(heard: &str) -> String {
    let words: Vec<String> = heard.split_whitespace().map(::regex::escape).collect();
    let body = words.join(r"\s+");
    let first = heard.trim().chars().next().is_some_and(is_word_char);
    let last = heard.trim().chars().last().is_some_and(is_word_char);
    format!(
        "{}{}{}",
        if first { r"\b" } else { "" },
        body,
        if last { r"\b" } else { "" }
    )
}

/// Deterministically rewrite known mis-hearings. Case-insensitive, whole
/// word/phrase, longest `heard` first (single pass, so output is never
/// re-matched). Entries with an empty `heard` are skipped.
pub fn apply_replacements(text: &str, replacements: &[Replacement]) -> String {
    let mut entries: Vec<&Replacement> = replacements
        .iter()
        .filter(|r| !r.heard.trim().is_empty())
        .collect();
    if entries.is_empty() {
        return text.to_string();
    }
    entries.sort_by_key(|r| std::cmp::Reverse(phrase_key(&r.heard).chars().count()));
    let mut map: HashMap<String, &str> = HashMap::new();
    for r in &entries {
        map.entry(phrase_key(&r.heard)).or_insert(r.write.as_str());
    }
    let pattern = entries
        .iter()
        .map(|r| format!("(?:{})", phrase_pattern(&r.heard)))
        .collect::<Vec<_>>()
        .join("|");
    let Ok(re) = Regex::new(&format!("(?i){pattern}")) else {
        return text.to_string();
    };
    re.replace_all(text, |caps: &::regex::Captures| {
        let m = &caps[0];
        map.get(&phrase_key(m)).copied().unwrap_or(m).to_string()
    })
    .into_owned()
}

/// Lowercase, strip punctuation, collapse whitespace and drop leading and
/// trailing fillers.
pub fn normalize_utterance(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| *c != '\'' && *c != '\u{2019}')
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .to_lowercase();
    let mut tokens: Vec<&str> = cleaned.split_whitespace().collect();
    while tokens.first().is_some_and(|t| FILLERS.contains(t)) {
        tokens.remove(0);
    }
    while tokens.last().is_some_and(|t| FILLERS.contains(t)) {
        tokens.pop();
    }
    tokens.join(" ")
}

/// The expansion of the first snippet whose trigger equals the whole utterance.
pub fn match_snippet<'a>(snippets: &'a [Snippet], utterance: &str) -> Option<&'a str> {
    let norm = normalize_utterance(utterance);
    if norm.is_empty() {
        return None;
    }
    snippets
        .iter()
        .find(|s| normalize_utterance(&s.trigger) == norm)
        .map(|s| s.expansion.as_str())
}

/// Dictionary, upstream custom words and replacement targets, trimmed and
/// deduplicated case-insensitively (first spelling wins).
pub fn merge_terms(
    dictionary: &[String],
    custom_words: &[String],
    replacements: &[Replacement],
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let targets = replacements
        .iter()
        .filter(|r| !r.heard.trim().is_empty())
        .map(|r| &r.write);
    for t in dictionary.iter().chain(custom_words).chain(targets) {
        let t = t.trim();
        if !t.is_empty() && seen.insert(t.to_lowercase()) {
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(h: &str, w: &str) -> Replacement {
        Replacement {
            heard: h.into(),
            write: w.into(),
        }
    }

    #[test]
    fn replacement_respects_boundaries() {
        let r = [rep("cron", "Cron")];
        assert_eq!(
            apply_replacements("edit the crontab", &r),
            "edit the crontab"
        );
        assert_eq!(apply_replacements("run a cron, now", &r), "run a Cron, now");
        assert_eq!(apply_replacements("CRON job", &r), "Cron job");
    }

    #[test]
    fn replacement_longest_first_and_skips_empty() {
        let r = [
            rep("", "X"),
            rep("you be", "UB"),
            rep("you be cron", "Ubikron"),
        ];
        assert_eq!(
            apply_replacements("ask you be cron today", &r),
            "ask Ubikron today"
        );
        assert_eq!(apply_replacements("you be here", &r), "UB here");
        assert_eq!(apply_replacements("hello", &[rep("  ", "X")]), "hello");
    }

    #[test]
    fn snippet_normalisation_and_miss() {
        let s = [Snippet {
            trigger: "My email address".into(),
            expansion: "a@b.com\nline2".into(),
        }];
        assert_eq!(
            match_snippet(&s, "Um, my email address."),
            Some("a@b.com\nline2")
        );
        assert_eq!(
            match_snippet(&s, "my email address uh"),
            Some("a@b.com\nline2")
        );
        assert_eq!(match_snippet(&s, "what is my email address"), None);
        assert_eq!(match_snippet(&s, "um"), None);
    }

    #[test]
    fn terms_dedup() {
        let t = merge_terms(
            &["Mimic".into(), " JSONata ".into()],
            &["mimic".into(), "Tauri".into(), "".into()],
            &[
                rep("you be cron", "Ubikron"),
                rep("x", "jsonata"),
                rep("", "Skip"),
            ],
        );
        assert_eq!(t, vec!["Mimic", "JSONata", "Tauri", "Ubikron"]);
    }
}
