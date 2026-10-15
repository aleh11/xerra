//! OpenRouter API key storage in the OS keychain (macOS Keychain, Windows
//! Credential Manager, Linux Secret Service). The key is cached in memory
//! after the first read. The key is never logged.

use log::{debug, warn};
use once_cell::sync::Lazy;
use std::sync::Mutex;

const ACCOUNT: &str = "openrouter_api_key";
const ENV_VAR: &str = "OPENROUTER_API_KEY";

/// `None` = not loaded yet; `Some(None)` = loaded, no key.
static CACHE: Lazy<Mutex<Option<Option<String>>>> = Lazy::new(|| Mutex::new(None));

/// Keychain service name: the app identifier from tauri.conf.json.
fn service_name() -> String {
    static CONF: &str = include_str!("../../tauri.conf.json");
    serde_json::from_str::<serde_json::Value>(CONF)
        .ok()
        .and_then(|v| v.get("identifier")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| "dev.alehh.xerra".to_string())
}

fn entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(&service_name(), ACCOUNT)
}

fn read_keychain() -> Option<String> {
    match entry().and_then(|e| e.get_password()) {
        Ok(k) if !k.trim().is_empty() => Some(k.trim().to_string()),
        Ok(_) | Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            debug!("Keychain read failed: {e}");
            None
        }
    }
}

/// The API key, from cache, the keychain, or (dev fallback) the
/// `OPENROUTER_API_KEY` env var. May block on the keychain the first time;
/// call from a blocking context.
pub fn get_api_key() -> Option<String> {
    let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(cached) = cache.as_ref() {
        return cached.clone();
    }
    let key = read_keychain().or_else(|| {
        std::env::var(ENV_VAR)
            .ok()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
    });
    *cache = Some(key.clone());
    key
}

/// Async wrapper that keeps keychain I/O off the async runtime threads.
pub async fn get_api_key_async() -> Option<String> {
    if let Some(cached) = CACHE.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        return cached.clone();
    }
    tauri::async_runtime::spawn_blocking(get_api_key)
        .await
        .unwrap_or_else(|e| {
            warn!("Keychain task failed: {e}");
            None
        })
}

/// Store the key; an empty (or whitespace) string deletes it.
pub fn set_api_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    let e = entry().map_err(|e| format!("Keychain unavailable: {e}"))?;
    if key.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => return Err(format!("Failed to delete API key: {err}")),
        }
    } else {
        e.set_password(key)
            .map_err(|err| format!("Failed to store API key: {err}"))?;
    }
    // Reset so the next read re-resolves (keychain, then env fallback).
    *CACHE.lock().unwrap_or_else(|p| p.into_inner()) = None;
    Ok(())
}
