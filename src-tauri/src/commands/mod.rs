//! M2M — Tauri Commands
//!
//! IPC bridge between the React UI and the Rust backend.
//! Each command validates inputs and returns safe, typed responses.
//! No secrets are exposed to the frontend.

use crate::error::AppError;
pub mod chat;
pub mod discovery;
pub mod files;
pub mod forwards;
pub mod groups;
pub mod network;
pub mod relay;
pub mod security;
pub mod settings;
pub mod util;
pub mod vault;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::state::PeerConnection;
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
    /// Sender of this message (used for group messages — Ed25519 hex).
    /// Empty string for 1:1 messages (implicit from conversation).
    #[serde(default)]
    pub sender_peer_key_hex: String,
}

impl ChatMessage {
    pub fn new(id: String, content: String, direction: String, timestamp: u64) -> Self {
        Self {
            id,
            content,
            direction,
            timestamp,
            read_at: None,
            edited_at: None,
            deleted: false,
            expires_at: None,
            reactions: std::collections::HashMap::new(),
            sender_peer_key_hex: String::new(),
        }
    }

    /// Builder-style setters so construction sites only specify the fields
    /// that differ from the defaults (single source of truth for the
    /// 11-field shape — adding a field requires touching `new` only).
    pub fn with_read_at(mut self, v: Option<i64>) -> Self {
        self.read_at = v;
        self
    }

    pub fn with_edited_at(mut self, v: Option<i64>) -> Self {
        self.edited_at = v;
        self
    }

    pub fn with_deleted(mut self, v: bool) -> Self {
        self.deleted = v;
        self
    }

    pub fn with_expires_at(mut self, v: Option<i64>) -> Self {
        self.expires_at = v;
        self
    }

    pub fn with_reactions(mut self, v: std::collections::HashMap<String, Vec<String>>) -> Self {
        self.reactions = v;
        self
    }

    pub fn with_sender(mut self, v: String) -> Self {
        self.sender_peer_key_hex = v;
        self
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
    /// Whether this conversation is favorited.
    #[serde(default)]
    pub is_favorite: bool,
    /// Whether this conversation is archived.
    #[serde(default)]
    pub archived: bool,
    /// Number of unread received messages.
    #[serde(default)]
    pub unread_count: u32,
}

pub use crate::storage::FamilyMember;

// ─── Group Chat Types (Phase 3) ───

/// Summary info for a group, sent to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct GroupInfo {
    pub group_id: String,
    pub group_name: String,
    pub member_count: u32,
    pub created_at: u64,
}

/// Full group detail with members, sent to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct GroupDetail {
    pub group_id: String,
    pub group_name: String,
    pub member_count: u32,
    pub created_at: u64,
    pub our_role: String,
    pub members: Vec<crate::group::GroupMember>,
}

/// Event emitted to the frontend when a group message arrives.
#[derive(Debug, Clone, Serialize)]
pub struct GroupMessageEvent {
    pub group_id: String,
    pub message: ChatMessage,
}

/// Event emitted to the frontend for group state changes.
#[derive(Debug, Clone, Serialize)]
pub struct GroupEvent {
    pub group_id: String,
    pub event_type: String,
    pub peer_key_hex: Option<String>,
}

/// Status of a pending reconnection attempt.
#[derive(Debug, Clone, Serialize)]
pub struct ReconnectAttemptEvent {
    pub peer_key_hex: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_secs: u64,
    pub state: String, // "attempting", "success", "failed"
}

/// Per-attempt TCP connect deadline for [`attempt_reconnect`].
///
/// A black-holed address must not block the reconnect command indefinitely;
/// without this the backoff loop could never reach its next attempt.
const RECONNECT_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Attempt to reconnect to a peer whose connection dropped.
/// Uses exponential backoff (1s, 2s, 4s, ..., 30s cap, max 5 attempts).
/// The user must explicitly call this — no auto-reconnect.
///
/// On TCP success we perform a REAL authenticated handshake as initiator
/// (classic Ed25519-signed ephemeral exchange — no prekey bundle needed,
/// which we deliberately don't have without re-sharing an invite). The
/// "established" state is only emitted once that handshake succeeds, so
/// the UI never reports an encrypted session that was never cryptographically
/// set up (M4).
#[tauri::command]
pub async fn attempt_reconnect(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
    peer_key_hex: String,
) -> Result<crate::commands::ConnectionInfo, AppError> {
    let info = {
        let mut pr = state.pending_reconnects.write().await;
        pr.remove(&peer_key_hex)
            .ok_or("no pending reconnect info for this peer")?
    };

    // Attempt with exponential backoff
    for attempt in 0..crate::reconnect::MAX_RECONNECT_ATTEMPTS {
        let delay = crate::reconnect::compute_backoff(attempt);

        let _ = app_handle.emit(
            "m2m://reconnect-attempt",
            crate::commands::ReconnectAttemptEvent {
                peer_key_hex: peer_key_hex.clone(),
                attempt: attempt + 1,
                max_attempts: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
                delay_secs: delay.as_secs(),
                state: "attempting".to_string(),
            },
        );

        // Try direct TCP connection to the last-known address.
        // Routed through the Tor-aware chokepoint (so a reconnect under Tor
        // does not expose the real IP) and given a bounded deadline, since
        // this used to await an unbounded connect and could hang the command
        // forever against a black-holed address.
        let hint: std::net::SocketAddr = info
            .peer_address_hint
            .parse()
            .map_err(|e| AppError::invalid(format!("invalid peer address hint: {e}")))?;

        match crate::dial::dial_with_timeout(hint, RECONNECT_CONNECT_TIMEOUT).await {
            Ok(mut stream) => {
                // ── Refused, not silently downgraded ──
                //
                // This used to perform `handshake_as_initiator`, the pre-X3DH
                // handshake, on every reconnect. `reconnect.rs`'s own module doc
                // promises "a fresh X3DH handshake" and CLAUDE.md states there is
                // no downgrade path, but the non-X3DH path has no one-time prekey
                // and therefore no forward secrecy — so a reconnect quietly
                // produced a session with weaker guarantees than the one it
                // replaced, and the only trace was a `tracing::warn!` nobody sees.
                //
                // Doing X3DH properly needs a *fresh* prekey bundle: the one in the
                // original invite carries a one-time prekey, which is single-use by
                // construction, so replaying it is not a fix. The protocol has no
                // prekey-refresh packet, so there is no way to obtain one today,
                // which makes refusing the honest option.
                //
                // The transport is still dialled, so reachability is proven and the
                // peer is not reported as down — we simply do not establish a
                // session we cannot establish safely.
                tracing::warn!(
                    peer = %peer_key_hex,
                    reason = "no fresh prekey bundle available for an X3DH reconnect",
                    "reconnect refused — peer reachable but handshake not attempted"
                );
                drop(stream);
                let _ = app_handle.emit(
                    "m2m://reconnect-attempt",
                    crate::commands::ReconnectAttemptEvent {
                        peer_key_hex: peer_key_hex.clone(),
                        attempt: attempt + 1,
                        max_attempts: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
                        delay_secs: 0,
                        state: "needs_new_invite".to_string(),
                    },
                );
                // Drop the metadata rather than leaving a prompt the user can keep
                // clicking into the same refusal.
                pr.remove(&peer_key_hex);
                // `Err`, not a synthesised `ConnectionInfo`: this returns
                // `Result<ConnectionInfo, AppError>` and the caller feeds the
                // success value straight into connection state. Returning a
                // fabricated "established" would be exactly the kind of claim the
                // UI cannot back.
                return Err(AppError::not_connected(
                    "This peer is reachable, but reconnecting would require the \
                     pre-5.0.0 handshake, which is refused because it has no forward \
                     secrecy. Exchange a fresh invite to reconnect.",
                ));
            }
            Err(_) => {
                // Wait before next attempt
                tokio::time::sleep(delay).await;
            }
        }
    }

    let _ = app_handle.emit(
        "m2m://reconnect-attempt",
        crate::commands::ReconnectAttemptEvent {
            peer_key_hex: peer_key_hex.clone(),
            attempt: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
            max_attempts: crate::reconnect::MAX_RECONNECT_ATTEMPTS,
            delay_secs: 0,
            state: "failed".to_string(),
        },
    );

    Err(AppError::not_connected(
        "reconnection failed after max attempts — the peer may be offline or the network changed",
    ))
}

/// List all peers with pending reconnection info.
#[tauri::command]
pub async fn list_pending_reconnects(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> Result<Vec<String>, AppError> {
    let pr = state.pending_reconnects.read().await;
    Ok(pr.keys().cloned().collect())
}
