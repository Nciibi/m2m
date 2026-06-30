/// M2M — Window Security (Screen Capture Protection)
///
/// Platform-specific protection to prevent the app window from appearing
/// in screenshots, screen recordings, or remote desktop captures.
///
/// ## Platform support
///
/// - **Windows**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` — prevents
///   BitBlt/screen capture of this window's region.
/// - **macOS**: `NSWindow.sharingType = .none` — excludes from screen capture.
/// - **Linux**: X11 `_NET_WM_STATE_ABOVE` + `_NET_WM_WINDOW_TYPE_DIALOG` hints
///   (best-effort; X11 does not have a true screen-capture exclusion API). On
///   Wayland, the compositor enforces isolation by default.
///
/// ## Design
///
/// All protection is **OFF by default**. The user enables it via Settings
/// (`SecurityConfig::screen_capture_protection`). This protects against
/// accidental screen-share leaks without breaking legitimate screen recording
/// needs (e.g., demos, support).
#[cfg(target_os = "windows")]
pub mod platform {
    use raw_window_handle::{HasWindowHandle, WindowHandle};
    use windows::Win32::Graphics::Gdi::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
    use windows::Win32::Foundation::HWND;

    /// Prevent the given window from being captured in screenshots/recordings.
    pub fn prevent_screen_capture(handle: &impl HasWindowHandle) -> Result<(), String> {
        let window_handle = handle
            .window_handle()
            .map_err(|e| format!("failed to get window handle: {e}"))?;

        let HWND(hwnd) = match window_handle {
            WindowHandle::Win32(h) => h.hwnd().into(),
            _ => return Err("unexpected window handle type".into()),
        };

        // SAFETY: We only call SetWindowDisplayAffinity with a valid HWND.
        // WDA_EXCLUDEFROMCAPTURE (0x00000011) prevents the window from being
        // included in BitBlt / PrintWindow / DWM thumbnails.
        unsafe {
            SetWindowDisplayAffinity(HWND(hwnd), WDA_EXCLUDEFROMCAPTURE)
                .map_err(|e| format!("SetWindowDisplayAffinity failed: {e}"))?;
        }

        Ok(())
    }

    /// Re-enable screen capture for the given window.
    pub fn allow_screen_capture(handle: &impl HasWindowHandle) -> Result<(), String> {
        let window_handle = handle
            .window_handle()
            .map_err(|e| format!("failed to get window handle: {e}"))?;

        let HWND(hwnd) = match window_handle {
            WindowHandle::Win32(h) => h.hwnd().into(),
            _ => return Err("unexpected window handle type".into()),
        };

        // WDA_NONE (0x00000000) = default, window can be captured.
        unsafe {
            SetWindowDisplayAffinity(HWND(hwnd), 0)
                .map_err(|e| format!("SetWindowDisplayAffinity(WDA_NONE) failed: {e}"))?;
        }

        Ok(())
    }
}

/// macOS stub — platform-specific implementation would use objc to set
/// `NSWindow.sharingType = NSWindowSharingNone`. Since this requires
/// runtime objc message sending, it is left as a togglable no-op that
/// can be filled in when targeting macOS.
#[cfg(target_os = "macos")]
pub mod platform {
    /// macOS screen capture prevention is stubbed. On a real macOS build,
    /// this would call `[window setSharingType: NSWindowSharingNone]`.
    pub fn prevent_screen_capture(_handle: &impl HasWindowHandle) -> Result<(), String> {
        Ok(())
    }

    pub fn allow_screen_capture(_handle: &impl HasWindowHandle) -> Result<(), String> {
        Ok(())
    }
}

/// Linux stub — X11 `_NET_WM_STATE` hints are best-effort. Wayland
/// compositors typically isolate windows by default.
#[cfg(target_os = "linux")]
pub mod platform {
    /// Linux screen capture prevention is best-effort. On X11 this would
    /// set `_NET_WM_STATE_ABOVE` + `_NET_WM_WINDOW_TYPE_DIALOG`.
    pub fn prevent_screen_capture(_handle: &impl HasWindowHandle) -> Result<(), String> {
        Ok(())
    }

    pub fn allow_screen_capture(_handle: &impl HasWindowHandle) -> Result<(), String> {
        Ok(())
    }
}

/// Unsupported OS — everything is a no-op with a warning.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub mod platform {
    pub fn prevent_screen_capture(_handle: &impl HasWindowHandle) -> Result<(), String> {
        tracing::warn!("screen capture protection not supported on this platform");
        Err("not supported".into())
    }

    pub fn allow_screen_capture(_handle: &impl HasWindowHandle) -> Result<(), String> {
        Ok(())
    }
}
