/// M2M — Ephemeral Peer Identity
///
/// A short-lived identity that rotates on network changes.
/// This is NOT your permanent Ed25519 identity key — it's a temporary
/// pseudonym that changes when you switch WiFi, move to a new network,
/// or every 24 hours. This prevents linkability across network sessions.
///
/// ## Why not use the permanent Ed25519 key?
///
/// The permanent Ed25519 key is your **true identity** — it never changes.
/// If we publish it in a DHT or shout it over LAN, an observer can:
///   1. Track you across networks ("Alice was on CoffeeShop WiFi, now at Home WiFi")
///   2. Correlate your IP history ("Here are all the IPs Alice has used")
///   3. Build a movement profile ("Alice connects from home at 9am, office at 10am")
///
/// An ephemeral peer ID breaks all three. When the peer ID changes,
/// no observer can link the old one to the new one.
use std::time::{SystemTime, UNIX_EPOCH};

/// How often the ephemeral peer ID rotates (24 hours in seconds).
const EPHEMERAL_ID_ROTATION_SECS: u64 = 24 * 60 * 60;

/// An ephemeral peer identity for DHT/LAN discovery.
///
/// This is a random 32-byte value generated at startup and periodically
/// rotated. It is NOT linked to your permanent Ed25519 key.
#[derive(Debug, Clone)]
pub struct EphemeralPeerId {
    /// The ephemeral ID bytes (random, rotated periodically).
    pub id: [u8; 32],
    /// When this ID was created (unix seconds).
    pub created_at: u64,
}

impl EphemeralPeerId {
    /// Generate a new random ephemeral peer ID.
    pub fn generate() -> Self {
        let bytes = crate::crypto::random_bytes(32);
        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Self {
            id,
            created_at: now_unix_secs(),
        }
    }

    /// Check if this ID should be rotated (older than ROTATION_SECS).
    pub fn should_rotate(&self) -> bool {
        now_unix_secs().saturating_sub(self.created_at) >= EPHEMERAL_ID_ROTATION_SECS
    }

    /// Get the hex-encoded ephemeral ID.
    pub fn hex(&self) -> String {
        hex::encode(self.id)
    }
}

/// Network change detector.
///
/// Monitors for IP address changes and signals when the network
/// has changed (new WiFi, new IP, etc.). When detected, the DHT
/// and LAN discovery modules rotate their ephemeral IDs.
pub struct NetworkMonitor {
    /// The last known local IP address.
    last_local_ip: Option<std::net::IpAddr>,
    /// The last known public IP address (via STUN).
    last_public_ip: Option<std::net::SocketAddr>,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            last_local_ip: None,
            last_public_ip: None,
        }
    }

    /// Check if the network has changed since the last check.
    /// Returns true if the IP changed (caller should rotate ephemeral IDs).
    pub fn check_for_change(
        &mut self,
        current_local_ip: Option<std::net::IpAddr>,
        current_public_ip: Option<std::net::SocketAddr>,
    ) -> bool {
        let local_changed = match (self.last_local_ip, current_local_ip) {
            (Some(old), Some(new)) => old != new,
            (None, Some(_)) => true,  // First time seeing a local IP
            _ => false,
        };

        let public_changed = match (self.last_public_ip, current_public_ip) {
            (Some(old), Some(new)) => old != new,
            (None, Some(_)) => true,  // First time seeing a public IP
            _ => false,
        };

        self.last_local_ip = current_local_ip;
        self.last_public_ip = current_public_ip;

        local_changed || public_changed
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod ephemeral_id_tests {
    use super::*;

    #[test]
    fn test_generate_unique_ids() {
        let id1 = EphemeralPeerId::generate();
        let id2 = EphemeralPeerId::generate();
        assert_ne!(id1.id, id2.id, "two generated IDs should differ");
    }

    #[test]
    fn test_hex_format() {
        let id = EphemeralPeerId::generate();
        let hex = id.hex();
        assert_eq!(hex.len(), 64, "hex should be 64 chars for 32 bytes");
    }

    #[test]
    fn test_fresh_id_should_not_rotate() {
        let id = EphemeralPeerId::generate();
        assert!(!id.should_rotate(), "a brand new ID should not need rotation");
    }

    #[test]
    fn test_network_monitor_detects_change() {
        let mut monitor = NetworkMonitor::new();
        let ip_a: std::net::IpAddr = "192.168.1.5".parse().unwrap();
        let ip_b: std::net::IpAddr = "192.168.1.10".parse().unwrap();

        // First check — no previous IP, should detect change
        assert!(monitor.check_for_change(Some(ip_a), None));

        // Same IP — no change
        assert!(!monitor.check_for_change(Some(ip_a), None));

        // Different IP — change detected
        assert!(monitor.check_for_change(Some(ip_b), None));
    }

    #[test]
    fn test_network_monitor_detects_public_ip_change() {
        let mut monitor = NetworkMonitor::new();
        let local: std::net::IpAddr = "10.0.0.5".parse().unwrap();
        let pub_a: std::net::SocketAddr = "1.2.3.4:9876".parse().unwrap();
        let pub_b: std::net::SocketAddr = "5.6.7.8:9876".parse().unwrap();

        assert!(monitor.check_for_change(Some(local), Some(pub_a)));
        assert!(!monitor.check_for_change(Some(local), Some(pub_a)));
        assert!(monitor.check_for_change(Some(local), Some(pub_b)));
    }
}
