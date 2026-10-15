//! Xerra: capture of the frontmost application / window at recording start.
//! Used to pick a cleanup style (see `cleanup::style`). Best-effort on every
//! platform; returns `None` when nothing can be determined.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type, PartialEq)]
pub struct AppContext {
    /// Human-readable application name, e.g. "Slack", "Code".
    pub app_name: String,
    /// macOS bundle id (e.g. "com.tinyspeck.slackmacgap") or the executable
    /// name on Windows/Linux (e.g. "slack.exe", "slack").
    pub bundle_id: Option<String>,
    /// Title of the focused window, when available.
    pub window_title: Option<String>,
    /// True when secure input (e.g. a password field) was active at capture.
    /// Callers must not send the window title to any remote service then.
    pub secure_input: bool,
}

/// Capture the frontmost app and window. Must be cheap and non-blocking
/// enough to call from the recording-start path.
pub fn capture() -> Option<AppContext> {
    None
}
