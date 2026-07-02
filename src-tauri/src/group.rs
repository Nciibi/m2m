/// M2M — Group Chat Module
///
/// Manages group state, member lifecycle, and Sender Key operations
/// for E2EE group messaging (Phase 3).
///
/// Each group has N members. Each member has their own Sender Key chain
/// for encrypting messages. All other members store a receiver chain
/// derived from the sender's initial chain key, allowing them to decrypt.
///
/// On member removal, all remaining members rotate their Sender Keys
/// to prevent the removed member from decrypting future messages.

use std::collections::HashMap;

use crate::crypto::{
    self, derive_receiver_chain, generate_sender_key_pair,
    generate_sender_signing_keypair, sign_group_message,
    verify_group_message_signature, SenderKeyChain,
};
use crate::protocol::{
    GroupCreateData, GroupEncryptedMessageData, GroupInfoData,
    GroupInviteData, GroupLeaveData, GroupRemoveData, GroupSenderKeyData,
};

/// Role a member holds in a group.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GroupRole {
    /// Can add/remove members, change group name.
    Admin,
    /// Standard member — can send messages and leave.
    Member,
}

impl std::fmt::Display for GroupRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupRole::Admin => write!(f, "admin"),
            GroupRole::Member => write!(f, "member"),
        }
    }
}

/// A member of a group.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupMember {
    /// Ed25519 public key hex of this member.
    pub peer_key_hex: String,
    /// Human-readable display name (None until known).
    pub display_name: Option<String>,
    /// Role in the group.
    pub role: GroupRole,
    /// Unix timestamp when they were added.
    pub added_at: u64,
}

/// Full state for a single group.
#[derive(Debug, Clone)]
pub struct Group {
    /// UUID v4 identifying the group.
    pub group_id: String,
    /// Human-readable group name.
    pub name: String,
    /// When the group was created (unix seconds).
    pub created_at: u64,
    /// Members of this group.
    pub members: Vec<GroupMember>,
    // ─── Sender Key State ───
    /// Our own sending chain (encrypts messages we send).
    pub our_sending_chain: Option<SenderKeyChain>,
    /// Our own initial chain key (the one we distributed to all members).
    /// Stored so we can re-distribute when new members join.
    pub our_initial_chain_key: Option<[u8; 32]>,
    /// Our own Ed25519 signing key secret (64 bytes) for this group.
    pub our_signing_key: Option<[u8; 64]>,
    /// Our own Ed25519 verification key (32 bytes) for this group.
    pub our_verification_key: Option<[u8; 32]>,
    /// Receiver chains for other members (peer_key_hex -> SenderKeyChain).
    /// Used to decrypt messages FROM those members.
    pub receiver_chains: HashMap<String, SenderKeyChain>,
    /// Verification keys for other members (peer_key_hex -> 32 bytes).
    /// Used to verify message signatures FROM those members.
    pub verification_keys: HashMap<String, [u8; 32]>,
    // ─── Metadata ───
    /// Timestamp of the last message (0 = none).
    pub last_message_at: u64,
    /// Preview of the last message.
    pub last_message_preview: Option<String>,
}

impl Group {
    /// Create a new group as the admin/creator.
    /// Generates our Sender Key chain and signing key.
    pub fn new(
        group_id: String,
        name: String,
        created_at: u64,
        our_peer_key_hex: String,
    ) -> Self {
        // Generate sender key pair for ourselves
        let (sending_chain, initial_chain_key) = generate_sender_key_pair();
        let (signing_key, verification_key) = generate_sender_signing_keypair();

        let our_member = GroupMember {
            peer_key_hex: our_peer_key_hex,
            display_name: None,
            role: GroupRole::Admin,
            added_at: created_at,
        };

        Self {
            group_id,
            name,
            created_at,
            members: vec![our_member],
            our_sending_chain: Some(sending_chain),
            our_initial_chain_key: Some(initial_chain_key),
            our_signing_key: Some(signing_key),
            our_verification_key: Some(verification_key),
            receiver_chains: HashMap::new(),
            verification_keys: HashMap::new(),
            last_message_at: 0,
            last_message_preview: None,
        }
    }

    /// Whether we are an admin of this group.
    pub fn is_admin(&self, our_peer_key_hex: &str) -> bool {
        self.members
            .iter()
            .any(|m| m.peer_key_hex == our_peer_key_hex && m.role == GroupRole::Admin)
    }

    /// Whether a peer is a member of this group.
    pub fn is_member(&self, peer_key_hex: &str) -> bool {
        self.members.iter().any(|m| m.peer_key_hex == peer_key_hex)
    }

    /// Get a member by peer key.
    pub fn get_member(&self, peer_key_hex: &str) -> Option<&GroupMember> {
        self.members.iter().find(|m| m.peer_key_hex == peer_key_hex)
    }

    /// Get the count of members.
    pub fn member_count(&self) -> u32 {
        self.members.len() as u32
    }

    /// Encrypt a plaintext message using our sending chain.
    /// Returns the raw GroupEncryptedMessageData ready to send over DR sessions.
    /// The caller is responsible for sending this to all online members.
    pub fn encrypt_message(
        &mut self,
        our_peer_key_hex: &str,
        plaintext: &[u8],
    ) -> Result<GroupEncryptedMessageData, String> {
        let chain = self
            .our_sending_chain
            .as_mut()
            .ok_or("no sending chain available")?;

        let (nonce, msg_key) = chain
            .next_message_key()
            .map_err(|e| format!("sender key derivation failed: {e}"))?;

        let aead_key = sodiumoxide::crypto::aead::xchacha20poly1305_ietf::Key::from_slice(&msg_key)
            .ok_or("invalid AEAD key")?;

        let padded = crypto::pad_message_variable(plaintext);
        let ciphertext = sodiumoxide::crypto::aead::xchacha20poly1305_ietf::seal(
            &padded,
            None,
            &sodiumoxide::crypto::aead::xchacha20poly1305_ietf::Nonce::from_slice(&nonce)
                .ok_or("invalid nonce")?,
            &aead_key,
        );

        let message_number = chain.current_message_number() - 1; // next_message_key already advanced

        // Build data to sign: group_id || message_number || nonce || ciphertext
        let mut sign_data = Vec::with_capacity(16 + 8 + 24 + ciphertext.len());
        sign_data.extend_from_slice(self.group_id.as_bytes());
        sign_data.extend_from_slice(&message_number.to_be_bytes());
        sign_data.extend_from_slice(&nonce);
        sign_data.extend_from_slice(&ciphertext);

        let signing_key = self
            .our_signing_key
            .as_ref()
            .ok_or("no signing key available")?;
        let signature = sign_group_message(signing_key, &sign_data)
            .map_err(|e| format!("signing failed: {e}"))?;

        Ok(GroupEncryptedMessageData {
            group_id: self.group_id.clone(),
            sender_peer_key_hex: our_peer_key_hex.to_string(),
            message_number,
            ciphertext,
            nonce: nonce.to_vec(),
            signature,
        })
    }

    /// Decrypt a group message from another member.
    /// Uses the receiver chain for that sender to derive the message key.
    pub fn decrypt_message(
        &mut self,
        data: &GroupEncryptedMessageData,
    ) -> Result<Vec<u8>, String> {
        // Verify signature first
        let verification_key = self
            .verification_keys
            .get(&data.sender_peer_key_hex)
            .ok_or("unknown sender — no verification key for this peer")?;

        let mut sign_data = Vec::with_capacity(16 + 8 + 24 + data.ciphertext.len());
        sign_data.extend_from_slice(self.group_id.as_bytes());
        sign_data.extend_from_slice(&data.message_number.to_be_bytes());
        sign_data.extend_from_slice(&data.nonce);
        sign_data.extend_from_slice(&data.ciphertext);

        if !verify_group_message_signature(verification_key, &sign_data, &data.signature) {
            return Err("group message signature verification failed".to_string());
        }

        // Derive message key from receiver chain
        let chain = self
            .receiver_chains
            .get_mut(&data.sender_peer_key_hex)
            .ok_or("no receiver chain for this sender")?;

        let (nonce, msg_key) = chain
            .peek_message_key(data.message_number)
            .map_err(|e| format!("receiver key derivation failed: {e}"))?;

        let aead_key = sodiumoxide::crypto::aead::xchacha20poly1305_ietf::Key::from_slice(&msg_key)
            .ok_or("invalid AEAD key")?;

        let aead_nonce =
            sodiumoxide::crypto::aead::xchacha20poly1305_ietf::Nonce::from_slice(&nonce)
                .ok_or("invalid nonce")?;

        let padded = sodiumoxide::crypto::aead::xchacha20poly1305_ietf::open(
            &data.ciphertext,
            None,
            &aead_nonce,
            &aead_key,
        )
        .map_err(|_| "group message decryption failed (AEAD open)".to_string())?;

        let plaintext = crypto::unpad_message_variable(&padded)
            .map_err(|e| format!("unpad failed: {e}"))?;

        Ok(plaintext)
    }

    /// Store a receiver's sender key bundle for a member.
    /// This allows us to decrypt messages from that member.
    pub fn store_receiver_key(
        &mut self,
        sender_peer_key_hex: &str,
        chain_key: &[u8; 32],
        verification_key: &[u8; 32],
    ) {
        let chain = derive_receiver_chain(chain_key);
        self.receiver_chains
            .insert(sender_peer_key_hex.to_string(), chain);
        self.verification_keys
            .insert(sender_peer_key_hex.to_string(), *verification_key);
    }

    /// Rotate our own Sender Key (after member removal).
    /// Generates a new sending chain + signing keypair.
    /// Returns the new initial chain key and verification key to distribute.
    pub fn rotate_own_sender_key(
        &mut self,
    ) -> Result<([u8; 32], [u8; 32]), String> {
        let (sending_chain, initial_chain_key) = generate_sender_key_pair();
        let (signing_key, verification_key) = generate_sender_signing_keypair();
        self.our_sending_chain = Some(sending_chain);
        self.our_initial_chain_key = Some(initial_chain_key);
        self.our_signing_key = Some(signing_key);
        self.our_verification_key = Some(verification_key);
        Ok((initial_chain_key, verification_key))
    }
}

/// Manager for all groups the local user belongs to.
pub struct GroupManager {
    /// All active groups, keyed by group_id.
    pub groups: HashMap<String, Group>,
}

impl GroupManager {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Create a new group.
    /// Returns the Group and a list of GroupSenderKeyData bundles to distribute
    /// to each initial member (excluding self).
    pub fn create_group(
        &mut self,
        group_id: String,
        name: String,
        created_at: u64,
        our_peer_key_hex: String,
        initial_members: &[String],
    ) -> Result<(String, Vec<(String, GroupSenderKeyData)>), String> {
        if self.groups.contains_key(&group_id) {
            return Err("group already exists".to_string());
        }
        if initial_members.len() > 31 {
            return Err("group size exceeds maximum (32 members)".to_string());
        }

        let mut group = Group::new(group_id.clone(), name, created_at, our_peer_key_hex.clone());

        // For each initial member: add them and prepare their sender key bundle
        let mut bundles = Vec::with_capacity(initial_members.len());

        for member_key_hex in initial_members {
            // This member gets OUR sender key (so they can decrypt our messages)
            let (_, their_initial_key) = generate_sender_key_pair();
            let (their_signing_key, their_verification_key) = generate_sender_signing_keypair();

            // Store THEIR receiver chain (so WE can decrypt THEIR messages)
            group.store_receiver_key(member_key_hex, &their_initial_key, &their_verification_key);

            // Build the bundle for THEM:
            // - They get THEIR OWN signing key (so they can send)
            // - They get OUR initial chain key + verification key (so they can decrypt our messages)
            let our_init_key = group.our_initial_chain_key.ok_or("no our initial key")?;
            let our_verify_key = group.our_verification_key.ok_or("no our verification key")?;

            // Their own sender key bundle (signing key included — only for the recipient)
            let their_own_bundle = GroupSenderKeyData {
                group_id: group_id.clone(),
                sender_peer_key_hex: member_key_hex.clone(),
                chain_key: their_initial_key,
                message_number: 0,
                signing_key: Some(their_signing_key.to_vec()),
                verification_key: their_verification_key,
                signature: Vec::new(), // Filled in by the caller with identity key
            };

            // Our sender key bundle (no signing key — just receiver info for them)
            let our_bundle = GroupSenderKeyData {
                group_id: group_id.clone(),
                sender_peer_key_hex: our_peer_key_hex.clone(),
                chain_key: our_init_key,
                message_number: 0,
                signing_key: None,
                verification_key: our_verify_key,
                signature: Vec::new(),
            };

            // Actually: we send them TWO key bundles, but they go in the same
            // GroupSenderKey packet. We'll use the sender_peer_key_hex pattern:
            // The member receives their own sender key AND our sender key separately.
            // For simplicity in the protocol, we send them as two separate 0x53 packets.

            bundles.push((member_key_hex.clone(), their_own_bundle));
            bundles.push((member_key_hex.clone(), our_bundle));

            // Add member
            group.members.push(GroupMember {
                peer_key_hex: member_key_hex.clone(),
                display_name: None,
                role: GroupRole::Member,
                added_at: created_at,
            });
        }

        self.groups.insert(group_id.clone(), group);
        Ok((group_id, bundles))
    }

    /// Add a new member to an existing group.
    /// Returns bundles to send over the new member's 1:1 DR session.
    pub fn add_member(
        &mut self,
        group_id: &str,
        new_member_key_hex: &str,
        our_peer_key_hex: &str,
        added_at: u64,
    ) -> Result<Vec<GroupSenderKeyData>, String> {
        let group = self
            .groups
            .get_mut(group_id)
            .ok_or("group not found")?;

        if group.is_member(new_member_key_hex) {
            return Err("member already in group".to_string());
        }
        if group.members.len() >= 32 {
            return Err("group size exceeds maximum (32 members)".to_string());
        }

        // Generate a sender key for the new member
        let (_, member_initial_key) = generate_sender_key_pair();
        let (member_signing_key, member_verification_key) = generate_sender_signing_keypair();

        // Store their receiver chain for us
        group.store_receiver_key(new_member_key_hex, &member_initial_key, &member_verification_key);

        // The new member needs:
        // 1. Their own sender key (with signing key)
        // 2. Our sender key (without signing key, for receiving our messages)

        let our_init_key = group
            .our_initial_chain_key
            .ok_or("no our initial key")?;
        let our_verify_key = group
            .our_verification_key
            .ok_or("no our verification key")?;

        let their_own_bundle = GroupSenderKeyData {
            group_id: group_id.to_string(),
            sender_peer_key_hex: new_member_key_hex.to_string(),
            chain_key: member_initial_key,
            message_number: 0,
            signing_key: Some(member_signing_key.to_vec()),
            verification_key: member_verification_key,
            signature: Vec::new(),
        };

        let our_bundle = GroupSenderKeyData {
            group_id: group_id.to_string(),
            sender_peer_key_hex: our_peer_key_hex.to_string(),
            chain_key: our_init_key,
            message_number: 0,
            signing_key: None,
            verification_key: our_verify_key,
            signature: Vec::new(),
        };

        // Add member
        group.members.push(GroupMember {
            peer_key_hex: new_member_key_hex.to_string(),
            display_name: None,
            role: GroupRole::Member,
            added_at,
        });

        // For each existing member (excluding us and the new member):
        // We need to send THEIR sender key to the new member too,
        // but only the receiver chain (no signing key).
        // NOTE: The new member needs each existing member's receiver chain,
        // but we don't store the initial chain key for existing members (only
        // the current receiver chain state). Phase 3 v2 should store initial
        // keys alongside receiver chains to support this.
        let mut bundles = vec![their_own_bundle, our_bundle];

        Ok(bundles)
    }

    /// Remove a member from a group. Rotates keys for all remaining members.
    /// Returns the new sender key bundles to distribute.
    pub fn remove_member(
        &mut self,
        group_id: &str,
        removed_key_hex: &str,
        our_peer_key_hex: &str,
    ) -> Result<Vec<(String, GroupSenderKeyData)>, String> {
        let group = self
            .groups
            .get_mut(group_id)
            .ok_or("group not found")?;

        // Verify we're admin
        if !group.is_admin(our_peer_key_hex) {
            return Err("only admins can remove members".to_string());
        }

        // Remove from members list
        let pos = group
            .members
            .iter()
            .position(|m| m.peer_key_hex == removed_key_hex)
            .ok_or("member not in group")?;
        group.members.remove(pos);

        // Remove their receiver chain and verification key
        group.receiver_chains.remove(removed_key_hex);
        group.verification_keys.remove(removed_key_hex);

        // Rotate OUR sender key (forward secrecy for removed member)
        let (new_initial_key, new_verification_key) = group.rotate_own_sender_key()?;

        // Build new key bundles for all remaining members
        let mut bundles = Vec::new();
        for member in &group.members {
            if member.peer_key_hex == our_peer_key_hex {
                continue;
            }
            bundles.push((
                member.peer_key_hex.clone(),
                GroupSenderKeyData {
                    group_id: group_id.to_string(),
                    sender_peer_key_hex: our_peer_key_hex.to_string(),
                    chain_key: new_initial_key,
                    message_number: 0,
                    signing_key: None,
                    verification_key: new_verification_key,
                    signature: Vec::new(),
                },
            ));
        }

        Ok(bundles)
    }

    /// Handle a member leaving voluntarily.
    pub fn leave_group(
        &mut self,
        group_id: &str,
        leaving_key_hex: &str,
    ) -> Result<(), String> {
        let group = self
            .groups
            .get_mut(group_id)
            .ok_or("group not found")?;

        let pos = group
            .members
            .iter()
            .position(|m| m.peer_key_hex == leaving_key_hex)
            .ok_or("member not in group")?;
        group.members.remove(pos);
        group.receiver_chains.remove(leaving_key_hex);
        group.verification_keys.remove(leaving_key_hex);

        Ok(())
    }

    /// Handle receiving a GroupSenderKey bundle from another member.
    ///
    /// The bundle ID scheme:
    /// - If `signing_key` is `Some(...)`, then this bundle contains OUR OWN signing key
    ///   (i.e., the recipient should set up their sending chain). The `sender_peer_key_hex`
    ///   identifies whose chain this is — if it's the recipient's own key, it's the
    ///   sending chain. Callers should verify the sender_peer_key_hex matches locally.
    /// - If `signing_key` is `None`, then this bundle contains another member's sender
    ///   key info and should be stored as a receiver chain to decrypt their messages.
    ///
    /// The `our_peer_key_hex` parameter tells us who WE are, so we can determine
    /// whether a bundle with a signing key is ours.
    pub fn handle_sender_key(
        &mut self,
        data: &GroupSenderKeyData,
        our_peer_key_hex: &str,
    ) -> Result<(), String> {
        let group = self
            .groups
            .get_mut(&data.group_id)
            .ok_or("group not found")?;

        // If this bundle contains a signing key, it's OUR sending chain
        // (the bundle was generated specifically for us).
        if let Some(sk_bytes) = &data.signing_key {
            // Only set up our sending chain if the bundle is addressed to us.
            // The sender_peer_key_hex tells us which peer this chain represents —
            // if it matches our own key, it's our sender chain.
            if data.sender_peer_key_hex == our_peer_key_hex {
                let mut sk = [0u8; 64];
                if sk_bytes.len() == 64 {
                    sk.copy_from_slice(&sk_bytes[..64]);
                }
                group.our_signing_key = Some(sk);
                group.our_verification_key = Some(data.verification_key);
                group.our_initial_chain_key = Some(data.chain_key);
                group.our_sending_chain = Some(crate::crypto::SenderKeyChain::new(data.chain_key));
                return Ok(());
            }
            // If the signing key is present but sender is NOT us, something is wrong
            // (signing keys should only be sent to the key's owner). Treat as error.
            return Err("received a sender key bundle with signing key for a different peer".to_string());
        }

        // Otherwise, store as a receiver chain for this sender
        group.store_receiver_key(
            &data.sender_peer_key_hex,
            &data.chain_key,
            &data.verification_key,
        );

        Ok(())
    }

    /// Get a group by ID.
    pub fn get_group(&self, group_id: &str) -> Option<&Group> {
        self.groups.get(group_id)
    }

    /// Get a mutable group by ID.
    pub fn get_group_mut(&mut self, group_id: &str) -> Option<&mut Group> {
        self.groups.get_mut(group_id)
    }

    /// List all groups with summary info.
    pub fn list_groups(&self) -> Vec<GroupSummary> {
        self.groups
            .values()
            .map(|g| GroupSummary {
                group_id: g.group_id.clone(),
                group_name: g.name.clone(),
                member_count: g.member_count(),
                created_at: g.created_at,
                last_message_at: g.last_message_at,
                last_message_preview: g.last_message_preview.clone(),
            })
            .collect()
    }

    /// Remove a group entirely.
    pub fn remove_group(&mut self, group_id: &str) {
        self.groups.remove(group_id);
    }

    /// Update group name.
    pub fn update_group_name(&mut self, group_id: &str, new_name: &str) -> Result<(), String> {
        let group = self
            .groups
            .get_mut(group_id)
            .ok_or("group not found")?;
        group.name = new_name.to_string();
        Ok(())
    }
}

/// Summary info for a group, used for list display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupSummary {
    pub group_id: String,
    pub group_name: String,
    pub member_count: u32,
    pub created_at: u64,
    pub last_message_at: u64,
    pub last_message_preview: Option<String>,
}

/// Full group detail including members, for frontend display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupDetail {
    pub group_id: String,
    pub group_name: String,
    pub member_count: u32,
    pub created_at: u64,
    pub our_role: String,
    pub members: Vec<GroupMember>,
    pub last_message_at: u64,
    pub last_message_preview: Option<String>,
}

impl From<&Group> for GroupDetail {
    fn from(g: &Group) -> Self {
        Self {
            group_id: g.group_id.clone(),
            group_name: g.name.clone(),
            member_count: g.member_count(),
            created_at: g.created_at,
            our_role: "admin".to_string(), // approximate; caller should check
            members: g.members.clone(),
            last_message_at: g.last_message_at,
            last_message_preview: g.last_message_preview.clone(),
        }
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;

    fn make_group_manager() -> GroupManager {
        GroupManager::new()
    }

    #[test]
    fn test_create_group() {
        let mut gm = make_group_manager();
        let result = gm.create_group(
            "group-1".to_string(),
            "Test Group".to_string(),
            1719000000,
            "alice".to_string(),
            &["bob".to_string(), "charlie".to_string()],
        );
        assert!(result.is_ok());
        let (gid, bundles) = result.unwrap();
        assert_eq!(gid, "group-1");
        // 2 members = 4 bundles (their_own + our for each)
        assert_eq!(bundles.len(), 4);

        let group = gm.get_group("group-1").unwrap();
        assert_eq!(group.members.len(), 3); // alice + bob + charlie
        assert_eq!(group.member_count(), 3);
    }

    #[test]
    fn test_create_group_exceeds_max_size() {
        let mut gm = make_group_manager();
        let members: Vec<String> = (0..32).map(|i| format!("peer-{}", i)).collect();
        let result = gm.create_group(
            "group-big".to_string(),
            "Big Group".to_string(),
            1719000000,
            "alice".to_string(),
            &members,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_create_group_duplicate_id() {
        let mut gm = make_group_manager();
        gm.create_group(
            "dup".to_string(),
            "First".to_string(),
            100,
            "alice".to_string(),
            &[],
        )
        .unwrap();
        let result = gm.create_group(
            "dup".to_string(),
            "Second".to_string(),
            200,
            "alice".to_string(),
            &[],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_group_is_member() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();
        let group = gm.get_group("g1").unwrap();
        assert!(group.is_member("alice"));
        assert!(group.is_member("bob"));
        assert!(!group.is_member("charlie"));
    }

    #[test]
    fn test_group_is_admin() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();
        let group = gm.get_group("g1").unwrap();
        assert!(group.is_admin("alice"));
        assert!(!group.is_admin("bob"));
    }

    #[test]
    fn test_add_member() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        let result = gm.add_member("g1", "charlie", "alice", 200);
        assert!(result.is_ok());

        let group = gm.get_group("g1").unwrap();
        assert_eq!(group.members.len(), 3);
        assert!(group.is_member("charlie"));
    }

    #[test]
    fn test_add_duplicate_member_fails() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        let result = gm.add_member("g1", "bob", "alice", 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_member() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string(), "charlie".to_string()],
        )
        .unwrap();

        let result = gm.remove_member("g1", "bob", "alice");
        assert!(result.is_ok());

        let group = gm.get_group("g1").unwrap();
        assert_eq!(group.members.len(), 2);
        assert!(!group.is_member("bob"));
    }

    #[test]
    fn test_remove_member_triggers_rotation() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        // Save old initial chain key
        let old_key = gm.get_group("g1").unwrap().our_initial_chain_key;

        let _ = gm.remove_member("g1", "bob", "alice");

        let new_key = gm.get_group("g1").unwrap().our_initial_chain_key;
        assert!(old_key != new_key, "sender key should rotate after removal");
    }

    #[test]
    fn test_remove_nonexistent_member_fails() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &[],
        )
        .unwrap();

        let result = gm.remove_member("g1", "bob", "alice");
        assert!(result.is_err());
    }

    #[test]
    fn test_leave_group() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        let result = gm.leave_group("g1", "bob");
        assert!(result.is_ok());

        let group = gm.get_group("g1").unwrap();
        assert_eq!(group.members.len(), 1);
        assert!(!group.is_member("bob"));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        // Bob needs to receive Alice's sender key to decrypt
        {
            let group = gm.get_group_mut("g1").unwrap();
            let init_key = group.our_initial_chain_key.unwrap();
            let verify_key = group.our_verification_key.unwrap();
            // Bob stores Alice's receiver chain (simulating receiving the bundle)
            group.store_receiver_key("alice", &init_key, &verify_key);
        }

        // Alice encrypts a message
        let plaintext = b"Hello group!";
        let encrypted = {
            let group = gm.get_group_mut("g1").unwrap();
            group
                .encrypt_message("alice", plaintext)
                .unwrap()
        };

        // Bob (via receiver chain) decrypts
        let decrypted = {
            let group = gm.get_group_mut("g1").unwrap();
            group.decrypt_message(&encrypted).unwrap()
        };

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_unknown_sender_fails() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        let plaintext = b"Hello!";
        let encrypted = {
            let group = gm.get_group_mut("g1").unwrap();
            group.encrypt_message("alice", plaintext).unwrap()
        };

        // Try to decrypt as unknown sender (no receiver chain for "eve")
        let group = gm.get_group_mut("g1").unwrap();
        let result = group.decrypt_message(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_groups() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "Alpha".to_string(),
            100,
            "alice".to_string(),
            &[],
        )
        .unwrap();
        gm.create_group(
            "g2".to_string(),
            "Beta".to_string(),
            200,
            "alice".to_string(),
            &[],
        )
        .unwrap();

        let list = gm.list_groups();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|g| g.group_name == "Alpha"));
        assert!(list.iter().any(|g| g.group_name == "Beta"));
    }

    #[test]
    fn test_encrypt_multiple_messages() {
        let mut gm = make_group_manager();
        gm.create_group(
            "g1".to_string(),
            "G".to_string(),
            100,
            "alice".to_string(),
            &["bob".to_string()],
        )
        .unwrap();

        // Bob stores Alice's key
        {
            let group = gm.get_group_mut("g1").unwrap();
            let init_key = group.our_initial_chain_key.unwrap();
            let verify_key = group.our_verification_key.unwrap();
            group.store_receiver_key("alice", &init_key, &verify_key);
        }

        // Send multiple messages
        for i in 0..5 {
            let msg = format!("Message {}", i);
            let encrypted = {
                let group = gm.get_group_mut("g1").unwrap();
                group.encrypt_message("alice", msg.as_bytes()).unwrap()
            };
            let decrypted = {
                let group = gm.get_group_mut("g1").unwrap();
                String::from_utf8(group.decrypt_message(&encrypted).unwrap()).unwrap()
            };
            assert_eq!(decrypted, msg);
        }
    }
}
