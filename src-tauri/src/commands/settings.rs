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
#[tauri::command]
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
#[tauri::command]
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
#[tauri::command]
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
#[tauri::command]
pub async fn get_stun_config(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::StunConfig, AppError> {
    let config = state.stun_config.read().await;
    Ok(config.clone())
}

/// Upper bound on configured STUN servers. One task and one socket per entry.
const MAX_STUN_SERVERS: usize = 8;

/// Lower bound on configured STUN servers.
///
/// `stun::MIN_CONSENSUS_SERVERS` independent responders are required before
/// `discover_public_addrs` will report a consensus address at all, so a shorter
/// list cannot produce a usable public address. The previous bound was
/// `1..=8`: with one entry, "all responding servers agree" is vacuously true,
/// so a single rogue server — or a single DNS-hijacked name already in the
/// user's list — became the public address advertised in invites and in the
/// plaintext handshake frame a peer dials first.
const MIN_STUN_SERVERS: usize = stun::MIN_CONSENSUS_SERVERS;

/// Update the STUN server list and configuration.
#[tauri::command]
pub async fn set_stun_servers(
    state: State<'_, Arc<AppState>>,
    servers: Vec<String>,
) -> Result<(), AppError> {
    validate_stun_server_list(&servers)?;

    let mut config = state.stun_config.write().await;
    config.servers = servers;
    tracing::info!("STUN configuration updated");
    Ok(())
}

/// Validate a user-supplied STUN server list.
///
/// Split out of the command so the rules are testable without a Tauri
/// `State`, and so the command body is just "validate, then store".
fn validate_stun_server_list(servers: &[String]) -> Result<(), AppError> {
    // ── Quorum floor ──
    //
    // Enforced here as well as in `stun`, because this is the only way a user
    // reaches that requirement: the consensus rule is unreadable in the UI if a
    // one-server list silently disables it.
    if servers.len() < MIN_STUN_SERVERS {
        return Err(AppError::invalid(format!(
            "at least {MIN_STUN_SERVERS} STUN servers are required so a single server \
             cannot define your advertised public address (got {})",
            servers.len()
        )));
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
    // ── No duplicates ──
    //
    // Duplicates satisfy the quorum with one server: `["evil.example:3478",
    // "evil.example:3478"]` looks like two independent answers and is one, and
    // the response passes the transaction ID and FINGERPRINT checks because it
    // really is a STUN server answering a real query. Compared
    // case-insensitively because host names are case-insensitive.
    //
    // This cannot enforce *operator* diversity — `stun.l.google.com` and
    // `stun1.l.google.com` are one operator, and nothing at this layer knows
    // who owns a host. Documented rather than pretended.
    let mut unique = std::collections::HashSet::with_capacity(servers.len());
    for s in servers {
        if !unique.insert(s.trim().to_ascii_lowercase()) {
            return Err(AppError::invalid(format!(
                "duplicate STUN server — two copies of one server cannot agree \
                 independently: {s}"
            )));
        }
    }
    // Each entry must be a real `host:port` and must parse.
    //
    // The previous check was `s.contains(':')`, which accepts `"a:b"`, an
    // empty host, and a non-numeric port. Whatever passes here is DNS-resolved
    // and sent a UDP datagram from the user's real address, so this list is a
    // probe target: it must be validated, not merely colon-checked.
    for s in servers {
        if s.len() > 255 {
            return Err(AppError::invalid(format!(
                "STUN server address too long: {s}"
            )));
        }
        if s.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(AppError::invalid(format!(
                "invalid STUN server address: {s}"
            )));
        }
        let (host, port) = s
            .rsplit_once(':')
            .ok_or_else(|| format!("invalid STUN server address (missing port): {s}"))?;
        if host.is_empty() {
            return Err(AppError::invalid(format!(
                "invalid STUN server address (missing host): {s}"
            )));
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
            return Err(AppError::invalid(format!(
                "invalid STUN server port (0): {s}"
            )));
        }
    }
    Ok(())
}

/// Toggle private mode (don't expose public IP in invites).
#[tauri::command]
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

/// Run a connectivity check: cross-server STUN agreement and NAT classification.
///
/// Note what this does **not** do: it does not verify that the listening port is
/// reachable from the public internet, and does not claim to. `reachable` is
/// reported as `None` ("not measured") — see the comment inside for why, and
/// for what a real measurement would cost.
#[tauri::command]
pub async fn check_connectivity(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::ConnectivityStatus, AppError> {
    state.ensure_not_air_gapped().await?;
    let config = state.stun_config.read().await;
    let multi_result = stun::discover_public_addrs(&config).await.map_err(|e| {
        AppError::invalid(format!("STUN discovery failed for connectivity check: {e}"))
    })?;

    let nat_type = stun::classify_nat(&multi_result);
    let host_addrs: Vec<String> = crate::local_addr::gather_host_candidates()
        .iter()
        .map(|a| a.to_string())
        .collect();

    // ── What this check can actually establish ──
    //
    // The old code returned `reachable = true` for a symmetric NAT and
    // `multi_result.consensus` for every cone type, under a doc comment reading
    // "whether the listening port is reachable from the public internet", and
    // the Settings view renders the field verbatim. Neither arm tested
    // reachability: the address STUN reports is the UDP mapping of an ephemeral
    // socket this command just created, which is generally not the TCP
    // listening port at all. So the button reported a claim it could not back,
    // and a symmetric-NAT user — the one case where inbound connections really
    // do need TURN — was told "reachable: true".
    //
    // Verifying inbound reachability for real needs an external service that
    // dials our TCP port. That is a product decision (another party learns the
    // user's address on every "Check"), not something to smuggle in as an
    // optimistic default, so `reachable` is reported as `None` = not measured.
    //
    // What *is* measured is cross-server agreement, so that is what carries a
    // value. `consensus` already requires `stun::MIN_CONSENSUS_SERVERS`
    // responders; the second condition is restated so the field stays correct if
    // that rule is ever moved.
    let stun_agreement = Some(
        multi_result.consensus && multi_result.responding_servers >= stun::MIN_CONSENSUS_SERVERS,
    );
    let behind_symmetric = nat_type == stun::NatType::Symmetric;

    let status = stun::ConnectivityStatus {
        reachable: None,
        stun_agreement,
        nat_type,
        public_addr: multi_result.consensus_addr.map(|a| a.to_string()),
        host_addrs,
        behind_symmetric_nat: behind_symmetric,
    };

    // No measurement happened, so nothing is recorded as verified. Storing a
    // derived value here is what let the stale claim reappear later in
    // `collect_network_diagnostics`.
    {
        let mut cv = state.connectivity_verified.write().await;
        *cv = None;
    }

    tracing::info!(
        stun_agreement = ?stun_agreement,
        servers_responding = multi_result.responding_servers,
        servers_total = multi_result.total_servers,
        nat = %nat_type,
        "STUN agreement check complete (inbound TCP reachability not measured)"
    );
    Ok(status)
}

/// Get full network diagnostics for the frontend.
#[tauri::command]
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
        return Err(AppError::blocked(
            "Tor routing is enabled — direct STUN diagnostics are blocked",
        ));
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
        // `check_all_servers` probes per-server liveness (did the query get an
        // answer at all). It runs no agreement check and discards the reported
        // addresses, so reporting agreement here would be inventing a fact.
        // `None` means "not checked here", not "servers disagreed" — the
        // Settings view must say so rather than printing `false`.
        stun_agreement: None,
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
#[tauri::command]
pub async fn set_tor_enabled(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), AppError> {
    // Air-gap mode: Tor traffic is internet-facing by definition.
    if enabled {
        state.ensure_not_air_gapped().await?;
    }

    // Enabling Tor must also stop the two things Tor cannot protect against.
    //
    // `set_discovery_config` refuses to *start* LAN discovery while Tor is on,
    // and its own comment names the gap this closes: the two settings are
    // separate toggles, so a user can plausibly have LAN discovery enabled from
    // before and then switch Tor on. Nothing on the Tor path touched
    // `state.lan_cancel` / `state.dht_cancel`, so the announcer kept
    // broadcasting the real listening port and a rotating token to every host
    // on the LAN every 30s forever, and the DHT loop kept announcing the real
    // bind address plus the current ephemeral id to every seed — while the UI
    // showed Tor ON.
    if enabled {
        let lan_was_on = state.lan_cancel.read().await.is_some();
        let dht_was_on = state.dht_cancel.read().await.is_some();

        if lan_was_on {
            if let Some(c) = state.lan_cancel.read().await.as_ref() {
                c.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            *state.lan_state.write().await = None;
            *state.lan_cancel.write().await = None;
            tracing::info!("LAN discovery stopped — Tor enabled");
            // `source` is part of the payload contract that `events.ts::asSecurityError`
            // enforces, so the event has to carry it or the frontend validator
            // drops the whole payload and the user never learns their
            // protection changed.
            let _ = tauri::Emitter::emit(
                &app_handle,
                "m2m://security-error",
                serde_json::json!({
                    "source": "tor",
                    "message": "LAN discovery was turned off because Tor was enabled. \
                                It broadcasts your listening port to every host on this \
                                network, which Tor cannot protect against.",
                }),
            );
        }
        if dht_was_on {
            if let Some(c) = state.dht_cancel.read().await.as_ref() {
                c.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            *state.dht_state.write().await = None;
            *state.dht_cancel.write().await = None;
            tracing::info!("DHT discovery stopped — Tor enabled");
            // Reported like the LAN case. Without an event the Settings DHT
            // toggle keeps reading as enabled while the DHT loop is gone, which
            // is the same "the UI asserts a state the backend does not hold".
            let _ = tauri::Emitter::emit(
                &app_handle,
                "m2m://security-error",
                serde_json::json!({
                    "source": "tor",
                    "message": "Peer discovery (DHT) was turned off because Tor was enabled. \
                                It announces your listening address and a rotating identifier \
                                to other peers, which Tor cannot protect against.",
                }),
            );
        }
    }

    tor::set_enabled(enabled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A one-server list makes "all responding servers agree" vacuously true,
    /// which is how a single rogue server used to define the user's advertised
    /// public address. `stun::aggregate_consensus` refuses to publish below the
    /// quorum, so the list must not be configurable that way in the first place.
    #[test]
    fn stun_list_requires_a_quorum() {
        assert!(validate_stun_server_list(&[]).is_err());
        assert!(validate_stun_server_list(&["stun.example:3478".to_string()]).is_err());
        assert!(validate_stun_server_list(&[
            "stun.a.example:3478".to_string(),
            "stun.b.example:3478".to_string(),
        ])
        .is_ok());
    }

    /// Two copies of one server look like a quorum and are not one. Compared
    /// case-insensitively because host names are case-insensitive, so
    /// `Stun.A.example` and `stun.a.example` are the same entry.
    #[test]
    fn stun_list_rejects_duplicates() {
        let err = validate_stun_server_list(&[
            "stun.evil.example:3478".to_string(),
            "STUN.Evil.Example:3478".to_string(),
        ])
        .expect_err("a duplicated server must be rejected");
        assert!(err.message.contains("duplicate"), "got: {err:?}");
    }

    /// The pre-existing per-entry validation must still apply now that the list
    /// is checked by a helper — a malformed entry must not slip through the
    /// quorum branch.
    #[test]
    fn stun_list_still_validates_each_entry() {
        assert!(
            validate_stun_server_list(&["a:b".to_string(), "stun.b:3478".to_string()]).is_err()
        );
        assert!(
            validate_stun_server_list(&[":3478".to_string(), "stun.b:3478".to_string()]).is_err()
        );
        assert!(
            validate_stun_server_list(&["stun.a:0".to_string(), "stun.b:3478".to_string()])
                .is_err()
        );
        assert!(validate_stun_server_list(&[
            "stun.a:99999".to_string(),
            "stun.b:3478".to_string()
        ])
        .is_err());
    }

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
                    .into()
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
            "Tor routing is enabled — direct STUN diagnostics are blocked".into()
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
