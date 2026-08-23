//! M2M — Security Commands
//!
//! Manages screen capture protection, clipboard auto-clear,
//! and security configuration toggles for the frontend.
//!
//! ## Persistence & re-application
//!
//! The security config is persisted to `<data_dir>/security.json` on every
//! change and re-applied:
//! - at app startup (setup hook in lib.rs) — survives restarts,
//! - when the frontend mounts / the webview is recreated
//!   (`reapply_security_config` command),
//! - whenever the main window regains focus (window event hook).
//!
//! This closes the "silent protection drop" gap: a webview recreation or an
//! app restart no longer leaves capture protection off.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::state::{AppState, SecurityConfig};
use crate::window_security;

/// Filename (inside the data directory) for the persisted security config.
const SECURITY_CONFIG_FILE: &str = "security.json";

// ─── Persistence helpers ────────────────────────────────────────────────────

fn config_path(data_dir: &str) -> std::path::PathBuf {
    std::path::Path::new(data_dir).join(SECURITY_CONFIG_FILE)
}

/// Persist config to disk. Failures are logged, never fatal: a missed write
/// must not take the app down; worst case a restart restores defaults (OFF),
/// which matches privacy-first fail-safe semantics.
pub fn persist_config(data_dir: &str, config: &SecurityConfig) {
    let path = config_path(data_dir);
    let json = match serde_json::to_string_pretty(config) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(error = %e, "failed to serialize security config");
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, json) {
        tracing::error!(path = %path.display(), error = %e, "failed to write security config");
    }
}

/// Load config from disk at startup. Corrupt files are quarantined (renamed)
/// rather than silently overwritten, so the user can inspect what happened.
pub fn load_config(data_dir: &str) -> Option<SecurityConfig> {
    let path = config_path(data_dir);
    let raw = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<SecurityConfig>(&raw) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "corrupt security config — quarantining");
            let _ = std::fs::rename(
                &path,
                path.with_extension(format!("json.corrupt-{}", chrono::Utc::now().timestamp())),
            );
            None
        }
    }
}

// ─── Monitor lifecycle ──────────────────────────────────────────────────────

/// Start or stop the capture-software detector to match the config.
fn sync_capture_monitor(state: &Arc<AppState>, app_handle: &AppHandle, enabled: bool) {
    if enabled {
        crate::capture_monitor::start_monitor(state.clone(), app_handle.clone());
    } else {
        // The running monitor observes the toggle each cycle and exits by
        // itself (emitting a cleared warning first). Nothing to do here.
    }
}

/// Apply every platform-side effect implied by `config`.
pub fn apply_side_effects(state: &Arc<AppState>, app_handle: &AppHandle, config: &SecurityConfig) {
    // Traffic-analysis knobs are process-global; cheap to (re)set always.
    crate::session::set_send_jitter_ms(config.send_batching_ms);

    if config.screen_capture_protection {
        if let Err(e) = window_security::apply_screen_protection(app_handle, true) {
            // Surface loudly: user believes they are protected but we failed.
            tracing::error!(error = %e, "screen capture protection FAILED to apply");
            let _ = tauri::Emitter::emit(app_handle, "m2m://security-error", serde_json::json!({
                "source": "screen_capture_protection",
                "message": e,
            }));
        }
    } else {
        let _ = window_security::apply_screen_protection(app_handle, false);
    }

    sync_capture_monitor(state, app_handle, config.capture_process_detection);
}

// ─── Commands ───────────────────────────────────────────────────────────────

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
/// protection is applied or removed immediately. The full config is
/// persisted and drives background monitors.
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

    // Capture monitor lifecycle follows its own toggle
    if config.capture_process_detection != old_config.capture_process_detection {
        sync_capture_monitor(&state, &app_handle, config.capture_process_detection);
    }

    // Persist config (disk) + runtime state
    persist_config(&state.data_dir, &config);
    {
        let mut sc = state.security_config.write().await;
        *sc = config.clone();
    }

    Ok(config)
}

/// Re-apply the CURRENT security config's platform side effects.
///
/// Called by the frontend on mount (covers webview recreation/reload) so
/// capture protection is never silently dropped after an internal reload.
/// Idempotent and cheap.
#[tauri::command]
pub async fn reapply_security_config(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let config = state.security_config.read().await.clone();
    apply_side_effects(&state, &app_handle, &config);
    tracing::debug!("security config side effects re-applied");
    Ok(())
}

/// Honest capability report for screen-capture protection on THIS platform,
/// so the Settings UI can show exactly what the toggle does and does not do.
#[tauri::command]
pub async fn get_capture_capability() -> Result<crate::window_security::CaptureCapability, String> {
    Ok(window_security::platform_capability())
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
