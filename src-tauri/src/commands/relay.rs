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

/// Get the current relay server configuration, **without the secret**.
///
/// This is an IPC boundary, so the shape that crosses it is ours to choose.
/// Returning `RelayConfig` directly handed the plaintext auth token to the
/// webview, where it sat in renderer memory and in any DevTools console — and
/// `#[derive(Debug)]` on `RelayConfig` meant a single `tracing::debug!(?config)`
/// anywhere would print it to the log file. The UI only ever needs to know
/// whether a token is *set*, so that is what it gets.
#[tauri::command]
pub async fn get_relay_config(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<RelayConfigView>, AppError> {
    let config = state.relay_config.read().await;
    Ok(config.as_ref().map(RelayConfigView::from))
}

/// The relay configuration as exposed to the frontend.
///
/// `auth_token` is reduced to a boolean. The real value never leaves Rust.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RelayConfigView {
    pub host: String,
    pub port: u16,
    /// Whether an auth token is configured. Never the token itself.
    pub has_auth_token: bool,
}

impl From<&RelayConfig> for RelayConfigView {
    fn from(c: &RelayConfig) -> Self {
        Self {
            host: c.host.clone(),
            port: c.port,
            has_auth_token: !c.auth_token.is_empty(),
        }
    }
}

/// Set the relay server configuration.
///
/// Pass `null` or an empty host to disable the relay.
/// When a valid config is set, relay candidates will be included in invites.
#[tauri::command]
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
#[tauri::command]
pub async fn get_relay_state(state: State<'_, Arc<AppState>>) -> Result<RelayState, AppError> {
    let relay_state = state.relay_state.read().await;
    Ok(relay_state.clone())
}
