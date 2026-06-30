//! M2M — Tauri Commands
//!
//! IPC bridge between the React UI and the Rust backend.
//! Each command validates inputs and returns safe, typed responses.
//! No secrets are exposed to the frontend.

pub mod chat;
pub mod discovery;
pub mod files;
pub mod forwards;
pub mod network;
pub mod relay;
pub mod settings;
pub mod util;
pub mod vault;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, zeroize::Zeroize)]
pub struct ChatMessage {
    pub id: String,
    pub content: String,
    pub direction: String,
    pub timestamp: u64,
}

impl Drop for ChatMessage {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.content.zeroize();
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
