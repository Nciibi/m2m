//! M2M — Security Commands
//!
//! Manages screen capture protection, clipboard auto-clear,
//! and security configuration toggles for the frontend.

use std::sync::Arc;

use tauri::{AppHandle, State};

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
    #[cfg(target_os = "windows")]
    {
        unsafe {
            if let Ok(user32) = libloading::Library::new("user32.dll") {
                if let Ok(func) = user32.get::<unsafe extern "system" fn() -> i32>(b"OpenClipboard\0") {
                    let result = func();
                    if result != 0 {
                        if let Ok(empty_func) = user32.get::<unsafe extern "system" fn() -> i32>(b"EmptyClipboard\0") {
                            empty_func();
                        }
                        if let Ok(close_func) = user32.get::<unsafe extern "system" fn() -> i32>(b"CloseClipboard\0") {
                            close_func();
                        }
                    }
                }
            }
        }
        tracing::debug!("Windows clipboard cleared via FFI");
    }

    #[cfg(not(target_os = "windows"))]
    {
        tracing::debug!("Clipboard clear requested — frontend handles via web API");
    }

    Ok(())
}
