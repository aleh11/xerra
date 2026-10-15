//! Deterministic cleanup used when the LLM is bypassed or fails.
//!
//! Conservative by design: it only removes unambiguous fillers and repeated
//! words and fixes mechanical punctuation/capitalisation, so it can never
//! lose the speaker's meaning. "like" is deliberately left alone.

use ::regex::Regex;
use once_cell::sync::Lazy;

/// Hesitation sounds, with an optional trailing comma.
static FILLER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:u+m+|u+h+|e+r+m*|h+m+|m+h+m+)\b,?").expect("valid regex"));
/// ", you know," used as an aside in the middle of a sentence.
static YOU_KNOW_MID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i),\s*you know\s*,").expect("valid regex"));
/// "you know," opening an utterance.
static YOU_KNOW_START_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^\s*you know\s*,\s*").expect("valid regex"));
/// ", you know" closing an utterance.
static YOU_KNOW_END_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i),\s*you know\s*([.!?]?)\s*$").expect("valid regex"));
static SPACE_BEFORE_PUNCT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+([,.!?;:])").expect("valid regex"));
static LEADING_PUNCT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\s,;:]+").expect("valid regex"));
static DOUBLE_COMMA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r",(?:\s*,)+").expect("valid regex"));
static STANDALONE_I_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bi\b").expect("valid regex"));
static SENTENCE_START_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[.!?]\s+(\p{Ll})").expect("valid regex"));

/// Lower-cased alphanumeric core of a token, for repeat comparison.
fn core(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .flat_map(char::to_lowercase)
        .collect()
}

/// "w-w-what" -> "what" when every leading part is a short prefix of the last.
fn collapse_hyphen_stutter(token: &str) -> String {
    let parts: Vec<&str> = token.split('-').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) {
        return token.to_string();
    }
    let last = parts[parts.len() - 1];
    let last_core = core(last);
    let is_stutter = parts[..parts.len() - 1].iter().all(|p| {
        let c = core(p);
        !c.is_empty() && c.chars().count() <= 3 && last_core.starts_with(&c)
    });
    if is_stutter {
        last.to_string()
    } else {
        token.to_string()
    }
}

/// Drops a word that is immediately repeated ("I I think", "the the").
fn collapse_repeats(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split_whitespace() {
        let token = collapse_hyphen_stutter(raw);
        if let Some(prev) = out.last() {
            let c = core(&token);
            let ends_sentence = prev.ends_with(['.', '!', '?']);
            let has_digit = c.chars().any(|ch| ch.is_ascii_digit());
            if !c.is_empty() && !ends_sentence && !has_digit && core(prev) == c {
                // Keep the later token: it carries the trailing punctuation.
                out.pop();
            }
        }
        out.push(token);
    }
    out.join(" ")
}

fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Mechanical cleanup of a raw transcript. Pure and idempotent.
pub fn regex_cleanup(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.is_empty() {
        return text;
    }

    text = YOU_KNOW_MID_RE.replace_all(&text, ",").into_owned();
    text = YOU_KNOW_START_RE.replace(&text, "").into_owned();
    text = YOU_KNOW_END_RE.replace(&text, "$1").into_owned();
    text = FILLER_RE.replace_all(&text, "").into_owned();
    text = collapse_repeats(&text);
    text = SPACE_BEFORE_PUNCT_RE.replace_all(&text, "$1").into_owned();
    text = DOUBLE_COMMA_RE.replace_all(&text, ",").into_owned();
    text = LEADING_PUNCT_RE.replace(&text, "").into_owned();
    text = text.trim().to_string();
    if text.is_empty() {
        return text;
    }

    text = STANDALONE_I_RE.replace_all(&text, "I").into_owned();
    text = uppercase_first(&text);
    text = SENTENCE_START_RE
        .replace_all(&text, |caps: &::regex::Captures| {
            let whole = &caps[0];
            let letter = &caps[1];
            format!(
                "{}{}",
                &whole[..whole.len() - letter.len()],
                letter.to_uppercase()
            )
        })
        .into_owned();

    // Terminal punctuation: swap a dangling comma/semicolon for a period.
    let trimmed = text.trim_end_matches([',', ';', ':', ' ']).to_string();
    let ends_with_terminal = trimmed.ends_with(['.', '!', '?', '…'])
        || trimmed.ends_with(['"', '\'', ')', ']', '”', '’'])
            && trimmed
                .trim_end_matches(['"', '\'', ')', ']', '”', '’'])
                .ends_with(['.', '!', '?']);
    if ends_with_terminal {
        trimmed
    } else {
        format!("{trimmed}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fillers() {
        assert_eq!(
            regex_cleanup("um so uh I think we should go"),
            "So I think we should go."
        );
        assert_eq!(regex_cleanup("well, erm, maybe"), "Well, maybe.");
        assert_eq!(regex_cleanup("hmm that works"), "That works.");
    }

    #[test]
    fn keeps_like_and_real_you_know() {
        assert_eq!(regex_cleanup("it looks like rain"), "It looks like rain.");
        assert_eq!(regex_cleanup("do you know the way"), "Do you know the way.");
        assert_eq!(regex_cleanup("it was, you know, fine"), "It was, fine.");
        assert_eq!(regex_cleanup("you know, it was fine"), "It was fine.");
        assert_eq!(regex_cleanup("it was fine, you know"), "It was fine.");
    }

    #[test]
    fn collapses_repeats_and_stutters() {
        assert_eq!(
            regex_cleanup("I I think the the plan is good"),
            "I think the plan is good."
        );
        assert_eq!(regex_cleanup("w-w-what happened"), "What happened.");
        assert_eq!(regex_cleanup("Wait. Wait for me"), "Wait. Wait for me.");
        assert_eq!(regex_cleanup("call 555 555"), "Call 555 555.");
    }

    #[test]
    fn fixes_spacing_and_punctuation() {
        assert_eq!(
            regex_cleanup("  hello   world , yes  !"),
            "Hello world, yes!"
        );
        assert_eq!(regex_cleanup("ok,"), "Ok.");
    }

    #[test]
    fn capitalizes() {
        assert_eq!(
            regex_cleanup("i think i can. i'm sure. is it?"),
            "I think I can. I'm sure. Is it?"
        );
    }

    #[test]
    fn terminal_period() {
        assert_eq!(regex_cleanup("hello there"), "Hello there.");
        assert_eq!(regex_cleanup("hello there?"), "Hello there?");
        assert_eq!(regex_cleanup("he said \"go.\""), "He said \"go.\"");
    }

    #[test]
    fn empty_and_filler_only() {
        assert_eq!(regex_cleanup(""), "");
        assert_eq!(regex_cleanup("   "), "");
        assert_eq!(regex_cleanup("um uh"), "");
    }

    #[test]
    fn idempotent() {
        let once = regex_cleanup("um i i think the the thing , works");
        assert_eq!(regex_cleanup(&once), once);
    }
}
