//! Xerra: capture of the frontmost application / window at recording start.
//! Used to pick a cleanup style (see `cleanup::style`). Best-effort on every
//! platform; returns `None` when nothing can be determined.
//!
//! The result is only used to choose a style. The window title is never sent
//! to a remote service when `secure_input` is true.

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
    imp::capture()
}

#[cfg(target_os = "macos")]
mod imp {
    use super::AppContext;
    use objc2::rc::Retained;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    use std::ffi::c_void;

    type AXError = i32;
    const AX_SUCCESS: AXError = 0;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
        fn AXUIElementCopyAttributeValue(
            element: *const c_void,
            attribute: *const c_void,
            value: *mut *const c_void,
        ) -> AXError;
        fn AXUIElementSetMessagingTimeout(element: *const c_void, seconds: f32) -> AXError;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    /// Owned CF object, released on drop.
    struct CfOwned(*const c_void);

    impl Drop for CfOwned {
        fn drop(&mut self) {
            // SAFETY: pointer is non-null and owned (+1) by this wrapper.
            unsafe { CFRelease(self.0) };
        }
    }

    /// Copy an AX attribute. NSString is toll-free bridged to CFString, so
    /// the attribute name can be passed as an `NSString` pointer.
    fn copy_attr(element: &CfOwned, name: &str) -> Option<CfOwned> {
        let key = NSString::from_str(name);
        let mut out: *const c_void = std::ptr::null();
        // SAFETY: element is a valid AXUIElementRef; key outlives the call;
        // out is a valid out-pointer. On success the value is +1 retained.
        let err = unsafe {
            AXUIElementCopyAttributeValue(
                element.0,
                &*key as *const NSString as *const c_void,
                &mut out,
            )
        };
        if err == AX_SUCCESS && !out.is_null() {
            Some(CfOwned(out))
        } else {
            None
        }
    }

    fn window_title(pid: i32) -> Option<String> {
        // SAFETY: plain C call; returns a +1 retained element or null.
        let app = unsafe { AXUIElementCreateApplication(pid) };
        if app.is_null() {
            return None;
        }
        let app = CfOwned(app);
        // Keep an unresponsive target app from stalling recording start.
        // SAFETY: valid element.
        unsafe { AXUIElementSetMessagingTimeout(app.0, 0.1) };
        let window = copy_attr(&app, "AXFocusedWindow")?;
        let title = copy_attr(&window, "AXTitle")?;
        // SAFETY: AXTitle is a CFString (toll-free bridged to NSString); we
        // take over the +1 reference, so it is released when `s` drops.
        // Forget the wrapper to avoid a double release.
        let ptr = title.0;
        std::mem::forget(title);
        let s: Retained<NSString> = unsafe { Retained::from_raw(ptr as *mut NSString)? };
        let s = s.to_string();
        (!s.is_empty()).then_some(s)
    }

    pub fn capture() -> Option<AppContext> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let app_name = app.localizedName().map(|n| n.to_string())?;
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let secure_input = crate::secure_input::is_enabled_now();
        // Title is None without Accessibility permission.
        let window_title = window_title(app.processIdentifier());
        Some(AppContext {
            app_name,
            bundle_id,
            window_title,
            secure_input,
        })
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::AppContext;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    fn exe_path(pid: u32) -> Option<String> {
        // SAFETY: handle is closed below; buffer length is passed in `len`.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let res = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(handle);
            res.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    pub fn capture() -> Option<AppContext> {
        // SAFETY: Win32 calls with valid out-buffers.
        let (title, pid) = unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(hwnd, &mut buf);
            let title = (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]));
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            (title, pid)
        };
        if pid == 0 {
            return None;
        }
        let path = exe_path(pid)?;
        let file_name = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
        let app_name = match file_name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => file_name.clone(),
        };
        Some(AppContext {
            app_name,
            bundle_id: Some(file_name),
            window_title: title,
            secure_input: false,
        })
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::AppContext;
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const XPROP_TIMEOUT: Duration = Duration::from_millis(150);

    /// Run xprop with a short timeout; None if missing, failing, or slow.
    fn xprop(args: &[&str]) -> Option<String> {
        let mut child = Command::new("xprop")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let start = Instant::now();
        loop {
            match child.try_wait().ok()? {
                Some(status) if status.success() => break,
                Some(_) => return None,
                None if start.elapsed() > XPROP_TIMEOUT => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                None => std::thread::sleep(Duration::from_millis(3)),
            }
        }
        let mut out = String::new();
        child.stdout.take()?.read_to_string(&mut out).ok()?;
        Some(out)
    }

    /// Parse `_NET_ACTIVE_WINDOW(WINDOW): window id # 0x4a00007`.
    pub(super) fn parse_active_window(out: &str) -> Option<String> {
        let id = out.split('#').nth(1)?.trim().split([',', ' ']).next()?;
        (id.starts_with("0x") && id != "0x0").then(|| id.to_string())
    }

    /// Extract the quoted strings from the right side of an xprop line.
    fn quoted_values(line: &str) -> Vec<String> {
        let rest = match line.split_once('=') {
            Some((_, r)) => r,
            None => return Vec::new(),
        };
        let mut vals = Vec::new();
        let mut cur: Option<String> = None;
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match (&mut cur, c) {
                (None, '"') => cur = Some(String::new()),
                (Some(_), '"') => vals.extend(cur.take()),
                (Some(s), '\\') => {
                    if let Some(n) = chars.next() {
                        s.push(n);
                    }
                }
                (Some(s), c) => s.push(c),
                _ => {}
            }
        }
        vals
    }

    /// Parse `xprop -id <id> WM_CLASS _NET_WM_NAME` output into
    /// (wm_class instance, wm_class class, title).
    pub(super) fn parse_props(out: &str) -> (Option<String>, Option<String>, Option<String>) {
        let (mut instance, mut class, mut title) = (None, None, None);
        for line in out.lines() {
            if line.starts_with("WM_CLASS") {
                let mut v = quoted_values(line).into_iter();
                instance = v.next().filter(|s| !s.is_empty());
                class = v.next().filter(|s| !s.is_empty());
            } else if line.starts_with("_NET_WM_NAME") {
                title = quoted_values(line)
                    .into_iter()
                    .next()
                    .filter(|s| !s.is_empty());
            }
        }
        (instance, class, title)
    }

    pub fn capture() -> Option<AppContext> {
        // X11 only; Wayland offers no portable way to query the focused window.
        if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
            return None;
        }
        let root = xprop(&["-root", "_NET_ACTIVE_WINDOW"])?;
        let id = parse_active_window(&root)?;
        let props = xprop(&["-id", &id, "WM_CLASS", "_NET_WM_NAME"])?;
        let (instance, class, title) = parse_props(&props);
        let app_name = class.clone().or_else(|| instance.clone())?;
        Some(AppContext {
            app_name,
            bundle_id: instance.or(class),
            window_title: title,
            secure_input: false,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_active_window() {
            assert_eq!(
                parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x4a00007\n"),
                Some("0x4a00007".to_string())
            );
            assert_eq!(
                parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0\n"),
                None
            );
            assert_eq!(parse_active_window("garbage"), None);
        }

        #[test]
        fn parses_props() {
            let out = "WM_CLASS(STRING) = \"slack\", \"Slack\"\n_NET_WM_NAME(UTF8_STRING) = \"general - \\\"Acme\\\"\"\n";
            let (i, c, t) = parse_props(out);
            assert_eq!(i.as_deref(), Some("slack"));
            assert_eq!(c.as_deref(), Some("Slack"));
            assert_eq!(t.as_deref(), Some("general - \"Acme\""));
        }

        #[test]
        fn missing_props() {
            let out = "WM_CLASS:  not found.\n_NET_WM_NAME:  not found.\n";
            assert_eq!(parse_props(out), (None, None, None));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod imp {
    use super::AppContext;

    pub fn capture() -> Option<AppContext> {
        None
    }
}
