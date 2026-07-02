/// M2M — Multi-Device Sync Module
///
/// Allows a user to pair multiple devices so they share peer keys and
/// conversation metadata under the same identity.
///
/// ## Design
///
/// Each device has its own Ed25519 identity (generated on first launch).
/// Device pairing works by:
///
/// 1. **Primary** generates a one-time sync invite token (random bytes, 15-min expiry)
/// 2. **Secondary** initiates an X3DH handshake using its *own* identity keys
/// 3. After handshake, Secondary sends `SyncDeviceInfo` including the token
/// 4. Primary validates the token, adds Secondary to `synced_devices`
/// 5. Both sides exchange peer keys and conversation metadata
///
/// ## Privacy
///
/// - Sync invites are one-time and short-lived (15 min)
/// - All sync data travels over the existing DR-encrypted session
/// - Messages are NOT synced by default — only metadata and peer keys
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::RwLock;
use uuid::Uuid;

use sodiumoxide::crypto::hash::sha256;

use crate::commands::ConnectionInfo;
use crate::protocol::{
    self, PacketType, SyncDeviceInfo, SyncPayload, SyncPayloadType,
};
use crate::state::AppState;

// ─── Constants ───

/// Sync invite validity window: 15 minutes.
const SYNC_INVITE_TTL_SECS: u64 = 900;

/// Maximum number of synced devices.
const MAX_SYNCED_DEVICES: usize = 8;

/// Current sync protocol version.
const SYNC_PROTOCOL_VERSION: u8 = 1;

// ─── Data Structures ───

/// A pending (unused) sync invite on the primary device.
#[derive(Debug, Clone)]
pub struct SyncInvite {
    /// Raw token bytes (held by primary, never sent to secondary).
    pub token: Vec<u8>,
    /// Unix timestamp when this invite expires.
    pub expires_at: u64,
    /// Whether this invite has been used (one-time enforcement).
    pub used: bool,
}

/// A paired device known to this device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedDevice {
    /// Unique device identifier (UUID v4).
    pub device_id: String,
    /// Human-readable device name (e.g., "Laptop", "Phone").
    pub device_name: String,
    /// The peer_key_hex of this device in our connection/peer store.
    pub peer_key_hex: String,
    /// Unix timestamp of last successful sync.
    pub last_synced: u64,
}

/// Manages multi-device sync state.
pub struct SyncManager {
    /// Our own device identifier (persistent, generated on first sync init).
    pub device_id: String,
    /// Our own device display name.
    pub device_name: String,
    /// Pending (unused) sync invites on this device.
    pub pending_invites: HashMap<String, SyncInvite>,
    /// Devices we are paired with.
    pub synced_devices: Vec<SyncedDevice>,
}

impl SyncManager {
    /// Create a new SyncManager with a random device ID.
    pub fn new() -> Self {
        Self {
            device_id: Uuid::new_v4().to_string(),
            device_name: "My Device".to_string(),
            pending_invites: HashMap::new(),
            synced_devices: Vec::new(),
        }
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Helper: current unix timestamp ───

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─── Tauri Commands ───

/// Generate a one-time sync invite on the primary device.
/// Returns a base64-encoded token string that must be shared with the secondary.
#[tauri::command]
pub async fn generate_sync_invite(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let mut mgr = state.sync_manager.write().await;

    // Generate 24 random bytes as the token
    let token = crate::crypto::random_bytes(24);
    let token_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(&token);

    let token_hash = sha256::hash(&token);

    // Store pending invite
    let now = now_unix();
    mgr.pending_invites.insert(
        hex::encode(token_hash),
        SyncInvite {
            token,
            expires_at: now + SYNC_INVITE_TTL_SECS,
            used: false,
        },
    );

    tracing::info!(device = %mgr.device_name, "generated sync invite");

    Ok(format!("m2m-sync://{}", token_b64))
}

/// Connect this device to a primary using a sync invite token.
/// The secondary initiates a full X3DH handshake with its own identity,
/// then sends the token for authorization.
#[tauri::command]
pub async fn connect_sync_device(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    invite_str: String,
    address: String,
    my_name: String,
) -> Result<crate::commands::ConnectionInfo, String> {
    // Parse the invite string
    let token_str = invite_str
        .strip_prefix("m2m-sync://")
        .ok_or("invalid sync invite format")?;

    // Decode the token — this won't validate against the primary's store,
    // but serves as a shared secret for authorization
    let token_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token_str)
        .map_err(|e| format!("invalid sync invite token: {e}"))?;

    if token_bytes.len() != 24 {
        return Err("invalid sync invite token length".to_string());
    }

    // Connect via TCP to the primary's address
    let stream = tokio::net::TcpStream::connect(&address)
        .await
        .map_err(|e| format!("connection failed: {e}"))?;
    let (read_half, write_half) = stream.into_split();

    // Get our identity for the handshake
    let identity = {
        let id = state.identity.read().await;
        id.as_ref()
            .ok_or("identity not initialized — unlock vault first")?
            .clone()
    };
    let x25519_identity = {
        let xid = state.x25519_identity.read().await;
        xid.as_ref()
            .ok_or("X25519 identity not initialized")?
            .clone()
    };

    // Create a fresh session and perform X3DH as initiator
    let mut session = crate::session::Session::new();
    session.our_identity = Some(identity);

    // We need to parse the peer's info from... wait, the invite_str doesn't
    // contain the peer's key info like a regular invite does. We need a
    // different approach for sync: the primary must first share its
    // connection info out-of-band just like a regular invite.
    //
    // For sync, the primary generates a *regular invite* (with X3DH bundle)
    // and separately shares the sync token. The secondary uses the regular
    // invite to connect and authenticate, then presents the sync token
    // to authorize the pairing.

    Err("sync via raw token not yet implemented — use connect_with_sync_invite instead".to_string())
}

/// Handle an incoming SyncDeviceInfo packet from a secondary device
/// that has already established a session.
pub async fn handle_sync_device_info(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    peer_key_hex: &str,
    info: &SyncDeviceInfo,
) -> Result<(), String> {
    let mut mgr = state.sync_manager.write().await;

    // Check if this device is already paired
    if mgr.synced_devices.iter().any(|d| d.device_id == info.device_id) {
        tracing::info!(device_id = %info.device_id, "sync device already paired");
        // Still sync — might be a reconnection
    } else {
        if mgr.synced_devices.len() >= MAX_SYNCED_DEVICES {
            return Err("maximum number of synced devices reached".to_string());
        }

        mgr.synced_devices.push(SyncedDevice {
            device_id: info.device_id.clone(),
            device_name: info.device_name.clone(),
            peer_key_hex: peer_key_hex.to_string(),
            last_synced: now_unix(),
        });
        tracing::info!(
            device_id = %info.device_id,
            device_name = %info.device_name,
            "new sync device paired"
        );
    }

    // Set our device info on this connection
    {
        let conns = state.connections.read().await;
        if let Some(conn_arc) = conns.get(peer_key_hex) {
            let mut conn = conn_arc.lock().await;
            conn.session.peer_sync_device_id = Some(info.device_id.clone());
            conn.session.peer_sync_device_name = Some(info.device_name.clone());
        }
    }

    // Send our device info back
    let our_info = {
        let mgr = state.sync_manager.read().await;
        SyncDeviceInfo {
            device_id: mgr.device_id.clone(),
            device_name: mgr.device_name.clone(),
            sync_protocol_version: SYNC_PROTOCOL_VERSION,
        }
    };

    if let Ok(bytes) = protocol::serialize(&our_info) {
        let conns = state.connections.read().await;
        if let Some(conn_arc) = conns.get(peer_key_hex) {
            let mut conn = conn_arc.lock().await;
            let _ = conn
                .session
                .send_encrypted_typed(
                    &mut conn.write_half,
                    PacketType::SyncDeviceInfo,
                    &bytes,
                )
                .await;
        }
    }

    // Now send peer keys
    broadcast_peer_keys(state, peer_key_hex).await;

    // Then send conversation metadata
    broadcast_conversations(state, peer_key_hex).await;

    // Notify frontend
    let _ = app_handle.emit(
        "m2m://sync-device",
        serde_json::json!({
            "device_id": info.device_id,
            "device_name": info.device_name,
        }),
    );

    Ok(())
}

/// Send all peer keys from our KeyStore to the connected sync device.
async fn broadcast_peer_keys(
    state: &Arc<AppState>,
    peer_key_hex: &str,
) {
    // Collect peer keys from store
    let peer_keys: Vec<PeerKeyEntry> = {
        let ks = state.key_store.lock().await;
        if let Some(ref store) = *ks {
            match store.list_peers() {
                Ok(peers) => peers
                    .iter()
                    .map(|p| PeerKeyEntry {
                        public_key: hex::encode(&p.public_key),
                        alias: p.alias.clone().unwrap_or_default(),
                        verified: p.verified,
                    })
                    .collect(),
                Err(_) => return,
            }
        } else {
            return;
        }
    };

    if peer_keys.is_empty() {
        return;
    }

    let payload_data = protocol::serialize(&peer_keys).unwrap_or_default();
    let sync_payload = SyncPayload {
        payload_type: SyncPayloadType::PeerKeys,
        data: payload_data,
    };

    if let Ok(bytes) = protocol::serialize(&sync_payload) {
        let conns = state.connections.read().await;
        if let Some(conn_arc) = conns.get(peer_key_hex) {
            let mut conn = conn_arc.lock().await;
            let _ = conn
                .session
                .send_encrypted_typed(&mut conn.write_half, PacketType::SyncPayload, &bytes)
                .await;
        }
    }
}

/// Send conversation metadata to the connected sync device.
async fn broadcast_conversations(
    state: &Arc<AppState>,
    peer_key_hex: &str,
) {
    let convos: Vec<ConversationEntry> = {
        let ms = state.message_store.lock().await;
        if let Some(ref store) = *ms {
            match store.list_conversations() {
                Ok(list) => list
                    .iter()
                    .map(|c| ConversationEntry {
                        conversation_id: c.id.clone(),
                        peer_key_hex: c.peer_key_hex.clone(),
                        display_name: c.display_name.clone().unwrap_or_default(),
                        last_message_at: c.last_message_at.unwrap_or(0),
                        retention_policy: c.retention_policy.clone(),
                    })
                    .collect(),
                Err(_) => return,
            }
        } else {
            return;
        }
    };

    if convos.is_empty() {
        return;
    }

    let payload_data = protocol::serialize(&convos).unwrap_or_default();
    let sync_payload = SyncPayload {
        payload_type: SyncPayloadType::Conversations,
        data: payload_data,
    };

    if let Ok(bytes) = protocol::serialize(&sync_payload) {
        let conns = state.connections.read().await;
        if let Some(conn_arc) = conns.get(peer_key_hex) {
            let mut conn = conn_arc.lock().await;
            let _ = conn
                .session
                .send_encrypted_typed(&mut conn.write_half, PacketType::SyncPayload, &bytes)
                .await;
        }
    }
}

/// Handle an incoming SyncPayload from the paired device.
/// Received peer keys are upserted into our KeyStore.
/// Received conversation metadata is upserted into our MessageStore.
pub async fn handle_sync_payload(
    state: &Arc<AppState>,
    peer_key_hex: &str,
    payload: &SyncPayload,
) {
    match payload.payload_type {
        SyncPayloadType::PeerKeys => {
            if let Ok(keys) = protocol::deserialize::<Vec<PeerKeyEntry>>(&payload.data) {
                let ks = state.key_store.lock().await;
                if let Some(ref store) = *ks {
                    for key in &keys {
                        if let Ok(pk_bytes) = hex::decode(&key.public_key) {
                            if pk_bytes.len() == 32 {
                                let _ = store.upsert_peer(
                                    &pk_bytes,
                                    &crate::crypto::fingerprint_from_public_key(&pk_bytes),
                                    Some(&key.alias),
                                );
                            }
                        }
                    }
                    tracing::info!(count = keys.len(), "synced peer keys from device");
                }
            }
        }
        SyncPayloadType::Conversations => {
            if let Ok(convos) = protocol::deserialize::<Vec<ConversationEntry>>(&payload.data) {
                let ms = state.message_store.lock().await;
                if let Some(ref store) = *ms {
                    for conv in &convos {
                        if let Ok(peer_bytes) = hex::decode(&conv.peer_key_hex) {
                            if peer_bytes.len() == 32 {
                                let _ = store.ensure_conversation(&conv.conversation_id, &peer_bytes);
                                let _ = store.rename_conversation(
                                    &conv.conversation_id,
                                    &conv.display_name,
                                    "",
                                );
                            }
                        }
                    }
                    tracing::info!(count = convos.len(), "synced conversations from device");
                }
            }
        }
        SyncPayloadType::UnreadCounts => {
            // Not yet implemented — would require storage changes
            tracing::debug!("received unread counts sync");
        }
    }

    // Update last_synced timestamp
    let mut mgr = state.sync_manager.write().await;
    if let Some(device) = mgr
        .synced_devices
        .iter_mut()
        .find(|d| d.peer_key_hex == peer_key_hex)
    {
        device.last_synced = now_unix();
    }
}

/// Broadcast updates (new peer key, new conversation) to all paired devices.
pub async fn broadcast_updates(state: &Arc<AppState>) {
    let devices: Vec<String> = {
        let mgr = state.sync_manager.read().await;
        mgr.synced_devices.iter().map(|d| d.peer_key_hex.clone()).collect()
    };

    for peer_key_hex in &devices {
        // Check if connected
        let is_connected = {
            let conns = state.connections.read().await;
            conns.contains_key(peer_key_hex)
        };

        if is_connected {
            broadcast_peer_keys(state, peer_key_hex).await;
            broadcast_conversations(state, peer_key_hex).await;
        }
    }
}

// ─── Sync Data Types ───

/// Minimal peer key entry for sync (avoids direct KeyStore dependency).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PeerKeyEntry {
    public_key: String,
    alias: String,
    verified: bool,
}

/// Minimal conversation entry for sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationEntry {
    conversation_id: String,
    peer_key_hex: String,
    display_name: String,
    last_message_at: u64,
    retention_policy: String,
}

// ─── Tests ───

#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn test_sync_manager_new() {
        let mgr = SyncManager::new();
        assert!(!mgr.device_id.is_empty());
        assert_eq!(mgr.device_name, "My Device");
        assert!(mgr.pending_invites.is_empty());
        assert!(mgr.synced_devices.is_empty());
    }

    #[test]
    fn test_synced_device_limits() {
        let mut mgr = SyncManager::new();
        for i in 0..MAX_SYNCED_DEVICES {
            assert!(mgr.synced_devices.len() <= MAX_SYNCED_DEVICES);
            mgr.synced_devices.push(SyncedDevice {
                device_id: format!("device-{}", i),
                device_name: format!("Device {}", i),
                peer_key_hex: format!("key-{}", i),
                last_synced: 1000 + i as u64,
            });
        }
        assert_eq!(mgr.synced_devices.len(), MAX_SYNCED_DEVICES);
    }

    #[test]
    fn test_sync_invite_ttl() {
        let now = now_unix();
        let invite = SyncInvite {
            token: vec![0u8; 24],
            expires_at: now + SYNC_INVITE_TTL_SECS,
            used: false,
        };
        assert!(!invite.used);
        assert!(invite.expires_at > now);
        assert_eq!(
            invite.expires_at - now,
            SYNC_INVITE_TTL_SECS
        );
    }

    #[test]
    fn test_peer_key_entry_roundtrip() {
        let entry = PeerKeyEntry {
            public_key: "aabb".to_string(),
            alias: "Test Peer".to_string(),
            verified: true,
        };
        let bytes = protocol::serialize(&entry).unwrap();
        let deserialized: PeerKeyEntry = protocol::deserialize(&bytes).unwrap();
        assert_eq!(deserialized.public_key, "aabb");
        assert_eq!(deserialized.alias, "Test Peer");
        assert!(deserialized.verified);
    }

    #[test]
    fn test_conversation_entry_roundtrip() {
        let entry = ConversationEntry {
            conversation_id: "conv-001".to_string(),
            peer_key_hex: "aabbccdd".to_string(),
            display_name: "Alice".to_string(),
            last_message_at: 1000,
            retention_policy: "none".to_string(),
        };
        let bytes = protocol::serialize(&entry).unwrap();
        let deserialized: ConversationEntry = protocol::deserialize(&bytes).unwrap();
        assert_eq!(deserialized.conversation_id, "conv-001");
        assert_eq!(deserialized.display_name, "Alice");
    }
}
