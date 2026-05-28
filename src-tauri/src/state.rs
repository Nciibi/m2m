/// M2M — Application State
///
/// Central application state shared across Tauri commands.
/// Manages the identity, active sessions, and storage handles.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::crypto::IdentityKeypair;
use crate::network::ConnectionState;
use crate::session::Session;
use crate::storage::{KeyStore, MessageStore};

/// Peer connection handle, holding the TCP stream and session state.
pub struct PeerConnection {
    pub stream: TcpStream,
    pub session: Session,
    pub remote_addr: SocketAddr,
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
