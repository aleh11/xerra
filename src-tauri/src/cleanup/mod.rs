//! Xerra dictation cleanup: LLM pass (OpenRouter) with a regex fallback,
//! applied between transcription and paste on the plain transcribe hotkey.
//! Invariant: the user's words are never lost; any failure pastes the
//! regex-cleaned text.

mod keychain;
mod openrouter;
mod regex_fallback;
mod style;

pub use regex_fallback::regex_cleanup;
pub use style::resolve_style;

use crate::app_context::AppContext;
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;

const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");
const REASONING_EFFORTS: [&str; 5] = ["none", "minimal", "low", "medium", "high"];

#[derive(Serialize, Deserialize, Type, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CleanupStyle {
    #[default]
    Neutral,
    Casual,
    Formal,
    Code,
}

#[derive(Serialize, Deserialize, Type, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchField {
    AppName,
    BundleId,
    WindowTitle,
}

#[derive(Serialize, Deserialize, Type, Clone, Debug, PartialEq)]
pub struct AppStyleRule {
    /// Case-insensitive substring.
    pub pattern: String,
    pub field: MatchField,
    pub style: CleanupStyle,
}

#[derive(Serialize, Deserialize, Type, Clone, Debug)]
#[serde(default)]
pub struct CleanupSettings {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    /// One of "none" | "minimal" | "low" | "medium" | "high".
    pub reasoning_effort: String,
    pub timeout_ms: u32,
    pub short_utterance_words: u32,
    pub app_style_rules: Vec<AppStyleRule>,
    pub default_style: CleanupStyle,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            model: "anthropic/claude-haiku-5.5".to_string(),
            reasoning_effort: "low".to_string(),
            timeout_ms: 2000,
            short_utterance_words: 8,
            app_style_rules: style::default_rules(),
            default_style: CleanupStyle::Neutral,
        }
    }
}

#[derive(Serialize, Deserialize, Type, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupSource {
    Llm,
    Regex,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct CleanupOutcome {
    pub text: String,
    pub source: CleanupSource,
    pub latency_ms: u64,
    pub error: Option<String>,
}

fn outcome(
    text: String,
    source: CleanupSource,
    start: Instant,
    error: Option<String>,
) -> CleanupOutcome {
    CleanupOutcome {
        text,
        source,
        latency_ms: start.elapsed().as_millis() as u64,
        error,
    }
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Build the variable part of the prompt. All variable content lives here so
/// the cached system prefix stays stable.
fn build_user_message(
    style: CleanupStyle,
    ctx: Option<&AppContext>,
    terms: &[String],
    transcript: &str,
) -> String {
    let mut s = String::from("<context>\n");
    s.push_str(&format!("STYLE: {}\n", style.as_str()));
    if let Some(ctx) = ctx {
        if !ctx.app_name.is_empty() {
            s.push_str(&format!("APP: {}\n", ctx.app_name));
        }
        if !ctx.secure_input {
            if let Some(t) = ctx.window_title.as_deref().filter(|t| !t.trim().is_empty()) {
                s.push_str(&format!("WINDOW: {}\n", t.trim()));
            }
        }
    }
    if !terms.is_empty() {
        s.push_str(&format!("VOCABULARY: {}\n", terms.join(", ")));
    }
    s.push_str("</context>\n<transcript>\n");
    s.push_str(transcript);
    s.push_str("\n</transcript>");
    s
}

/// Remove an echoed `<transcript>` wrapper or wrapping quotes.
fn sanitize_llm_output(out: &str, input: &str) -> String {
    let mut t = out.trim();
    if let Some(r) = t.strip_prefix("<transcript>") {
        t = r.trim_start();
    }
    if let Some(r) = t.strip_suffix("</transcript>") {
        t = r.trim_end();
    }
    let wrapped = |open: char, close: char| {
        t.chars().count() >= 2
            && t.starts_with(open)
            && t.ends_with(close)
            && !input.trim_start().starts_with(open)
    };
    if wrapped('"', '"') || wrapped('\u{201C}', '\u{201D}') {
        t = t
            .strip_prefix(['"', '\u{201C}'])
            .and_then(|r| r.strip_suffix(['"', '\u{201D}']))
            .unwrap_or(t)
            .trim();
    }
    t.to_string()
}

/// Output far longer than the input means the model answered instead of cleaning.
fn is_suspicious(output: &str, input: &str) -> bool {
    output.chars().count() > input.chars().count() * 2 + 200
}

/// Run the cleanup pipeline. Never fails: falls back to regex output.
pub async fn run(
    settings: &CleanupSettings,
    raw: &str,
    ctx: Option<&AppContext>,
    terms: &[String],
) -> CleanupOutcome {
    run_inner(settings, raw, ctx, terms, None, false).await
}

async fn run_inner(
    settings: &CleanupSettings,
    raw: &str,
    ctx: Option<&AppContext>,
    terms: &[String],
    style_override: Option<CleanupStyle>,
    force_llm: bool,
) -> CleanupOutcome {
    let start = Instant::now();
    let style = style_override.unwrap_or_else(|| resolve_style(settings, ctx));

    if raw.trim().is_empty() {
        return outcome(raw.to_string(), CleanupSource::Unchanged, start, None);
    }
    let regex_text = regex_cleanup(raw);
    // Filler-only input: pasting nothing is correct.
    if regex_text.is_empty() {
        return outcome(regex_text, CleanupSource::Regex, start, None);
    }

    if !force_llm {
        if !settings.enabled {
            return outcome(raw.to_string(), CleanupSource::Unchanged, start, None);
        }
        let short = word_count(raw) < settings.short_utterance_words as usize;
        if short && style != CleanupStyle::Code {
            return outcome(regex_text, CleanupSource::Regex, start, None);
        }
    }

    let Some(api_key) = keychain::get_api_key_async().await else {
        let err = force_llm.then(|| "No OpenRouter API key configured".to_string());
        return outcome(regex_text, CleanupSource::Regex, start, err);
    };

    let user_message = build_user_message(style, ctx, terms, raw);
    let body = openrouter::build_request_body(
        &settings.model,
        &settings.reasoning_effort,
        SYSTEM_PROMPT,
        &user_message,
        openrouter::max_tokens_for(raw.chars().count()),
    );
    let call = openrouter::complete(&settings.base_url, &api_key, &body);
    let result =
        tokio::time::timeout(Duration::from_millis(settings.timeout_ms.into()), call).await;

    let failure = match result {
        Err(_) => format!("LLM cleanup timed out after {} ms", settings.timeout_ms),
        Ok(Err(e)) => e,
        Ok(Ok(text)) => {
            let cleaned = sanitize_llm_output(&text, raw);
            if cleaned.is_empty() {
                "LLM returned empty output".to_string()
            } else if is_suspicious(&cleaned, raw) {
                "LLM output rejected (much longer than input)".to_string()
            } else {
                return outcome(cleaned, CleanupSource::Llm, start, None);
            }
        }
    };
    warn!("Cleanup falling back to regex: {failure}");
    outcome(regex_text, CleanupSource::Regex, start, Some(failure))
}

/// Cleanup entry point for the transcribe action: reads settings and
/// vocabulary from app settings, logs the outcome.
pub async fn run_for_app(app: &AppHandle, raw: &str, ctx: Option<&AppContext>) -> CleanupOutcome {
    let settings = crate::settings::get_settings(app);
    let out = run(&settings.cleanup, raw, ctx, &settings.custom_words).await;
    info!(
        "Cleanup finished: source={:?} latency={}ms error={}",
        out.source,
        out.latency_ms,
        out.error.as_deref().unwrap_or("none")
    );
    out
}

// ---------------------------------------------------------------------------
// App context captured at recording start, consumed at stop.
// ---------------------------------------------------------------------------

type ContextSlot = Arc<(Mutex<Option<Option<AppContext>>>, Condvar)>;

static CONTEXTS: Lazy<Mutex<HashMap<String, ContextSlot>>> = Lazy::new(Default::default);

/// Capture the frontmost app on a background thread so the start path is not
/// blocked. Replaces any stale capture for the binding.
pub fn begin_context_capture(binding_id: &str) {
    let slot: ContextSlot = Arc::new((Mutex::new(None), Condvar::new()));
    CONTEXTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(binding_id.to_string(), Arc::clone(&slot));
    std::thread::spawn(move || {
        let ctx = crate::app_context::capture();
        let (lock, cv) = &*slot;
        *lock.lock().unwrap_or_else(|p| p.into_inner()) = Some(ctx);
        cv.notify_all();
    });
}

/// Take the context captured for `binding_id`, waiting briefly if the capture
/// thread is still running.
pub async fn take_context(binding_id: &str) -> Option<AppContext> {
    let slot = CONTEXTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(binding_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cv) = &*slot;
        let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
        let (mut guard, _) = cv
            .wait_timeout_while(guard, Duration::from_millis(300), |v| v.is_none())
            .unwrap_or_else(|p| p.into_inner());
        guard.take().flatten()
    })
    .await
    .unwrap_or_else(|e| {
        debug!("Context capture task failed: {e}");
        None
    })
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn set_cleanup_settings(app: AppHandle, settings: CleanupSettings) -> Result<(), String> {
    if !REASONING_EFFORTS.contains(&settings.reasoning_effort.as_str()) {
        return Err(format!(
            "Invalid reasoning_effort '{}'; expected one of {:?}",
            settings.reasoning_effort, REASONING_EFFORTS
        ));
    }
    if settings.timeout_ms == 0 {
        return Err("timeout_ms must be greater than 0".to_string());
    }
    let mut app_settings = crate::settings::get_settings(&app);
    app_settings.cleanup = settings;
    crate::settings::write_settings(&app, app_settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_cleanup_api_key(key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || keychain::set_api_key(&key))
        .await
        .map_err(|e| format!("Keychain task failed: {e}"))?
}

#[tauri::command]
#[specta::specta]
pub async fn has_cleanup_api_key() -> bool {
    keychain::get_api_key_async().await.is_some()
}

#[derive(Serialize, Deserialize, Type, Clone, Debug)]
pub struct CleanupTestResult {
    pub output: String,
    pub source: CleanupSource,
    pub style: CleanupStyle,
    pub latency_ms: u32,
    pub error: Option<String>,
}

/// Run cleanup on sample text, forcing the LLM path (no short-utterance bypass).
#[tauri::command]
#[specta::specta]
pub async fn test_cleanup(
    app: AppHandle,
    raw: String,
    style: Option<CleanupStyle>,
    app_name: Option<String>,
    window_title: Option<String>,
) -> CleanupTestResult {
    let app_settings = crate::settings::get_settings(&app);
    let ctx = (app_name.is_some() || window_title.is_some()).then(|| AppContext {
        app_name: app_name.unwrap_or_default(),
        bundle_id: None,
        window_title,
        secure_input: false,
    });
    let resolved = style.unwrap_or_else(|| resolve_style(&app_settings.cleanup, ctx.as_ref()));
    let out = run_inner(
        &app_settings.cleanup,
        &raw,
        ctx.as_ref(),
        &app_settings.custom_words,
        Some(resolved),
        true,
    )
    .await;
    CleanupTestResult {
        output: out.text,
        source: out.source,
        style: resolved,
        latency_ms: out.latency_ms.min(u32::MAX as u64) as u32,
        error: out.error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(secure: bool, title: Option<&str>) -> AppContext {
        AppContext {
            app_name: "Visual Studio Code".into(),
            bundle_id: None,
            window_title: title.map(Into::into),
            secure_input: secure,
        }
    }

    #[test]
    fn user_message_layout() {
        let m = build_user_message(
            CleanupStyle::Code,
            Some(&ctx(false, Some("main.rs"))),
            &["Tauri".into(), "Xerra".into()],
            "hello",
        );
        assert_eq!(
            m,
            "<context>\nSTYLE: Code\nAPP: Visual Studio Code\nWINDOW: main.rs\nVOCABULARY: Tauri, Xerra\n</context>\n<transcript>\nhello\n</transcript>"
        );
    }

    #[test]
    fn window_omitted_for_secure_input_or_empty() {
        assert!(!build_user_message(
            CleanupStyle::Neutral,
            Some(&ctx(true, Some("pw"))),
            &[],
            "x"
        )
        .contains("WINDOW"));
        assert!(!build_user_message(
            CleanupStyle::Neutral,
            Some(&ctx(false, Some("  "))),
            &[],
            "x"
        )
        .contains("WINDOW"));
        assert!(!build_user_message(CleanupStyle::Neutral, None, &[], "x").contains("VOCABULARY"));
    }

    #[test]
    fn sanitizes_echoes() {
        assert_eq!(
            sanitize_llm_output("<transcript>\nHi there.\n</transcript>", "hi there"),
            "Hi there."
        );
        assert_eq!(
            sanitize_llm_output("\"Hi there.\"", "hi there"),
            "Hi there."
        );
        assert_eq!(
            sanitize_llm_output("\"Hi,\" he said.", "hi he said"),
            "\"Hi,\" he said."
        );
        assert_eq!(
            sanitize_llm_output("\"Quoted\"", "\"quoted\""),
            "\"Quoted\""
        );
    }

    #[test]
    fn suspicious_length() {
        assert!(!is_suspicious("short", "short"));
        assert!(is_suspicious(&"x".repeat(300), "hi"));
    }

    #[test]
    fn defaults_are_sane() {
        let s = CleanupSettings::default();
        assert!(s.enabled);
        assert!(REASONING_EFFORTS.contains(&s.reasoning_effort.as_str()));
        assert_eq!(s.timeout_ms, 2000);
    }

    #[tokio::test]
    async fn bypass_paths_use_regex_without_network() {
        let mut s = CleanupSettings::default();
        let o = run(&s, "um hello there", None, &[]).await;
        assert_eq!(o.source, CleanupSource::Regex);
        assert_eq!(o.text, "Hello there.");
        let o = run(&s, "um uh", None, &[]).await;
        assert_eq!(o.text, "");
        s.enabled = false;
        let o = run(&s, "um hello there", None, &[]).await;
        assert_eq!(o.source, CleanupSource::Unchanged);
        assert_eq!(o.text, "um hello there");
    }

    /// Live smoke test; needs OPENROUTER_API_KEY. Run with `--ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn live_openrouter_smoke() {
        let Ok(key) = std::env::var("OPENROUTER_API_KEY") else {
            eprintln!("OPENROUTER_API_KEY not set; skipping");
            return;
        };
        let s = CleanupSettings::default();
        let raw = "um so i i wanted to to ask you know whether the the meeting on tuesday no wait wednesday is still on";
        let body = openrouter::build_request_body(
            &s.model,
            &s.reasoning_effort,
            SYSTEM_PROMPT,
            &build_user_message(CleanupStyle::Neutral, None, &[], raw),
            openrouter::max_tokens_for(raw.len()),
        );
        let t = Instant::now();
        let out = openrouter::complete(&s.base_url, &key, &body).await;
        eprintln!("latency={:?} result={:?}", t.elapsed(), out);
        assert!(out.is_ok());
    }
}
