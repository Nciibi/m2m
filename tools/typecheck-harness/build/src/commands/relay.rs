//! Relay server configuration commands.
//!
//! Allows the user to configure a TCP relay server for NAT traversal fallback.
//! When configured, relay candidates are included in invites alongside direct
//! candidates. The relay is only used as a last resort (priority 0).

use crate::error::AppError;
use std::sync::Arc;

use tauri::State;

use crate::relay::{RelayConfig, RelayState};
use crate::state::AppState;

/// Get the current relay server configuration.
pub async fn get_relay_config(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<RelayConfig>, AppError> {
    let config = state.relay_config.read().await;
    Ok(config.clone())
}

/// Set the relay server configuration.
///
/// Pass `null` or an empty host to disable the relay.
/// When a valid config is set, relay candidates will be included in invites.
pub async fn set_relay_config(
    state: State<'_, Arc<AppState>>,
    config: Option<RelayConfig>,
) -> Result<(), AppError> {
    // Validate the config if provided
    if let Some(ref cfg) = config {
        if cfg.host.trim().is_empty() {
            return Err(AppError::invalid("relay host cannot be empty"));
        }
        if cfg.port == 0 {
            return Err(AppError::invalid("relay port must be > 0"));
        }
        if cfg.auth_token.len() > 256 {
            return Err(AppError::invalid("auth token too long (max 256 chars)"));
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
pub async fn get_relay_state(state: State<'_, Arc<AppState>>) -> Result<RelayState, AppError> {
    let relay_state = state.relay_state.read().await;
    Ok(relay_state.clone())
}
