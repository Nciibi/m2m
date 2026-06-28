/// M2M — Application State
///
/// Central application state shared across Tauri commands.
/// Manages the identity, active sessions, storage handles,
/// and network configuration (STUN, candidates, Tor).
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use crate::network;
use crate::relay;
use crate::stun;

use crate::crypto::IdentityKeypair;
use crate::network::ConnectionState;
use crate::session::Session;
use crate::storage;

/// Peer connection handle, holding the write half and session state.
/// The read half is consumed by the receive loop task.
pub struct PeerConnection {
    pub write_half: OwnedWriteHalf,
    pub session: Session,
    /// Remote address (stored for diagnostics).
    #[allow(dead_code)]
    pub remote_addr: SocketAddr,
}

/// State for an in-progress file transfer (receiving side).
///
/// Chunks are written directly to a temporary file on disk as they arrive,
/// NOT buffered in RAM. Only a sparse bitmask is kept in memory to track
/// which chunks have been received. This prevents OOM attacks from peers
/// claiming large files (e.g. 4GB).
pub struct IncomingFileTransfer {
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub file_hash: Vec<u8>,
    pub save_path: PathBuf,
    /// Temporary file on disk — chunks are written here as they arrive.
    pub temp_file: Option<std::fs::File>,
    /// Path to the temporary file (for cleanup on failure).
    pub temp_path: Option<PathBuf>,
    /// Number of chunks received so far.
    pub chunks_received: u32,
    /// Bitmask of received chunks: true = chunk received.
    /// Size = total_chunks, initialized to all false.
    pub chunks_bitmask: Vec<bool>,
}

/// A port forwarding rule the user configured manually on their router.
///
/// Unlike UPnP/NAT-PMP/PCP (which M2M creates programmatically), a manual
/// forward is created by the user in their router's admin panel. M2M stores
/// it, includes it in invites as a reliable candidate, and never tries to
/// remove or renew it — the user manages its lifecycle themselves.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManualForward {
    /// The public IP:port that remote peers should connect to.
    /// This is what the router is forwarding to us.
    pub public_addr: String,
    /// The local TCP port our listener is bound to.
    pub listen_port: u16,
    /// Optional human label (e.g. "Home router", "Office pfSense").
    pub label: String,
    /// Arbitrary sort order (lower = higher priority).
    pub order: u32,
}

/// Central application state.
pub struct AppState {
    /// The local identity keypair (loaded from encrypted storage).
    pub identity: RwLock<Option<IdentityKeypair>>,
    /// X25519 identity keypair for X3DH key agreement.
    pub x25519_identity: RwLock<Option<crate::crypto::X25519IdentityKeypair>>,
    /// Active signed prekey for the current invite (consumed by X3DH handshake).
    pub active_signed_prekey: RwLock<Option<crate::crypto::EphemeralKeypair>>,
    /// Active peer connections, keyed by peer public key hex.
    pub connections: RwLock<HashMap<String, Arc<Mutex<PeerConnection>>>>,
    /// TCP listener address (if listening).
    pub listen_addr: RwLock<Option<SocketAddr>>,
    /// Channel for incoming connection notifications.
    pub incoming_tx: Mutex<Option<tokio::sync::mpsc::Sender<(TcpStream, SocketAddr)>>>,
    /// Whether message history is enabled.
    pub history_enabled: RwLock<bool>,
    /// Data directory path.
    #[allow(dead_code)]
    pub data_dir: String,
    /// Pending outgoing file transfers. Key: transfer_id, Value: filepath
    pub outgoing_transfers: RwLock<HashMap<String, String>>,
    /// Active incoming file transfers. Key: transfer_id
    pub incoming_transfers: RwLock<HashMap<String, IncomingFileTransfer>>,
    /// Message store (initialised when identity is loaded).
    pub message_store: Mutex<Option<storage::MessageStore>>,
    /// Key store (initialised when identity is loaded).
    pub key_store: Mutex<Option<storage::KeyStore>>,
    /// The storage encryption key (derived from passphrase or identity).
    /// Wrapped in StorageKey to ensure:
    /// - Locked in physical RAM (mlock/VirtualLock) — cannot be paged to swap
    /// - Zeroized on drop (automatic via Drop impl + StorageKey)
    pub storage_key: RwLock<Option<crate::secure_key::StorageKey>>,
    /// Whether the vault has been unlocked with a passphrase.
    pub vault_unlocked: RwLock<bool>,
    /// Whether a vault passphrase has been set (first-run detection).
    pub vault_initialized: RwLock<bool>,
    /// Disovered public IP address (via STUN).
    pub public_ip: RwLock<Option<SocketAddr>>,
    // ─── NAT Traversal & Network Diagnostics (NEW) ───
    /// STUN configuration (server list, timeouts, privacy mode).
    pub stun_config: RwLock<stun::StunConfig>,
    /// Cached candidate set (refreshed on STUN discovery).
    pub candidates: RwLock<Vec<crate::candidate::NetworkCandidate>>,
    /// Cached NAT type classification.
    pub nat_type: RwLock<stun::NatType>,
    /// Whether connectivity check has passed (port is reachable).
    pub connectivity_verified: RwLock<bool>,
    /// Whether we're in private mode (don't expose IP in invites).
    pub private_mode: RwLock<bool>,
    /// Connection rate limiter for DoS protection.
    pub connection_limiter: network::ConnectionLimiter,
    /// User-configured manual port forwards (stored in state, not persisted).
    /// The UI manages this list; each entry becomes a candidate in invites.
    pub manual_forwards: RwLock<Vec<ManualForward>>,
    /// Relay server configuration (optional).
    /// When set, relay candidates are included in invites as a fallback.
    pub relay_config: RwLock<Option<relay::RelayConfig>>,
    /// Current relay connection state (for frontend diagnostics).
    pub relay_state: RwLock<relay::RelayState>,
}

impl AppState {
    pub fn new(data_dir: String) -> Self {
        Self {
            identity: RwLock::new(None),
            x25519_identity: RwLock::new(None),
            active_signed_prekey: RwLock::new(None),
            connections: RwLock::new(HashMap::new()),
            listen_addr: RwLock::new(None),
            incoming_tx: Mutex::new(None),
            history_enabled: RwLock::new(true),
            data_dir,
            outgoing_transfers: RwLock::new(HashMap::new()),
            incoming_transfers: RwLock::new(HashMap::new()),
            message_store: Mutex::new(None),
            key_store: Mutex::new(None),
            storage_key: RwLock::new(None),
            vault_unlocked: RwLock::new(false),
            vault_initialized: RwLock::new(false),
            public_ip: RwLock::new(None),
            // NAT traversal defaults
            stun_config: RwLock::new(stun::StunConfig::default()),
            candidates: RwLock::new(Vec::new()),
            nat_type: RwLock::new(stun::NatType::Unknown),
            connectivity_verified: RwLock::new(false),
            private_mode: RwLock::new(false),
            connection_limiter: network::ConnectionLimiter::new(),
            manual_forwards: RwLock::new(Vec::new()),
            relay_config: RwLock::new(None),
            relay_state: RwLock::new(relay::RelayState::default()),
        }
    }

    /// Get the connection state for a peer by their public key hex.
    pub async fn connection_state(&self, peer_key_hex: &str) -> ConnectionState {
        let conns = self.connections.read().await;
        match conns.get(peer_key_hex) {
            Some(conn) => {
                let c = conn.lock().await;
                c.session.state
            }
            None => ConnectionState::Disconnected,
        }
    }

    /// Refresh STUN discovery and update stored candidates/NAT type.
    pub async fn refresh_stun(&self) -> Result<stun::StunMultiResult, stun::StunError> {
        let config = self.stun_config.read().await;
        let multi = stun::discover_public_addrs(&config).await?;

        // Update public IP
        if let Some(addr) = multi.consensus_addr {
            let mut pip = self.public_ip.write().await;
            *pip = Some(addr);
        }

        // Update NAT type
        let nat = stun::classify_nat(&multi);
        {
            let mut nt = self.nat_type.write().await;
            *nt = nat;
        }

        // Update candidates from STUN results
        let reflexive_candidates =
            crate::candidate::gather_reflexive_candidates(&multi);
        let host_candidates = crate::candidate::gather_host_candidates();
        let ipv6_candidates = crate::candidate::gather_ipv6_candidates();

        let mut all_candidates = host_candidates;
        all_candidates.extend(ipv6_candidates);
        all_candidates.extend(reflexive_candidates);
        all_candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        {
            let mut cand = self.candidates.write().await;
            *cand = all_candidates;
        }

        tracing::info!(nat = %nat, public_ip = ?multi.consensus_addr, "STUN refresh complete");
        Ok(multi)
    }
}
