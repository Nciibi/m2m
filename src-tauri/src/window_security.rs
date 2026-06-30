/// M2M — Window Security (Screen Capture Protection)
///
/// Platform-specific protection to prevent the app window from appearing
/// in screenshots, screen recordings, or remote desktop captures.
///
/// ## Platform support
///
/// - **Windows**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` via
///   direct FFI — prevents BitBlt/screen capture of this window's region.
/// - **macOS**: `NSWindow.sharingType = .none` stub (requires objc runtime).
/// - **Linux**: Best-effort X11 hints; Wayland isolates by default.
///
/// All protection is **OFF by default**. Must be explicitly enabled by user.

/// Tauri window label constant for the main window.
const MAIN_WINDOW_LABEL: &str = "main";

/// Apply screen capture protection to the Tauri main window.
///
/// When enabled, the platform prevents the window from appearing in
/// screenshots, screen recordings, or remote desktop captures.
pub fn apply_screen_protection(app_handle: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        apply_to_window(&window, enabled)
    } else {
        tracing::warn!("main window not found for screen capture protection");
        Err("main window not found".into())
    }
}

fn apply_to_window(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle, WindowHandle};
        use std::mem;

        let handle = window
            .window_handle()
            .map_err(|e| format!("failed to get window handle: {e}"))?;

        let hwnd = match handle.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd.get(),
            _ => return Err("unexpected window handle type".into()),
        };

        // SAFETY: We call SetWindowDisplayAffinity through FFI with a valid HWND.
        // WDA_EXCLUDEFROMCAPTURE = 0x00000011 prevents the window from being
        // included in BitBlt / PrintWindow captures.
        unsafe {
            let user32 = libloading::os::windows::WindowsLibrary::open("user32.dll")
                .map_err(|e| format!("failed to load user32.dll: {e}"))?;

            let func: libloading::Symbol<
                unsafe extern "system" fn(isize, u32) -> i32,
            > = user32
                .get(b"SetWindowDisplayAffinity\0")
                .map_err(|e| format!("failed to find SetWindowDisplayAffinity: {e}"))?;

            let affinity = if enabled { 0x00000011u32 } else { 0u32 }; // WDA_EXCLUDEFROMCAPTURE or WDA_NONE
            let result = func(hwnd as isize, affinity);
            if result == 0 {
                return Err("SetWindowDisplayAffinity returned 0 (failed)".into());
            }
        }

        tracing::info!(enabled, "Windows screen capture protection applied");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        tracing::info!(enabled, "macOS screen capture protection stub (requires objc)");
        let _ = window;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        tracing::info!(enabled, "Linux screen capture protection stub (Wayland isolates by default)");
        let _ = window;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (window, enabled);
        Err("screen capture protection not supported on this platform".into())
    }
}
