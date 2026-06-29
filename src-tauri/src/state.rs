/// M2M — Application State
///
/// Central application state shared across Tauri commands.
/// Manages the identity, active sessions, storage handles,
/// and network configuration (STUN, candidates, Tor).
use std::collections::{HashMap, HashSet, VecDeque};
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
    #[expect(dead_code, reason = "Reserved for diagnostic display")]
    pub remote_addr: SocketAddr,
    /// The Happy Eyeballs connection strategy that won this connection
    /// (e.g. "host", "ipv6", "port-mapped", "srflx", "prflx", "relay").
    /// Used for adaptive chunk size computation.
    pub strategy_name: String,
}

/// Transfer state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransferState {
    Pending,         // Waiting for accept (sender) / awaiting chunks (receiver)
    Transferring,    // Chunks actively flowing
    Paused,          // User-initiated pause
    Completed,       // File fully transferred and verified
    Failed,          // Irrecoverable error (disconnect, hash mismatch)
    Cancelled,       // User-initiated cancel or peer cancelled
}

impl std::fmt::Display for TransferState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferState::Pending => write!(f, "pending"),
            TransferState::Transferring => write!(f, "transferring"),
            TransferState::Paused => write!(f, "paused"),
            TransferState::Completed => write!(f, "completed"),
            TransferState::Failed => write!(f, "failed"),
            TransferState::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// State for an in-progress file transfer (sending side).
///
/// The sender reads the file in a streaming fashion (never full-file in RAM),
/// tracks which chunks have been ACKed by the receiver, and supports retry
/// on timeouts. Pre-computed chunk hashes are stored for verification
/// before each send.
pub struct OutgoingFileTransfer {
    pub transfer_id: String,
    pub peer_key_hex: String,
    pub file_path: PathBuf,
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub file_hash: [u8; 32],
    /// Per-chunk SHA-256 hashes, pre-computed in a single streaming pass.
    pub chunk_hashes: Vec<[u8; 32]>,
    /// Chunk size in bytes used when computing hashes and sending.
    /// Adapted to the connection strategy (512 KiB for LAN, 256 KiB default, 128 KiB relay).
    pub chunk_size: usize,
    /// Version of file transfer protocol the peer supports (0x01 = legacy, 0x02 = ACKs).
    pub peer_protocol_version: u8,
    pub state: TransferState,
    /// Chunks dispatched (may not be acked yet).
    pub chunks_sent: u32,
    /// Chunks confirmed by receiver via ACK packets.
    pub chunks_acked: u32,
    /// Index of last chunk acked — used for resume on reconnect.
    pub last_acked_index: u32,
    /// Created timestamp (unix seconds).
    pub created_at: u64,
    /// Last activity timestamp (unix seconds).
    pub last_activity_at: u64,
}

impl OutgoingFileTransfer {
    /// Check if all chunks have been sent and acked.
    pub fn is_complete(&self) -> bool {
        self.chunks_acked >= self.total_chunks
    }

    /// Fraction [0.0, 1.0] of chunks completed.
    pub fn progress_fraction(&self) -> f64 {
        if self.total_chunks == 0 { return 1.0; }
        self.chunks_acked as f64 / self.total_chunks as f64
    }
}

/// State for an in-progress file transfer (receiving side).
///
/// Chunks are written directly to a temporary file on disk as they arrive,
/// NOT buffered in RAM. Only a sparse bitmask is kept in memory to track
/// which chunks have been received. This prevents OOM attacks from peers
/// claiming large files (e.g. 4GB).
pub struct IncomingFileTransfer {
    pub transfer_id: String,
    pub peer_key_hex: String,
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub file_hash: Vec<u8>,
    /// Per-chunk SHA-256 hashes from v2 request (empty if v1 sender).
    pub chunk_hashes: Vec<Vec<u8>>,
    /// File transfer protocol version used by the sender (0x01 = legacy, 0x02 = v2).
    pub peer_protocol_version: u8,
    pub save_path: PathBuf,
    /// Temporary file on disk — chunks are written here as they arrive.
    pub temp_file: Option<std::fs::File>,
    /// Path to the temporary file (for cleanup on failure).
    pub temp_path: Option<PathBuf>,
    /// Number of chunks received so far.
    pub chunks_received: u32,
    /// Total bytes received so far.
    pub bytes_received: u64,
    /// Bitmask of received chunks: true = chunk received.
    /// Size = total_chunks, initialized to all false.
    pub chunks_bitmask: Vec<bool>,
    pub state: TransferState,
    /// Created timestamp (unix seconds).
    pub created_at: u64,
    /// Error description if state = Failed.
    pub error: Option<String>,
}

impl IncomingFileTransfer {
    pub fn progress_fraction(&self) -> f64 {
        if self.total_chunks == 0 { return 1.0; }
        self.chunks_received as f64 / self.total_chunks as f64
    }

    /// Check if all chunks have been received (bitmask fully true).
    pub fn all_chunks_received(&self) -> bool {
        if self.chunks_bitmask.is_empty() {
            return self.chunks_received >= self.total_chunks;
        }
        self.chunks_bitmask.iter().all(|&b| b)
    }

    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }
}

/// Transfer queue with concurrency limits.
///
/// - Max concurrent: 3 (configurable)
/// - Max queue depth: 100
/// - Transfers beyond the queue are rejected immediately.
pub struct TransferQueue {
    /// Ordered transfer IDs waiting to start.
    pub queue: VecDeque<String>,
    /// Transfer IDs currently being transferred.
    pub active: HashSet<String>,
    /// Max concurrent active transfers.
    pub max_concurrent: u32,
}

impl TransferQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            active: HashSet::new(),
            max_concurrent: 3,
        }
    }

    /// Check if we can start a new transfer.
    pub fn can_start(&self) -> bool {
        self.active.len() < self.max_concurrent as usize
    }

    /// Enqueue a transfer ID. Returns Err if queue is full.
    pub fn enqueue(&mut self, transfer_id: String) -> Result<(), &'static str> {
        const MAX_QUEUE_DEPTH: usize = 100;
        if self.queue.len() + self.active.len() >= MAX_QUEUE_DEPTH {
            return Err("transfer queue is full (max 100 pending)");
        }
        self.queue.push_back(transfer_id);
        Ok(())
    }

    /// Try to start the next queued transfer. Returns the transfer_id if one was started.
    pub fn dequeue(&mut self) -> Option<String> {
        if !self.can_start() { return None; }
        let id = self.queue.pop_front()?;
        self.active.insert(id.clone());
        Some(id)
    }

    /// Mark a transfer as done (completed/failed/cancelled). Returns the next queued transfer.
    pub fn finish(&mut self, transfer_id: &str) -> Option<String> {
        self.active.remove(transfer_id);
        self.dequeue()
    }
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
    #[expect(dead_code, reason = "Reserved for diagnostics/settings display")]
    pub data_dir: String,
    /// Pending outgoing file transfers. Key: transfer_id, Value: transfer state.
    pub outgoing_transfers: RwLock<HashMap<String, OutgoingFileTransfer>>,
    /// Active incoming file transfers. Key: transfer_id
    pub incoming_transfers: RwLock<HashMap<String, IncomingFileTransfer>>,
    /// Ordered transfer queue (outgoing).
    pub transfer_queue: RwLock<TransferQueue>,
    /// Message store (initialised when identity is loaded).
    pub message_store: Mutex<Option<storage::MessageStore>>,
    /// Key store (initialised when identity is loaded).
    pub key_store: Mutex<Option<storage::KeyStore>>,
    /// Transfer history store (initialised when identity is loaded).
    pub transfer_store: Mutex<Option<storage::TransferStore>>,
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
            transfer_queue: RwLock::new(TransferQueue::new()),
            message_store: Mutex::new(None),
            key_store: Mutex::new(None),
            transfer_store: Mutex::new(None),
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
