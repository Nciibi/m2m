//! M2M — Group Chat Module
//!
//! Manages group state, member lifecycle, and Sender Key operations
//! for E2EE group messaging (Phase 3).
//!
//! Each group has N members. Each member has their own Sender Key chain
//! for encrypting messages. All other members store a receiver chain
//! derived from the sender's initial chain key, allowing them to decrypt.
//!
//! On member removal, all remaining members rotate their Sender Keys
//! to prevent the removed member from decrypting future messages.

use std::collections::HashMap;

use crate::crypto::{
    self, derive_receiver_chain, generate_sender_key_pair,
    generate_sender_signing_keypair, sign_group_message,
    verify_group_message_signature, SenderKeyChain,
};
use crate::protocol::{
    GroupEncryptedMessageData, GroupSenderKeyData,
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

impl Drop for Group {
    fn drop(&mut self) {
        // Wipe per-group secret material. SenderKeyChain instances wipe
        // themselves via their own Drop.
        use zeroize::Zeroize;
        if let Some(k) = self.our_initial_chain_key.as_mut() {
            k.zeroize();
        }
        if let Some(k) = self.our_signing_key.as_mut() {
            k.zeroize();
        }
        for k in self.verification_keys.values_mut() {
            k.zeroize();
        }
        self.verification_keys.clear();
    }
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

    /// Build OUR OWN unsigned sender-key bundle for distribution.
    ///
    /// The caller MUST sign it with our long-term Ed25519 identity key
    /// (`signature` field) before sending — receivers verify that signature
    /// against the transport peer's identity key (H2 trust model v2).
    pub fn own_sender_bundle(&self) -> Result<GroupSenderKeyData, String> {
        let chain_key = self.our_initial_chain_key.ok_or("no our initial key")?;
        let verification_key = self.our_verification_key.ok_or("no our verification key")?;
        Ok(GroupSenderKeyData {
            group_id: self.group_id.clone(),
            sender_peer_key_hex: String::new(), // filled by caller (our hex)
            chain_key,
            message_number: 0,
            signing_key: None, // private keys are NEVER distributed (H2)
            verification_key,
            signature: Vec::new(),
        })
    }
}

/// Canonical bytes covered by a GroupSenderKeyData signature (trust model v2).
///
/// Binds the chain key and verification key to the owning member's long-term
/// Ed25519 identity within a specific group. The private `signing_key` field
/// is deliberately excluded — it must always be None under model v2.
pub fn sender_key_bundle_sign_bytes(data: &GroupSenderKeyData) -> Vec<u8> {
    let mut b = Vec::with_capacity(
        8 + data.group_id.len() + data.sender_peer_key_hex.len() + 64,
    );
    b.extend_from_slice(b"M2M-GSK2");
    b.extend_from_slice(data.group_id.as_bytes());
    b.extend_from_slice(data.sender_peer_key_hex.as_bytes());
    b.extend_from_slice(&data.chain_key);
    b.extend_from_slice(&data.verification_key);
    b
}

/// Manager for all groups the local user belongs to.
pub struct GroupManager {
    /// All active groups, keyed by group_id.
    pub groups: HashMap<String, Group>,
}

/// Outcome of processing an incoming sender-key bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderKeyReceipt {
    /// Sender was already known (key rotated or re-sent).
    Known,
    /// Sender was previously unknown — caller should reply with our own
    /// bundle to complete mutual key exchange.
    NewMember,
}

impl GroupManager {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Create a new group.
    ///
    /// Trust model v2: the admin generates ONLY their own sending chain and
    /// signing key. Each member generates their own keys locally when they
    /// receive the GroupCreate roster — the admin never holds or ships
    /// member private keys. Returns one bundle per initial member (our own,
    /// unsigned — caller signs with the long-term identity key).
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

        let mut bundles = Vec::with_capacity(initial_members.len());
        let our_bundle = group.own_sender_bundle()?;

        for member_key_hex in initial_members {
            bundles.push((member_key_hex.clone(), our_bundle.clone()));

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

    /// Add a new member to an existing group (admin side).
    ///
    /// Trust model v2: the admin generates NOTHING on behalf of the new
    /// member. The new member generates their own keys when they receive the
    /// invite. Returns our own bundle to send to them (caller signs it).
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

        let our_bundle = group.own_sender_bundle()?;

        // Add member
        group.members.push(GroupMember {
            peer_key_hex: new_member_key_hex.to_string(),
            display_name: None,
            role: GroupRole::Member,
            added_at,
        });

        Ok(vec![our_bundle])
    }

    /// Join a group from the receiving side (GroupCreate / GroupInvite).
    ///
    /// Creates local state with OUR OWN freshly-generated sending chain and
    /// signing key — we never accept key material generated for us by
    /// someone else. Roster members are recorded so messages can be routed.
    ///
    /// Returns our unsigned bundle; caller signs with the long-term identity
    /// key and fans it out to roster members.
    pub fn join_group(
        &mut self,
        group_id: String,
        name: String,
        created_at: u64,
        our_peer_key_hex: String,
        is_admin: bool,
        roster: &[String],
    ) -> Result<GroupSenderKeyData, String> {
        if let Some(existing) = self.groups.get_mut(&group_id) {
            // Already joined — just make sure the roster includes everyone.
            for peer in roster {
                if !existing.is_member(peer) && peer != &our_peer_key_hex {
                    existing.members.push(GroupMember {
                        peer_key_hex: peer.clone(),
                        display_name: None,
                        role: GroupRole::Member,
                        added_at: created_at,
                    });
                }
            }
            return existing.own_sender_bundle();
        }

        if roster.len() > 31 {
            return Err("group size exceeds maximum (32 members)".to_string());
        }

        // Generate OUR OWN keys locally.
        let mut group = Group::new(group_id.clone(), name, created_at, our_peer_key_hex.clone());
        if is_admin {
            group.members[0].role = GroupRole::Admin;
        }

        for peer in roster {
            if peer == &our_peer_key_hex || group.is_member(peer) {
                continue;
            }
            group.members.push(GroupMember {
                peer_key_hex: peer.clone(),
                display_name: None,
                role: GroupRole::Member,
                added_at: created_at,
            });
        }

        let bundle = group.own_sender_bundle()?;
        self.groups.insert(group_id, group);
        Ok(bundle)
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
    /// Trust model v2 — every bundle MUST be:
    /// 1. Free of private key material (`signing_key` must be None — signing
    ///    keys are generated locally by their owner and never distributed).
    /// 2. Sent by its owner: `data.sender_peer_key_hex` must equal the hex of
    ///    the long-term identity key of the DIRECT transport peer.
    /// 3. Signed by that identity key over
    ///    [`sender_key_bundle_sign_bytes`](`self::sender_key_bundle_sign_bytes`).
    ///
    /// Returns [`SenderKeyReceipt::NewMember`] when the sender was previously
    /// unknown, so the caller can reply with our own bundle (mutual exchange;
    /// also how late joiners receive existing members' chain keys).
    pub fn handle_sender_key(
        &mut self,
        data: &GroupSenderKeyData,
        our_peer_key_hex: &str,
        peer_identity_pub: &[u8; 32],
    ) -> Result<SenderKeyReceipt, String> {
        let _ = our_peer_key_hex;

        if data.signing_key.is_some() {
            return Err(
                "rejected sender key bundle: contains private signing key material".to_string(),
            );
        }

        // The bundle must be owned by the direct transport peer.
        let peer_hex = hex::encode(peer_identity_pub);
        if data.sender_peer_key_hex != peer_hex {
            return Err(format!(
                "rejected sender key bundle: claimed sender is not the transport peer"
            ));
        }

        // Signature must verify under the transport peer's identity key.
        let sign_bytes = sender_key_bundle_sign_bytes(data);
        crate::crypto::verify_signature(peer_identity_pub, &sign_bytes, &data.signature)
            .map_err(|_| "rejected sender key bundle: invalid signature".to_string())?;

        let group = self
            .groups
            .get_mut(&data.group_id)
            .ok_or("group not found")?;

        let is_new = !group.verification_keys.contains_key(&data.sender_peer_key_hex)
            && !group.is_member(&data.sender_peer_key_hex);

        group.store_receiver_key(
            &data.sender_peer_key_hex,
            &data.chain_key,
            &data.verification_key,
        );

        Ok(if is_new {
            SenderKeyReceipt::NewMember
        } else {
            SenderKeyReceipt::Known
        })
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
#[allow(dead_code)]
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
        // Trust model v2: one bundle per initial member (our own, unsigned —
        // members generate their own keys locally).
        assert_eq!(bundles.len(), 2);
        for (_, b) in &bundles {
            assert!(b.signing_key.is_none(), "private keys must never be shipped");
        }

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
        let old_key = gm.get_group("g1").unwrap().our_initial_chain_key.clone();

        let _ = gm.remove_member("g1", "bob", "alice");

        let new_key = gm.get_group("g1").unwrap().our_initial_chain_key.clone();
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
            let init_key = group.our_initial_chain_key.clone().unwrap();
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
            let init_key = group.our_initial_chain_key.clone().unwrap();
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

    // ─── Trust model v2 (H2): signed sender-key bundles ─────────────────────

    fn make_signed_bundle(
        owner_id: &crate::crypto::IdentityKeypair,
        group_id: &str,
    ) -> (GroupSenderKeyData, [u8; 32]) {
        let mut gm = make_group_manager();
        let owner_hex = hex::encode(owner_id.public_key_bytes());
        gm.create_group(group_id.to_string(), "G".to_string(), 1, owner_hex.clone(), &[])
            .unwrap();
        let mut bundle = {
            let g = gm.get_group(group_id).unwrap();
            g.own_sender_bundle().unwrap()
        };
        bundle.sender_peer_key_hex = owner_hex.clone();
        bundle.signature = owner_id.sign(&sender_key_bundle_sign_bytes(&bundle));
        (bundle, owner_id.public_key_bytes())
    }

    #[test]
    fn test_sender_key_bundle_signature_roundtrip() {
        let alice_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let alice_hex = hex::encode(alice_id.public_key_bytes());
        let bob_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let bob_hex = hex::encode(bob_id.public_key_bytes());

        let mut gm_alice = make_group_manager();
        gm_alice.create_group("g".to_string(), "G".to_string(), 1, alice_hex.clone(), &[])
            .unwrap();

        // Bob joins with his own keys and announces a signed bundle.
        let mut gm_bob = make_group_manager();
        gm_bob.join_group("g".to_string(), "G".to_string(), 1, bob_hex.clone(), false, &[alice_hex.clone()])
            .unwrap();
        let mut bundle = {
            let g = gm_bob.get_group("g").unwrap();
            g.own_sender_bundle().unwrap()
        };
        bundle.sender_peer_key_hex = bob_hex.clone();
        bundle.signature = bob_id.sign(&sender_key_bundle_sign_bytes(&bundle));

        let receipt = gm_alice.handle_sender_key(&bundle, &alice_hex, &bob_id.public_key_bytes());
        assert_eq!(receipt.unwrap(), SenderKeyReceipt::NewMember);
        assert!(gm_alice.get_group("g").unwrap().verification_keys.contains_key(&bob_hex));
    }

    #[test]
    fn test_unsigned_bundle_rejected() {
        let alice_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let alice_hex = hex::encode(alice_id.public_key_bytes());
        let (mut bundle, bob_pub) = make_signed_bundle(&crate::crypto::IdentityKeypair::generate().unwrap(), "g");
        bundle.signature = Vec::new(); // strip signature

        let mut gm_alice = make_group_manager();
        gm_alice.create_group("g".to_string(), "G".to_string(), 1, alice_hex, &[]).unwrap();
        assert!(gm_alice.handle_sender_key(&bundle, "", &bob_pub).is_err());
    }

    #[test]
    fn test_wrong_signer_bundle_rejected() {
        let alice_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let alice_hex = hex::encode(alice_id.public_key_bytes());
        // Bundle signed by Mallory but claiming Bob as sender.
        let mallory_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let bob_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let (mut bundle, _) = make_signed_bundle(&mallory_id, "g");
        bundle.sender_peer_key_hex = hex::encode(bob_id.public_key_bytes());
        bundle.signature = mallory_id.sign(&sender_key_bundle_sign_bytes(&bundle));

        let mut gm_alice = make_group_manager();
        gm_alice.create_group("g".to_string(), "G".to_string(), 1, alice_hex, &[]).unwrap();
        // Claimed sender != transport peer → rejected.
        assert!(gm_alice.handle_sender_key(&bundle, "", &bob_id.public_key_bytes()).is_err());
        // Even if transport peer == claimed sender, the signature is under the
        // WRONG identity key → still rejected.
        assert!(gm_alice.handle_sender_key(&bundle, "", &mallory_id.public_key_bytes()).is_err());
    }

    #[test]
    fn test_private_key_material_bundle_rejected() {
        let alice_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let alice_hex = hex::encode(alice_id.public_key_bytes());
        let (mut bundle, bob_pub) = make_signed_bundle(&crate::crypto::IdentityKeypair::generate().unwrap(), "g");
        bundle.signing_key = Some(vec![0u8; 64]); // private key material

        let mut gm_alice = make_group_manager();
        gm_alice.create_group("g".to_string(), "G".to_string(), 1, alice_hex, &[]).unwrap();
        assert!(gm_alice.handle_sender_key(&bundle, "", &bob_pub).is_err());
    }

    #[test]
    fn test_join_group_generates_own_keys() {
        let alice_id = crate::crypto::IdentityKeypair::generate().unwrap();
        let alice_hex = hex::encode(alice_id.public_key_bytes());
        let mut gm_bob = make_group_manager();
        let b1 = gm_bob.join_group("g".to_string(), "G".to_string(), 1,
            "bob".to_string(), false, &[alice_hex.clone()]).unwrap();
        let b2 = gm_bob.join_group("g".to_string(), "G".to_string(), 2,
            "bob".to_string(), false, &[alice_hex]).unwrap();
        // Re-joining is idempotent: same chain key returned, no new keys.
        assert_eq!(b1.chain_key, b2.chain_key);
        assert!(b1.signing_key.is_none() && b2.signing_key.is_none());
        assert_eq!(gm_bob.get_group("g").unwrap().members.len(), 2); // bob + alice
    }
}
