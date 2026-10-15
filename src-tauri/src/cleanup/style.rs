//! Style selection: maps the frontmost app / window to a `CleanupStyle`
//! using ordered, case-insensitive substring rules (first match wins).

use super::{AppStyleRule, CleanupSettings, CleanupStyle, MatchField};
use crate::app_context::AppContext;

impl CleanupStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            CleanupStyle::Neutral => "Neutral",
            CleanupStyle::Casual => "Casual",
            CleanupStyle::Formal => "Formal",
            CleanupStyle::Code => "Code",
        }
    }
}

fn rule(pattern: &str, field: MatchField, style: CleanupStyle) -> AppStyleRule {
    AppStyleRule {
        pattern: pattern.to_string(),
        field,
        style,
        exact: false,
    }
}

fn exact_rule(pattern: &str, field: MatchField, style: CleanupStyle) -> AppStyleRule {
    AppStyleRule {
        exact: true,
        ..rule(pattern, field, style)
    }
}

/// Default rules. Order matters: code editors/terminals first (so a file named
/// `Gmail.ts` open in an editor stays Code), then chat, then mail apps, and
/// finally window-title rules for web mail in a browser (browsers have no app
/// rule, so nothing generic can shadow them).
pub fn default_rules() -> Vec<AppStyleRule> {
    use CleanupStyle::{Casual, Code, Formal};
    use MatchField::{AppName, BundleId, WindowTitle};
    vec![
        // Code
        rule("Visual Studio Code", AppName, Code),
        rule("com.microsoft.VSCode", BundleId, Code),
        // Windows exe `Code.exe` and Linux WM_CLASS "Code"/"code".
        exact_rule("Code", AppName, Code),
        exact_rule("code.exe", BundleId, Code),
        exact_rule("code", BundleId, Code),
        rule("Cursor", AppName, Code),
        rule("Zed", AppName, Code),
        rule("dev.zed", BundleId, Code),
        rule("Xcode", AppName, Code),
        rule("IntelliJ", AppName, Code),
        rule("WebStorm", AppName, Code),
        rule("PyCharm", AppName, Code),
        rule("GoLand", AppName, Code),
        rule("RustRover", AppName, Code),
        rule("com.jetbrains", BundleId, Code),
        rule("Windows Terminal", AppName, Code),
        rule("Terminal", AppName, Code),
        rule("iTerm", AppName, Code),
        rule("Warp", AppName, Code),
        rule("Ghostty", AppName, Code),
        rule("Alacritty", AppName, Code),
        rule("kitty", AppName, Code),
        rule("WezTerm", AppName, Code),
        // Casual chat
        rule("Slack", AppName, Casual),
        rule("Discord", AppName, Casual),
        rule("Messages", AppName, Casual),
        rule("WhatsApp", AppName, Casual),
        rule("Telegram", AppName, Casual),
        // Formal mail apps
        rule("Mail", AppName, Formal),
        rule("Outlook", AppName, Formal),
        // Web mail in a browser
        rule("Gmail", WindowTitle, Formal),
        rule("Outlook", WindowTitle, Formal),
    ]
}

fn field_value(ctx: &AppContext, field: MatchField) -> Option<&str> {
    match field {
        MatchField::AppName => Some(ctx.app_name.as_str()),
        MatchField::BundleId => ctx.bundle_id.as_deref(),
        MatchField::WindowTitle => ctx.window_title.as_deref(),
    }
}

/// Resolve the style for `ctx`. Without a context (or a match) the default
/// style applies. `Code` can only come from rules, never from the default.
pub fn resolve_style(settings: &CleanupSettings, ctx: Option<&AppContext>) -> CleanupStyle {
    let Some(ctx) = ctx else {
        return settings.default_style;
    };
    for r in &settings.app_style_rules {
        if r.pattern.is_empty() {
            continue;
        }
        if let Some(value) = field_value(ctx, r.field) {
            let (v, p) = (value.to_lowercase(), r.pattern.to_lowercase());
            if (r.exact && v == p) || (!r.exact && v.contains(&p)) {
                return r.style;
            }
        }
    }
    settings.default_style
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(app: &str, bundle: Option<&str>, title: Option<&str>) -> AppContext {
        AppContext {
            app_name: app.into(),
            bundle_id: bundle.map(Into::into),
            window_title: title.map(Into::into),
            secure_input: false,
        }
    }

    fn style(c: &AppContext) -> CleanupStyle {
        resolve_style(&CleanupSettings::default(), Some(c))
    }

    #[test]
    fn slack_is_casual() {
        assert_eq!(style(&ctx("Slack", None, None)), CleanupStyle::Casual);
    }

    #[test]
    fn gmail_in_chrome_is_formal() {
        let c = ctx("Google Chrome", None, Some("Inbox (3) - me@x.com - Gmail"));
        assert_eq!(style(&c), CleanupStyle::Formal);
    }

    #[test]
    fn vscode_is_code() {
        assert_eq!(
            style(&ctx("Visual Studio Code", None, None)),
            CleanupStyle::Code
        );
        assert_eq!(
            style(&ctx("Electron", Some("com.microsoft.VSCode"), None)),
            CleanupStyle::Code
        );
    }

    #[test]
    fn vscode_windows_and_linux_exact() {
        assert_eq!(style(&ctx("Code", None, None)), CleanupStyle::Code);
        assert_eq!(style(&ctx("x", Some("code.exe"), None)), CleanupStyle::Code);
        assert_eq!(style(&ctx("x", Some("Code"), None)), CleanupStyle::Code);
        // Exact: a different app merely containing "code" is not matched.
        assert_eq!(
            style(&ctx("Barcode Scanner", None, None)),
            CleanupStyle::Neutral
        );
    }

    #[test]
    fn editor_with_gmail_file_stays_code() {
        let c = ctx("Cursor", None, Some("Gmail.ts"));
        assert_eq!(style(&c), CleanupStyle::Code);
    }

    #[test]
    fn unknown_and_missing_are_neutral() {
        assert_eq!(
            style(&ctx("Notes", None, Some("Shopping"))),
            CleanupStyle::Neutral
        );
        assert_eq!(
            resolve_style(&CleanupSettings::default(), None),
            CleanupStyle::Neutral
        );
    }

    #[test]
    fn case_insensitive_and_first_match_wins() {
        assert_eq!(style(&ctx("iterm2", None, None)), CleanupStyle::Code);
        assert_eq!(
            style(&ctx("Windows Terminal", None, None)),
            CleanupStyle::Code
        );
        assert_eq!(style(&ctx("Apple Mail", None, None)), CleanupStyle::Formal);
    }
}
