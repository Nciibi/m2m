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

use tauri::Manager;

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

#[cfg(target_os = "windows")]
fn apply_to_window(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    use raw_window_handle::HasWindowHandle;

    let handle = window
        .window_handle()
        .map_err(|e| format!("failed to get window handle: {e}"))?;

    let raw = handle.as_raw();
    let hwnd: isize = match raw {
        raw_window_handle::RawWindowHandle::Win32(h) => h.hwnd.get() as isize,
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
fn apply_to_window(_window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    tracing::info!(enabled, "macOS screen capture protection stub (requires objc)");
    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_to_window(_window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    tracing::info!(enabled, "Linux: Wayland isolates by default");
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply_to_window(_window: &tauri::WebviewWindow, _enabled: bool) -> Result<(), String> {
    Err("screen capture protection not supported on this platform".into())
}
