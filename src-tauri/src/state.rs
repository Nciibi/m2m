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

use crate::crypto::IdentityKeypair;
use crate::network::ConnectionState;
use crate::session::Session;
use crate::storage;
use crate::stun;

/// Peer connection handle, holding the write half and session state.
/// The read half is consumed by the receive loop task.
pub struct PeerConnection {
    pub write_half: OwnedWriteHalf,
    pub session: Session,
    pub remote_addr: SocketAddr,
}

/// State for an in-progress file transfer (receiving side).
pub struct IncomingFileTransfer {
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub file_hash: Vec<u8>,
    pub received_chunks: HashMap<u32, Vec<u8>>,
    pub save_path: PathBuf,
}

/// Central application state.
pub struct AppState {
    /// The local identity keypair (loaded from encrypted storage).
    pub identity: RwLock<Option<IdentityKeypair>>,
    /// Active peer connections, keyed by peer public key hex.
    pub connections: RwLock<HashMap<String, Arc<Mutex<PeerConnection>>>>,
    /// TCP listener address (if listening).
    pub listen_addr: RwLock<Option<SocketAddr>>,
    /// Channel for incoming connection notifications.
    pub incoming_tx: Mutex<Option<tokio::sync::mpsc::Sender<(TcpStream, SocketAddr)>>>,
    /// Whether message history is enabled.
    pub history_enabled: RwLock<bool>,
    /// Data directory path.
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
    /// Held in memory only for the lifetime of the app.
    pub storage_key: RwLock<Option<[u8; 32]>>,
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
}

impl AppState {
    pub fn new(data_dir: String) -> Self {
        Self {
            identity: RwLock::new(None),
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

        let mut all_candidates = host_candidates;
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
