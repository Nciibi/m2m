/// M2M — Application State
///
/// Central application state shared across Tauri commands.
/// Manages the identity, active sessions, and storage handles.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::crypto::IdentityKeypair;
use crate::network::ConnectionState;
use crate::session::Session;
use crate::storage;

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
    pub incoming_tx: Mutex<Option<mpsc::Sender<(TcpStream, SocketAddr)>>>,
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
    /// Discovered public IP address (via STUN).
    pub public_ip: RwLock<Option<SocketAddr>>,
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
}
