/// M2M — Reconnection Metadata
///
/// Stores metadata needed to reconnect to a peer after a connection drop.
/// The reconnection is **NOT automatic** — it waits for the user to confirm.
/// This preserves M2M's anti-tracking property: network changes result in
/// fresh sessions with new ephemeral keys.
///
/// ## Design
///
/// When a TCP connection drops, we save the peer metadata (strategy,
/// address, candidates) so the UI can show a "Reconnect?" prompt.
/// If the user agrees, we attempt a fresh X3DH handshake with exponential
/// backoff (1s, 2s, 4s, 8s, 16s, 30s cap). Pending messages saved while
/// offline are flushed after successful reconnection.
///
/// ## Privacy
///
/// Auto-reconnect would defeat M2M's anti-tracking mission — it would
/// prove to the network that the same identity is reconnecting. Instead,
/// we save the *capability* to reconnect and let the user *choose*.
use std::time::Duration;

/// Maximum number of reconnection attempts before giving up.
pub const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Initial backoff delay in seconds (doubles each attempt).
pub const INITIAL_BACKOFF_SECS: u64 = 1;

/// Maximum backoff delay in seconds.
pub const MAX_BACKOFF_SECS: u64 = 30;

/// Metadata needed to reconnect to a peer.
#[derive(Debug, Clone)]
pub struct ReconnectInfo {
    /// Peer's Ed25519 identity public key (already hex-encoded).
    pub peer_key_hex: String,
    /// Peer's fingerprint (for display in reconnecting badge).
    pub peer_fingerprint: String,
    /// The connection strategy that worked before.
    pub strategy_name: String,
    /// Peer's last-known address for reconnect.
    pub peer_address_hint: String,
    pub peer_verified: bool,
    pub ratchet_interval: u64,
}

/// Compute exponential backoff delay for a given attempt.
/// Attempt 0 → 1s, 1 → 2s, 2 → 4s, ..., capped at MAX_BACKOFF_SECS.
pub fn compute_backoff(attempt: u32) -> Duration {
    let multiplier = 2u64.saturating_pow(attempt);
    let secs = INITIAL_BACKOFF_SECS.saturating_mul(multiplier);
    Duration::from_secs(secs.min(MAX_BACKOFF_SECS))
}

#[cfg(test)]
mod reconnect_tests {
    use super::*;

    #[test]
    fn test_compute_backoff_exponential() {
        assert_eq!(compute_backoff(0), Duration::from_secs(1));
        assert_eq!(compute_backoff(1), Duration::from_secs(2));
        assert_eq!(compute_backoff(2), Duration::from_secs(4));
        assert_eq!(compute_backoff(3), Duration::from_secs(8));
        assert_eq!(compute_backoff(4), Duration::from_secs(16));
    }

    #[test]
    fn test_compute_backoff_capped() {
        assert_eq!(compute_backoff(5), Duration::from_secs(30));
        assert_eq!(compute_backoff(100), Duration::from_secs(30));
    }

    #[test]
    fn test_compute_backoff_zero_attempt() {
        assert_eq!(compute_backoff(0), Duration::from_secs(1));
    }
}
