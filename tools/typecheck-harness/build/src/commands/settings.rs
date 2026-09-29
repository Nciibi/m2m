//! Network settings and diagnostics commands.
//!
//! Handles STUN discovery, Tor proxy configuration, private mode,
//! connectivity checks, and full network diagnostics for the frontend.

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use tauri::State;

use crate::candidate;
use crate::state::AppState;
use crate::stun;
use crate::tor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePreference {
    pub theme: String,
    #[serde(default = "default_accent")]
    pub accent_color: String,
}

fn default_accent() -> String {
    "#6366f1".to_string()
}

/// Get the user's theme preference.
pub async fn get_theme_preference(
    state: State<'_, Arc<AppState>>,
) -> Result<ThemePreference, AppError> {
    let theme_str = state.theme_preference.read().await;
    let accent = state.accent_color.read().await;
    Ok(ThemePreference {
        theme: theme_str.clone(),
        accent_color: accent.clone(),
    })
}

/// Set the user's theme preference.
pub async fn set_theme_preference(
    state: State<'_, Arc<AppState>>,
    theme: String,
    accent_color: Option<String>,
) -> Result<(), AppError> {
    let valid = ["light", "dark", "system"];
    if !valid.contains(&theme.as_str()) {
        return Err(AppError::invalid("Invalid theme value"));
    }
    let mut tp = state.theme_preference.write().await;
    *tp = theme;
    if let Some(color) = accent_color {
        let mut ac = state.accent_color.write().await;
        *ac = color;
    }
    Ok(())
}

/// Discover the public IP address using enhanced STUN (parallel queries + consensus).
pub async fn discover_public_ip(state: State<'_, Arc<AppState>>) -> Result<String, AppError> {
    state.ensure_not_air_gapped().await?;
    let result = state
        .refresh_stun()
        .await
        .map_err(|e| AppError::invalid(format!("STUN discovery failed: {e}")))?;

    let addr = result
        .consensus_addr
        .map(|a| a.to_string())
        .unwrap_or_else(|| "no consensus".to_string());

    // Capture values before the tracing macro to avoid Send issues.
    let nat_type_str = state.nat_type.read().await.to_string();
    tracing::info!(
        servers = result.responding_servers,
        total = result.total_servers,
        consensus = result.consensus,
        public_ip = %addr,
        nat_type = %nat_type_str,
        "STUN discovery completed"
    );

    Ok(addr)
}

/// Get the current STUN configuration.
pub async fn get_stun_config(state: State<'_, Arc<AppState>>) -> Result<stun::StunConfig, AppError> {
    let config = state.stun_config.read().await;
    Ok(config.clone())
}

/// Upper bound on configured STUN servers. One task and one socket per entry.
const MAX_STUN_SERVERS: usize = 8;

/// Update the STUN server list and configuration.
pub async fn set_stun_servers(
    state: State<'_, Arc<AppState>>,
    servers: Vec<String>,
) -> Result<(), AppError> {
    if servers.is_empty() {
        return Err(AppError::invalid("STUN server list cannot be empty"));
    }
    // Cap the list. `discover_public_addrs` spawns one task and one socket per
    // entry, so an unbounded list is a task/socket exhaustion primitive
    // reachable straight from the renderer.
    if servers.len() > MAX_STUN_SERVERS {
        return Err(AppError::invalid(format!(
            "too many STUN servers: {} (max {})",
            servers.len(),
            MAX_STUN_SERVERS
        )));
    }
    // Each entry must be a real `host:port` and must parse.
    //
    // The previous check was `s.contains(':')`, which accepts `"a:b"`, an
    // empty host, and a non-numeric port. Whatever passes here is DNS-resolved
    // and sent a UDP datagram from the user's real address, so this list is a
    // probe target: it must be validated, not merely colon-checked.
    for s in &servers {
        if s.len() > 255 {
            return Err(AppError::invalid(format!("STUN server address too long: {s}")));
        }
        if s.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(AppError::invalid(format!("invalid STUN server address: {s}")));
        }
        let (host, port) = s
            .rsplit_once(':')
            .ok_or_else(|| format!("invalid STUN server address (missing port): {s}"))?;
        if host.is_empty() {
            return Err(AppError::invalid(format!("invalid STUN server address (missing host): {s}")));
        }
        if host.contains(':') && !host.starts_with('[') {
            return Err(AppError::invalid(format!(
                "invalid STUN server address (bare IPv6 must be bracketed): {s}"
            )));
        }
        let port: u16 = port
            .parse()
            .map_err(|_| format!("invalid STUN server port: {s}"))?;
        if port == 0 {
            return Err(AppError::invalid(format!("invalid STUN server port (0): {s}")));
        }
    }

    let mut config = state.stun_config.write().await;
    config.servers = servers;
    tracing::info!("STUN configuration updated");
    Ok(())
}

/// Toggle private mode (don't expose public IP in invites).
pub async fn set_private_mode(
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), AppError> {
    let mut pm = state.private_mode.write().await;
    *pm = enabled;
    let mut config = state.stun_config.write().await;
    config.private_mode = enabled;
    tracing::info!(private_mode = enabled, "privacy mode updated");
    Ok(())
}

/// Run connectivity verification: check if the listening port is reachable.
pub async fn check_connectivity(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::ConnectivityStatus, AppError> {
    state.ensure_not_air_gapped().await?;
    let config = state.stun_config.read().await;
    let multi_result = stun::discover_public_addrs(&config)
        .await
        .map_err(|e| AppError::invalid(format!("STUN discovery failed for connectivity check: {e}")))?;

    let nat_type = stun::classify_nat(&multi_result);
    let host_addrs: Vec<String> = crate::local_addr::gather_host_candidates()
        .iter()
        .map(|a| a.to_string())
        .collect();

    // Determine reachability based on NAT type and STUN consensus.
    let (reachable, behind_symmetric) = match nat_type {
        stun::NatType::Symmetric => {
            // Symmetric NAT: STUN works for outbound, but inbound won't work
            // without TURN. We still report the public IP but warn the user.
            (true, true)
        }
        stun::NatType::Blocked => (false, false),
        stun::NatType::None => (true, false),
        _ => {
            // Cone NAT types: inbound should work if the port mapping is stable.
            // We can't fully verify without an external echo service, but we
            // report optimistic reachability with a note.
            (multi_result.consensus, false)
        }
    };

    let status = stun::ConnectivityStatus {
        reachable,
        nat_type,
        public_addr: multi_result.consensus_addr.map(|a| a.to_string()),
        host_addrs,
        behind_symmetric_nat: behind_symmetric,
    };

    // Update state
    {
        let mut cv = state.connectivity_verified.write().await;
        *cv = reachable;
    }

    tracing::info!(reachable = reachable, nat = %nat_type, "connectivity check complete");
    Ok(status)
}

/// Get full network diagnostics for the frontend.
pub async fn get_network_diagnostics(
    state: State<'_, Arc<AppState>>,
) -> Result<candidate::NetworkDiagnostics, AppError> {
    collect_network_diagnostics(&state, tor::is_enabled()).await
}

async fn collect_network_diagnostics(
    state: &AppState,
    tor_enabled: bool,
) -> Result<candidate::NetworkDiagnostics, AppError> {
    state.ensure_not_air_gapped().await?;
    if tor_enabled {
        return Err(AppError::blocked("Tor routing is enabled — direct STUN diagnostics are blocked"));
    }

    let nat_type = *state.nat_type.read().await;
    // Snapshotted, not held. Taking `candidates.read()` and then blocking on
    // `stun_config.read()` inverts the order `refresh_stun` uses, and both are
    // write-preferring — one queued writer on either side is enough to wedge
    // both permanently.
    let candidates: Vec<_> = state.candidates.read().await.clone();
    let config = { state.stun_config.read().await.clone() };

    let stun_servers = stun::check_all_servers(&config).await;

    let host_addrs: Vec<String> = crate::local_addr::gather_host_candidates()
        .iter()
        .map(|a| a.to_string())
        .collect();

    let public_addr = state.public_ip.read().await.map(|a| a.to_string());
    let connectivity = stun::ConnectivityStatus {
        reachable: *state.connectivity_verified.read().await,
        nat_type,
        public_addr,
        host_addrs,
        behind_symmetric_nat: nat_type == stun::NatType::Symmetric,
    };

    Ok(candidate::NetworkDiagnostics {
        candidates: candidates.clone(),
        nat_type,
        stun_servers,
        connectivity,
    })
}

/// Get current network settings for the frontend.
pub async fn get_network_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<tor::NetworkSettings, AppError> {
    let tor_reachable = tor::check_proxy_reachable().await;
    let public_ip = state.public_ip.read().await;

    Ok(tor::NetworkSettings {
        tor_enabled: tor::is_enabled(),
        tor_proxy_addr: tor::TOR_PROXY_ADDR.to_string(),
        tor_reachable,
        public_ip: public_ip.map(|a| a.to_string()),
    })
}

/// Enable or disable Tor routing.
pub async fn set_tor_enabled(state: State<'_, Arc<AppState>>, enabled: bool) -> Result<(), AppError> {
    // Air-gap mode: Tor traffic is internet-facing by definition.
    if enabled {
        state.ensure_not_air_gapped().await?;
    }
    tor::set_enabled(enabled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn diagnostics_reject_air_gap_before_accessing_stun_config() {
        let state = AppState::new(String::new());
        state.security_config.write().await.air_gap_mode = true;
        let _config_guard = state.stun_config.write().await;

        for tor_enabled in [false, true] {
            let result = tokio::time::timeout(
                Duration::from_secs(1),
                collect_network_diagnostics(&state, tor_enabled),
            )
            .await
            .expect("diagnostics must reject before accessing STUN configuration");

            assert_eq!(
                result.unwrap_err(),
                "air-gap mode is enabled — this internet-facing operation is blocked"
            );
        }
    }

    #[tokio::test]
    async fn diagnostics_reject_tor_before_accessing_stun_config() {
        let state = AppState::new(String::new());
        let _config_guard = state.stun_config.write().await;

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            collect_network_diagnostics(&state, true),
        )
        .await
        .expect("diagnostics must reject before accessing STUN configuration");

        assert_eq!(
            result.unwrap_err(),
            "Tor routing is enabled — direct STUN diagnostics are blocked"
        );
    }

    #[tokio::test]
    async fn diagnostics_allow_unrestricted_mode() {
        let state = AppState::new(String::new());
        state.stun_config.write().await.servers.clear();

        let diagnostics = collect_network_diagnostics(&state, false).await.unwrap();

        assert!(diagnostics.stun_servers.is_empty());
        assert!(diagnostics.candidates.is_empty());
        assert!(diagnostics.connectivity.public_addr.is_none());
    }
}
