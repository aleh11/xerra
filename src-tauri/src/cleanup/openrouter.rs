//! OpenRouter chat-completions client (OpenAI-compatible, SSE streaming).
//!
//! Field shapes verified against the OpenRouter docs:
//! - reasoning: unified `reasoning` object, `effort` in
//!   max|xhigh|high|medium|low|minimal|none; `"none"` disables reasoning;
//!   `exclude: true` hides reasoning from the response.
//! - prompt caching: `cache_control: {"type":"ephemeral"}` on a content part;
//!   hits show in `usage.prompt_tokens_details.cached_tokens`.
//! - streaming: `data:` lines, `data: [DONE]`, `: OPENROUTER PROCESSING`
//!   comments, mid-stream errors as a top-level `error` object inside a 200
//!   stream, and a final usage chunk.
//!
//! Haiku 5.5 rejects non-default sampling params and assistant prefill, so the
//! body deliberately has no temperature/top_p/top_k and no assistant message.

use futures_util::StreamExt;
use log::{debug, info};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, REFERER, USER_AGENT};
use serde_json::{json, Value};
use std::sync::OnceLock;

const REFERER_URL: &str = "https://github.com/aleh11/xerra";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
    pub cost: Option<f64>,
}

/// Request body for a cleanup call.
pub fn build_request_body(
    model: &str,
    reasoning_effort: &str,
    system_prompt: &str,
    user_message: &str,
    max_tokens: u32,
) -> Value {
    let reasoning = if reasoning_effort == "none" {
        json!({ "effort": "none" })
    } else {
        json!({ "effort": reasoning_effort, "exclude": true })
    };
    json!({
        "model": model,
        "stream": true,
        "max_tokens": max_tokens,
        "usage": { "include": true },
        "reasoning": reasoning,
        "messages": [
            {
                "role": "system",
                "content": [{
                    "type": "text",
                    "text": system_prompt,
                    "cache_control": { "type": "ephemeral" }
                }]
            },
            { "role": "user", "content": user_message }
        ]
    })
}

/// `input_chars / 2 + 1024`, capped at 8192. The headroom covers reasoning
/// tokens, which count against `max_tokens` when effort is above "none".
pub fn max_tokens_for(input_chars: usize) -> u32 {
    (input_chars / 2 + 1024).min(8192) as u32
}

/// Incremental SSE parser. Feed raw network chunks (which may split lines or
/// even UTF-8 sequences); read the accumulated text when done.
#[derive(Debug, Default)]
pub struct SseAccumulator {
    buf: Vec<u8>,
    pub text: String,
    pub usage: Option<Usage>,
    pub done: bool,
    /// Last `finish_reason` seen; a reply without "stop" may be truncated.
    pub finish_reason: Option<String>,
}

impl SseAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed bytes. Returns `Err` on a mid-stream error object.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.buf.extend_from_slice(bytes);
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            self.handle_line(&String::from_utf8_lossy(&line))?;
        }
        Ok(())
    }

    /// Process a trailing line that has no final newline.
    pub fn finish(&mut self) -> Result<(), String> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let line = std::mem::take(&mut self.buf);
        self.handle_line(&String::from_utf8_lossy(&line))
    }

    fn handle_line(&mut self, line: &str) -> Result<(), String> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() || line.starts_with(':') {
            return Ok(());
        }
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(());
        };
        let data = data.trim();
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            debug!("Ignoring unparseable SSE data line");
            return Ok(());
        };
        if let Some(err) = chunk.get("error").filter(|e| !e.is_null()) {
            let msg = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            let code = err.get("code").map(|c| c.to_string()).unwrap_or_default();
            return Err(format!("OpenRouter stream error ({code}): {msg}"));
        }
        let choice = chunk.get("choices").and_then(|c| c.get(0));
        if let Some(content) = choice
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("content"))
            .and_then(Value::as_str)
        {
            self.text.push_str(content);
        }
        if let Some(reason) = choice
            .and_then(|c| c.get("finish_reason"))
            .and_then(Value::as_str)
        {
            if reason == "error" {
                return Err("OpenRouter stream ended with finish_reason=error".to_string());
            }
            self.finish_reason = Some(reason.to_string());
        }
        if let Some(u) = chunk.get("usage").filter(|u| u.is_object()) {
            self.usage = Some(parse_usage(u));
        }
        Ok(())
    }
}

fn parse_usage(u: &Value) -> Usage {
    Usage {
        prompt_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        cached_tokens: u
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        completion_tokens: u
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cost: u.get("cost").and_then(Value::as_f64),
    }
}

fn http_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(REFERER, HeaderValue::from_static(REFERER_URL));
    headers.insert("X-Title", HeaderValue::from_static("Xerra"));
    headers.insert(USER_AGENT, HeaderValue::from_static("Xerra/1.0"));
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Stream a chat completion and return the full text. The caller applies the
/// overall timeout.
pub async fn complete(base_url: &str, api_key: &str, body: &Value) -> Result<String, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let resp = http_client()?
        .post(&url)
        .bearer_auth(api_key)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("OpenRouter request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let snippet: String = text.chars().take(300).collect();
        return Err(format!("OpenRouter returned {status}: {snippet}"));
    }

    let mut acc = SseAccumulator::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("OpenRouter stream read failed: {e}"))?;
        acc.feed(&chunk)?;
        if acc.done {
            break;
        }
    }
    acc.finish()?;

    if let Some(u) = &acc.usage {
        info!(
            "Cleanup usage: prompt={} cached={} completion={} cost={}",
            u.prompt_tokens,
            u.cached_tokens,
            u.completion_tokens,
            u.cost
                .map(|c| format!("${c:.6}"))
                .unwrap_or_else(|| "n/a".into())
        );
    }
    // Never paste a partial reply: a dropped stream or a max_tokens cut-off
    // would silently lose the end of the user's dictation.
    match acc.finish_reason.as_deref() {
        Some("stop") | Some("end_turn") => Ok(acc.text),
        other => Err(format!(
            "OpenRouter reply incomplete (finish_reason={})",
            other.unwrap_or("none")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(content: &str) -> String {
        format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n",
            json!(content)
        )
    }

    #[test]
    fn parses_stream_with_keepalive_and_done() {
        let mut a = SseAccumulator::new();
        let stream = format!(
            ": OPENROUTER PROCESSING\n\n{}{}\ndata: {{\"choices\":[{{\"delta\":{{\"reasoning\":\"hmm\"}}}}]}}\n\ndata: {{\"choices\":[{{\"delta\":{{\"content\":\"\"}},\"finish_reason\":\"stop\"}}],\"usage\":{{\"prompt_tokens\":600,\"completion_tokens\":5,\"cost\":0.0007,\"prompt_tokens_details\":{{\"cached_tokens\":512}}}}}}\n\ndata: [DONE]\n\n",
            chunk("Hello"),
            chunk(" world.")
        );
        a.feed(stream.as_bytes()).unwrap();
        assert!(a.done);
        assert_eq!(a.text, "Hello world.");
        let u = a.usage.unwrap();
        assert_eq!(
            (u.prompt_tokens, u.cached_tokens, u.completion_tokens),
            (600, 512, 5)
        );
        assert_eq!(u.cost, Some(0.0007));
    }

    #[test]
    fn handles_chunks_split_anywhere() {
        let stream = format!(
            ": ping\r\n\r\n{}{}data: [DONE]\r\n\r\n",
            chunk("héllo"),
            chunk(" ✓")
        );
        let bytes = stream.as_bytes();
        for step in [1usize, 2, 3, 7] {
            let mut a = SseAccumulator::new();
            for part in bytes.chunks(step) {
                a.feed(part).unwrap();
            }
            a.finish().unwrap();
            assert_eq!(a.text, "héllo ✓", "step {step}");
            assert!(a.done);
        }
    }

    #[test]
    fn final_line_without_newline() {
        let mut a = SseAccumulator::new();
        a.feed(chunk("a").trim_end().as_bytes()).unwrap();
        assert_eq!(a.text, "");
        a.finish().unwrap();
        assert_eq!(a.text, "a");
    }

    #[test]
    fn mid_stream_error_is_reported() {
        let mut a = SseAccumulator::new();
        a.feed(chunk("partial").as_bytes()).unwrap();
        let err = a
            .feed(b"data: {\"error\":{\"code\":502,\"message\":\"Provider disconnected\"},\"choices\":[{\"delta\":{\"content\":\"\"},\"finish_reason\":\"error\"}]}\n\n")
            .unwrap_err();
        assert!(err.contains("Provider disconnected"));
    }

    #[test]
    fn request_body_shape() {
        let b = build_request_body("m", "low", "SYS", "USER", 300);
        assert_eq!(b["stream"], true);
        assert_eq!(b["reasoning"], json!({"effort": "low", "exclude": true}));
        for k in [
            "temperature",
            "top_p",
            "top_k",
            "frequency_penalty",
            "presence_penalty",
            "seed",
        ] {
            assert!(b.get(k).is_none(), "{k} must not be sent");
        }
        let sys = &b["messages"][0];
        assert_eq!(sys["role"], "system");
        assert_eq!(
            sys["content"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
        assert_eq!(sys["content"][0]["text"], "SYS");
        let msgs = b["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert!(msgs.iter().all(|m| m["role"] != "assistant"));
        assert_eq!(msgs[1]["content"], "USER");
    }

    #[test]
    fn reasoning_none_disables() {
        let b = build_request_body("m", "none", "S", "U", 10);
        assert_eq!(b["reasoning"], json!({"effort": "none"}));
    }

    #[test]
    fn max_tokens_scales_and_caps() {
        assert_eq!(max_tokens_for(0), 1024);
        assert_eq!(max_tokens_for(1000), 1524);
        assert_eq!(max_tokens_for(100_000), 8192);
    }

    #[test]
    fn records_finish_reason() {
        let mut acc = SseAccumulator::new();
        acc.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"finish_reason\":\"length\"}]}\n\n")
            .unwrap();
        assert_eq!(acc.finish_reason.as_deref(), Some("length"));
        assert_eq!(acc.text, "Hi");
    }
}
