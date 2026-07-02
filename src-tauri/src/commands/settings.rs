//! Network settings and diagnostics commands.
//!
//! Handles STUN discovery, Tor proxy configuration, private mode,
//! connectivity checks, and full network diagnostics for the frontend.

use std::sync::Arc;

use tauri::State;

use crate::candidate;
use crate::state::AppState;
use crate::stun;
use crate::tor;

/// Get the user's theme preference.
#[tauri::command]
pub async fn get_theme_preference(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let theme = state.theme_preference.read().await;
    Ok(theme.clone())
}

/// Set the user's theme preference.
#[tauri::command]
pub async fn set_theme_preference(
    state: State<'_, Arc<AppState>>,
    theme: String,
) -> Result<(), String> {
    let valid = ["light", "dark", "system"];
    if !valid.contains(&theme.as_str()) {
        return Err("Invalid theme value".to_string());
    }
    let mut tp = state.theme_preference.write().await;
    *tp = theme.clone();
    Ok(())
}

/// Discover the public IP address using enhanced STUN (parallel queries + consensus).
#[tauri::command]
pub async fn discover_public_ip(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let result = state.refresh_stun()
        .await
        .map_err(|e| format!("STUN discovery failed: {e}"))?;

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
#[tauri::command]
pub async fn get_stun_config(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::StunConfig, String> {
    let config = state.stun_config.read().await;
    Ok(config.clone())
}

/// Update the STUN server list and configuration.
#[tauri::command]
pub async fn set_stun_servers(
    state: State<'_, Arc<AppState>>,
    servers: Vec<String>,
) -> Result<(), String> {
    if servers.is_empty() {
        return Err("STUN server list cannot be empty".to_string());
    }
    // Basic validation: each entry must contain a colon (host:port)
    for s in &servers {
        if !s.contains(':') {
            return Err(format!("invalid STUN server address (missing port): {s}"));
        }
        if s.len() > 255 {
            return Err(format!("STUN server address too long: {s}"));
        }
    }

    let mut config = state.stun_config.write().await;
    config.servers = servers;
    tracing::info!("STUN configuration updated");
    Ok(())
}

/// Toggle private mode (don't expose public IP in invites).
#[tauri::command]
pub async fn set_private_mode(
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    let mut pm = state.private_mode.write().await;
    *pm = enabled;
    let mut config = state.stun_config.write().await;
    config.private_mode = enabled;
    tracing::info!(private_mode = enabled, "privacy mode updated");
    Ok(())
}

/// Run connectivity verification: check if the listening port is reachable.
#[tauri::command]
pub async fn check_connectivity(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::ConnectivityStatus, String> {
    let config = state.stun_config.read().await;
    let multi_result = stun::discover_public_addrs(&config)
        .await
        .map_err(|e| format!("STUN discovery failed for connectivity check: {e}"))?;

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
#[tauri::command]
pub async fn get_network_diagnostics(
    state: State<'_, Arc<AppState>>,
) -> Result<candidate::NetworkDiagnostics, String> {
    let nat_type = *state.nat_type.read().await;
    let candidates = state.candidates.read().await;
    let config = state.stun_config.read().await;

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
#[tauri::command]
pub async fn get_network_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<tor::NetworkSettings, String> {
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
#[tauri::command]
pub async fn set_tor_enabled(
    enabled: bool,
) -> Result<(), String> {
    tor::set_enabled(enabled);
    Ok(())
}
