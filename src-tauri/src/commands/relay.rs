//! Relay server configuration commands.
//!
//! Allows the user to configure a TCP relay server for NAT traversal fallback.
//! When configured, relay candidates are included in invites alongside direct
//! candidates. The relay is only used as a last resort (priority 0).

use std::sync::Arc;

use tauri::State;

use crate::relay::{RelayConfig, RelayState};
use crate::state::AppState;

/// Get the current relay server configuration.
#[tauri::command]
pub async fn get_relay_config(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<RelayConfig>, String> {
    let config = state.relay_config.read().await;
    Ok(config.clone())
}

/// Set the relay server configuration.
///
/// Pass `null` or an empty host to disable the relay.
/// When a valid config is set, relay candidates will be included in invites.
#[tauri::command]
pub async fn set_relay_config(
    state: State<'_, Arc<AppState>>,
    config: Option<RelayConfig>,
) -> Result<(), String> {
    // Validate the config if provided
    if let Some(ref cfg) = config {
        if cfg.host.trim().is_empty() {
            return Err("relay host cannot be empty".to_string());
        }
        if cfg.port == 0 {
            return Err("relay port must be > 0".to_string());
        }
        if cfg.auth_token.len() > 256 {
            return Err("auth token too long (max 256 chars)".to_string());
        }
    }

    let mut relay_cfg = state.relay_config.write().await;
    *relay_cfg = config.clone();

    // Reset relay state when config changes
    let mut relay_st = state.relay_state.write().await;
    *relay_st = RelayState::default();

    tracing::info!(configured = config.is_some(), "relay configuration updated");
    Ok(())
}

/// Get the current relay connection state (for frontend diagnostics).
#[tauri::command]
pub async fn get_relay_state(
    state: State<'_, Arc<AppState>>,
) -> Result<RelayState, String> {
    let relay_state = state.relay_state.read().await;
    Ok(relay_state.clone())
}
