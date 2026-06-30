//! M2M — Security Commands
//!
//! Manages screen capture protection, clipboard auto-clear,
//! and security configuration toggles for the frontend.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::state::{AppState, SecurityConfig};
use crate::window_security;

/// Get the current security configuration.
#[tauri::command]
pub async fn get_security_config(
    state: State<'_, Arc<AppState>>,
) -> Result<SecurityConfig, String> {
    let config = state.security_config.read().await;
    Ok(config.clone())
}

/// Update the security configuration.
///
/// When `screen_capture_protection` changes, the platform window
/// protection is applied or removed immediately.
/// When `clipboard_clear_secs` changes, the new timeout is stored
/// for the frontend to use on subsequent clipboard writes.
#[tauri::command]
pub async fn set_security_config(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    config: SecurityConfig,
) -> Result<SecurityConfig, String> {
    let old_config = state.security_config.read().await.clone();

    // Handle screen capture protection toggle
    if config.screen_capture_protection != old_config.screen_capture_protection {
        if config.screen_capture_protection {
            window_security::apply_screen_protection(&app_handle, true)?;
            tracing::info!("Screen capture protection ENABLED");
        } else {
            window_security::apply_screen_protection(&app_handle, false)?;
            tracing::info!("Screen capture protection DISABLED");
        }
    }

    // Persist config
    {
        let mut sc = state.security_config.write().await;
        *sc = config.clone();
    }

    Ok(config)
}

/// Clear the system clipboard.
///
/// Called by the frontend after the auto-clear timer fires,
/// or manually from the settings panel.
#[tauri::command]
pub async fn clear_clipboard() -> Result<(), String> {
    // Use Tauri clipboard plugin if available, fall back to web API.
    // The frontend can also call navigator.clipboard.writeText("") as a fallback.
    #[cfg(target_os = "windows")]
    {
        // On Windows, we can clear clipboard via FFI
        unsafe {
            let user32 = libloading::os::windows::WindowsLibrary::open("user32.dll")
                .map_err(|e| format!("failed to load user32.dll: {e}"))?;

            let func: libloading::Symbol<unsafe extern "system" fn() -> i32> = user32
                .get(b"OpenClipboard\0")
                .map_err(|e| format!("failed to find OpenClipboard: {e}"))?;

            let result = func();
            if result == 0 {
                // Clipboard may already be open by another app — that's fine
                return Ok(());
            }

            let empty_func: libloading::Symbol<unsafe extern "system" fn() -> i32> = user32
                .get(b"EmptyClipboard\0")
                .map_err(|e| format!("failed to find EmptyClipboard: {e}"))?;

            empty_func();

            let close_func: libloading::Symbol<unsafe extern "system" fn() -> i32> = user32
                .get(b"CloseClipboard\0")
                .map_err(|e| format!("failed to find CloseClipboard: {e}"))?;

            close_func();
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On macOS/Linux, clipboard clear is handled by the frontend
        // via `navigator.clipboard.writeText("")`.
        tracing::debug!("clipboard clear requested — frontend will handle");
    }

    Ok(())
}
