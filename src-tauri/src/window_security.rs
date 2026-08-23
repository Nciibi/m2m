/// M2M — Window Security (Screen Capture Protection)
///
/// Platform-specific protection to prevent the app window from appearing
/// in screenshots, screen recordings, or remote desktop captures.
///
/// ## Platform support
///
/// - **Windows**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` via
///   direct FFI — prevents BitBlt/screen capture of this window's region.
/// - **macOS**: `NSWindow.sharingType = .none` via libobjc FFI — excludes
///   the window from ScreenCaptureKit/screencapture and window streaming.
/// - **Linux**: Wayland = best-effort (compositor isolates by default, but
///   a granted screen-share portal captures everything); X11 = NOT
///   SUPPORTED (any X client can read other windows' contents) — enabling
///   the toggle there reports an error instead of pretending.
///
/// All protection is **OFF by default**. Must be explicitly enabled by user.
use tauri::Manager;

const MAIN_WINDOW_LABEL: &str = "main";

/// Honest capability report for the Settings UI, per current platform.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptureCapability {
    /// "full" | "partial" | "unsupported"
    pub level: &'static str,
    /// Human-readable explanation of what is and isn't covered.
    pub note: &'static str,
}

pub fn platform_capability() -> CaptureCapability {
    #[cfg(target_os = "windows")]
    {
        CaptureCapability {
            level: "full",
            note: "Windows WDA_EXCLUDEFROMCAPTURE: hidden from screenshots, recordings, and most capture apps. Cannot defeat kernel-level or hardware (HDMI) capture.",
        }
    }
    #[cfg(target_os = "macos")]
    {
        CaptureCapability {
            level: "partial",
            note: "macOS NSWindowSharingNone: excluded from screenshots and ScreenCaptureKit streams. Does not defeat kernel-level or hardware capture.",
        }
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE").map(|v| v == "wayland").unwrap_or(false)
        {
            CaptureCapability {
                level: "partial",
                note: "Wayland: compositor isolates windows by default, but a screen-share portal you grant can still record the whole screen.",
            }
        } else {
            CaptureCapability {
                level: "unsupported",
                note: "X11 cannot exclude a window from capture — any X11 client can read other windows' contents. Use Wayland for best-effort isolation.",
            }
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        CaptureCapability { level: "unsupported", note: "Unsupported platform." }
    }
}

/// Apply screen capture protection to the Tauri main window.
///
/// When enabled, the platform prevents the window from appearing in
/// screenshots, screen recordings, or remote desktop captures (to the
/// extent the platform supports — see [`platform_capability`]).
pub fn apply_screen_protection(app_handle: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        apply_to_window(&window, enabled)
    } else {
        tracing::warn!("main window not found for screen capture protection");
        Err("main window not found".into())
    }
}

#[cfg(target_os = "windows")]
fn apply_to_window(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    use raw_window_handle::HasWindowHandle;

    let handle = window
        .window_handle()
        .map_err(|e| format!("failed to get window handle: {e}"))?;

    let raw = handle.as_raw();
    let hwnd: isize = match raw {
        raw_window_handle::RawWindowHandle::Win32(h) => h.hwnd.get(),
        _ => return Err("unexpected window handle type".into()),
    };

    // SAFETY: Call SetWindowDisplayAffinity through FFI with a valid HWND.
    // WDA_EXCLUDEFROMCAPTURE = 0x00000011 prevents screen capture.
    unsafe {
        let user32 = libloading::Library::new("user32.dll")
            .map_err(|e| format!("failed to load user32.dll: {e}"))?;

        let func: libloading::Symbol<
            unsafe extern "system" fn(isize, u32) -> i32,
        > = user32
            .get(b"SetWindowDisplayAffinity\0")
            .map_err(|e| format!("failed to find SetWindowDisplayAffinity: {e}"))?;

        let affinity = if enabled { 0x00000011u32 } else { 0u32 };
        let result = func(hwnd, affinity);
        if result == 0 {
            return Err("SetWindowDisplayAffinity returned 0 (failed)".into());
        }
    }

    tracing::info!(enabled, "Windows screen capture protection applied");
    Ok(())
}

#[cfg(target_os = "macos")]
mod mac_ffi {
    use std::ffi::c_void;

    /// Objective-C runtime selector registry.
    #[link(name = "objc", kind = "dylib")]
    extern "C" {
        pub fn sel_registerName(name: *const i8) -> *const c_void;
    }

    /// objc_msgSend casts, matched exactly to the signatures we use.
    /// This is the documented way to call ObjC methods without variadics:
    /// each call site must use the variant matching its argument/return
    /// types (arm64/x86_64 ABI).
    #[link(name = "objc", kind = "dylib")]
    extern "C" {
        /// `[receiver sel] -> id`
        pub fn msg_send_id(receiver: *mut c_void, sel: *const c_void) -> *mut c_void;
        /// `[receiver sel arg:NSUInteger] -> void`
        pub fn msg_send_ulong(receiver: *mut c_void, sel: *const c_void, arg: usize);
    }

    pub fn sel(name: &str) -> *const c_void {
        let c = std::ffi::CString::new(name).expect("selector name has no NUL");
        unsafe { sel_registerName(c.as_ptr()) }
    }
}

#[cfg(target_os = "macos")]
fn apply_to_window(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;
    use mac_ffi::{msg_send_id, msg_send_ulong, sel};

    let handle = window
        .window_handle()
        .map_err(|e| format!("failed to get window handle: {e}"))?;

    let ns_view: *mut c_void = match handle.as_raw() {
        RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
        _ => return Err("unexpected window handle type".into()),
    };
    if ns_view.is_null() {
        return Err("NSView pointer is null".into());
    }

    unsafe {
        // NSWindowSharingNone = 0 in NSWindowSharing enum (NSUInteger).
        const NS_WINDOW_SHARING_NONE: usize = 0;

        // [nsView window] -> NSWindow*
        let get_window = sel("window");
        let msg_get: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
            std::mem::transmute(msg_send_id as *const ());
        let ns_window = msg_get(ns_view, get_window);
        if ns_window.is_null() {
            return Err("NSView is not attached to an NSWindow yet".into());
        }

        // [window setSharingType:NSWindowSharingNone]
        let set_sharing = sel("setSharingType:");
        let msg_set: unsafe extern "C" fn(*mut c_void, *const c_void, usize) =
            std::mem::transmute(msg_send_ulong as *const ());
        if enabled {
            msg_set(ns_window, set_sharing, NS_WINDOW_SHARING_NONE);
        } else {
            // NSWindowSharingReadWrite = 2 — the default sharing mode.
            msg_set(ns_window, set_sharing, 2);
        }
    }

    tracing::info!(enabled, "macOS screen capture protection applied (sharingType)");
    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_to_window(_window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    let on_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE").map(|v| v == "wayland").unwrap_or(false);

    if !enabled {
        tracing::debug!("Linux screen capture protection disabled (was best-effort at most)");
        return Ok(());
    }

    if on_wayland {
        // Honest best-effort: the compositor isolates unmapped/unshared
        // windows by default, but any portal grant overrides it. Nothing
        // to set — report success WITH the caveat logged and surfaced in
        // the UI via platform_capability().
        tracing::warn!(
            "Wayland: capture protection is best-effort only — a granted \
             screen-share portal can still capture the screen"
        );
        Ok(())
    } else {
        // X11: NEVER pretend. Any X client can read our window contents.
        tracing::warn!("X11: capture exclusion is not possible — refusing to fake it");
        Err(
            "X11 cannot exclude this window from capture. Run under Wayland \
             for best-effort isolation."
                .into(),
        )
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply_to_window(_window: &tauri::WebviewWindow, _enabled: bool) -> Result<(), String> {
    Err("screen capture protection not supported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_report_is_honest_per_platform() {
        let cap = platform_capability();
        match cap.level {
            "full" => assert!(cap.note.contains("WDA")),
            "partial" => assert!(!cap.note.is_empty()),
            "unsupported" => assert!(cap.note.contains("X11") || cap.note.contains("Unsupported")),
            other => panic!("unknown capability level: {other}"),
        }
    }
}
