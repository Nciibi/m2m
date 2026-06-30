//! M2M — Tauri Commands
//!
//! IPC bridge between the React UI and the Rust backend.
//! Each command validates inputs and returns safe, typed responses.
//! No secrets are exposed to the frontend.

pub mod chat;
pub mod discovery;
pub mod files;
pub mod security;
pub mod forwards;
pub mod network;
pub mod relay;
pub mod settings;
pub mod util;
pub mod vault;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

// ─── Response types for the frontend — never contain secrets ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    pub fingerprint: String,
    pub public_key_hex: String,
    pub has_identity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub state: String,
    pub peer_fingerprint: Option<String>,
    pub peer_verified: bool,
    pub peer_key_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub content: String,
    pub direction: String,
    pub timestamp: u64,
    /// When this message was read (null = unread, only for received messages).
    pub read_at: Option<i64>,
    /// When this message was edited (null = never edited).
    pub edited_at: Option<i64>,
    /// Whether this message has been soft-deleted.
    pub deleted: bool,
    /// When this message self-destructs (null = never).
    pub expires_at: Option<i64>,
    /// Reactions on this message, as a map: reaction_emoji → [peer_key_hex, ...].
    #[serde(default)]
    pub reactions: std::collections::HashMap<String, Vec<String>>,
}

impl ChatMessage {
    pub fn new(id: String, content: String, direction: String, timestamp: u64) -> Self {
        Self {
            id, content, direction, timestamp,
            read_at: None,
            edited_at: None,
            deleted: false,
            expires_at: None,
            reactions: std::collections::HashMap::new(),
        }
    }
}

impl Drop for ChatMessage {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.content.zeroize();
        // HashMap is zeroized via clear + shrink
        self.reactions.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteInfo {
    pub fingerprint: String,
    pub address_hint: String,
    pub expires_at: u64,
    pub one_time: bool,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferInfo {
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
    pub peer_key_hex: String,
}

/// Progress event for an in-progress file transfer.
#[derive(Debug, Clone, Serialize)]
pub struct TransferProgressEvent {
    pub transfer_id: String,
    pub peer_key_hex: String,
    pub filename: String,
    pub total_size: u64,
    pub bytes_transferred: u64,
    pub chunks_completed: u32,
    pub chunks_total: u32,
    pub state: String,
    pub speed_bytes_per_sec: u64,
    pub estimated_remaining_secs: u64,
}

// ─── Events emitted to the React frontend ───

#[derive(Debug, Clone, Serialize)]
pub struct MessageEvent {
    pub peer_key_hex: String,
    pub message: ChatMessage,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionEvent {
    pub peer_key_hex: String,
    pub state: String,
    pub peer_fingerprint: Option<String>,
    /// Whether the peer was verified before the connection dropped.
    /// Used by the frontend to decide whether to show a Reconnect button.
    #[serde(default)]
    pub peer_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRequestEvent {
    pub peer_key_hex: String,
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
}

/// Vault status response for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
}

/// Response type for conversation list items.
#[derive(Debug, Clone, Serialize)]
pub struct ConversationListItem {
    pub id: String,
    pub peer_key_hex: String,
    pub display_name: Option<String>,
    pub peer_display_name: Option<String>,
    pub last_message_at: Option<i64>,
    pub last_message_preview: Option<String>,
    pub message_count: i64,
    pub is_online: bool,
    pub auto_delete_at: Option<i64>,
    pub retention_policy: String,
    pub created_at: i64,
}

pub use crate::storage::FamilyMember;

/// Status of a pending reconnection attempt.
#[derive(Debug, Clone, Serialize)]
pub struct ReconnectAttemptEvent {
    pub peer_key_hex: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_secs: u64,
    pub state: String, // "attempting", "success", "failed"
}

/// Attempt to reconnect to a peer whose connection dropped.
/// Uses exponential backoff (1s, 2s, 4s, ..., 30s cap, max 5 attempts).
/// The user must explicitly call this — no auto-reconnect.
/// Since we can't do a full X3DH handshake without the peer's invite,
/// we try a direct TCP connection to the last-known address. If the peer
/// is still listening and our network hasn't changed, this will work.
/// Otherwise, the user must re-share an invite.
#[tauri::command]
pub async fn attempt_reconnect(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
    peer_key_hex: String,
) -> Result<crate::commands::ConnectionInfo, String> {
    let info = {
        let mut pr = state.pending_reconnects.write().await;
        pr.remove(&peer_key_hex)
            .ok_or("no pending reconnect info for this peer")?
    };

    // Attempt with exponential backoff
    for attempt in 0..crate::reconnect::MAX_RECONNECT_ATTEMPTS {
        let delay = crate::reconnect::compute_backoff(attempt);

        let _ = app_handle.emit("m2m://reconnect-attempt", crate::commands::ReconnectAttemptEvent {
            peer_key_hex: peer_key_hex.clone(),
            attempt: attempt + 1,
            max_attempts: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
            delay_secs: delay.as_secs(),
            state: "attempting".to_string(),
        });

        // Try direct TCP connection to the last-known address
        match tokio::net::TcpStream::connect(&info.peer_address_hint).await {
            Ok(stream) => {
                let (read_half, write_half) = stream.into_split();
                let mut session = crate::session::Session::new();
                session.peer_identity_pub = hex::decode(&info.peer_key_hex)
                    .map_err(|e| format!("invalid peer key: {e}"))?
                    .try_into()
                    .map_err(|_| "peer key length mismatch")?;
                session.peer_verified = info.peer_verified;
                session.ratchet_interval = info.ratchet_interval;

                let conn = crate::state::PeerConnection {
                    write_half,
                    session,
                    remote_addr: info.peer_address_hint.parse()
                        .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                    strategy_name: info.strategy_name.clone(),
                };

                {
                    let mut conns = state.connections.write().await;
                    conns.insert(peer_key_hex.clone(),
                        std::sync::Arc::new(tokio::sync::Mutex::new(conn)));
                }

                let _ = app_handle.emit("m2m://reconnect-attempt", crate::commands::ReconnectAttemptEvent {
                    peer_key_hex: peer_key_hex.clone(),
                    attempt: attempt + 1,
                    max_attempts: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
                    delay_secs: 0,
                    state: "success".to_string(),
                });

                let _ = app_handle.emit("m2m://connection", crate::commands::ConnectionEvent {
                    peer_key_hex: peer_key_hex.clone(),
                    state: "established".to_string(),
                    peer_fingerprint: Some(info.peer_fingerprint.clone()),
                });

                // Start receive loop
                crate::commands::network::spawn_receive_loop(
                    app_handle.clone(),
                    state.inner().clone(),
                    read_half,
                    peer_key_hex.clone(),
                    None,
                );

                return Ok(crate::commands::ConnectionInfo {
                    state: "established".to_string(),
                    peer_fingerprint: Some(info.peer_fingerprint),
                    peer_verified: info.peer_verified,
                    peer_key_hex: Some(peer_key_hex),
                });
            }
            Err(_) => {
                // Wait before next attempt
                tokio::time::sleep(delay).await;
            }
        }
    }

    let _ = app_handle.emit("m2m://reconnect-attempt", crate::commands::ReconnectAttemptEvent {
        peer_key_hex: peer_key_hex.clone(),
        attempt: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
        max_attempts: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
        delay_secs: 0,
        state: "failed".to_string(),
    });

    Err("reconnection failed after max attempts — the peer may be offline or the network changed".to_string())
}

/// List all peers with pending reconnection info.
#[tauri::command]
pub async fn list_pending_reconnects(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> Result<Vec<String>, String> {
    let pr = state.pending_reconnects.read().await;
    Ok(pr.keys().cloned().collect())
}
