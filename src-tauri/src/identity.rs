/// M2M — Identity Module
///
/// Manages long-term identity, invite creation/validation, and fingerprint display.
/// Identity is a single Ed25519 keypair generated on first launch.
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto::{self, IdentityKeypair};
use crate::protocol::{
    self, InvitePayload, SignedInvite, CLOCK_SKEW_TOLERANCE_SECS, INVITE_FLAG_LISTENER,
    INVITE_FLAG_ONE_TIME, INVITE_PREFIX, MAX_ADDRESS_HINT_LENGTH, MAX_INVITE_LENGTH,
    MAX_INVITE_VALIDITY_SECS, PROTOCOL_VERSION,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::CryptoError),
    #[error("protocol error: {0}")]
    Protocol(#[from] protocol::ProtocolError),
    #[error("invite expired")]
    InviteExpired,
    #[error("invite created in the future")]
    InviteFutureTimestamp,
    #[error("invite validity window too large")]
    InviteValidityTooLarge,
    #[error("invite signature invalid")]
    InviteSignatureInvalid,
    #[error("invite format invalid: {0}")]
    InviteFormatInvalid(String),
    #[allow(dead_code)]
    #[error("invite already consumed")]
    InviteAlreadyConsumed,
    #[error("address hint too long")]
    AddressHintTooLong,
}

/// Get the current Unix timestamp in seconds.
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
}

/// Create a signed invite link.
///
/// - `identity`: the local identity keypair (signs the invite)
/// - `address_hint`: IP:port or hostname:port where we're listening
/// - `validity_secs`: how long the invite is valid
/// - `one_time`: if true, invite can only be used once
pub fn create_invite(
    identity: &IdentityKeypair,
    address_hint: &str,
    validity_secs: u64,
    one_time: bool,
    candidates: Vec<crate::protocol::WireCandidate>,
) -> Result<String, IdentityError> {
    if address_hint.len() > MAX_ADDRESS_HINT_LENGTH {
        return Err(IdentityError::AddressHintTooLong);
    }
    if validity_secs > MAX_INVITE_VALIDITY_SECS {
        return Err(IdentityError::InviteValidityTooLarge);
    }

    let now = now_unix_secs();
    let nonce = crypto::random_bytes(16);

    let mut flags: u8 = INVITE_FLAG_LISTENER; // We are the listener
    if one_time {
        flags |= INVITE_FLAG_ONE_TIME;
    }

    let payload = InvitePayload {
        version: PROTOCOL_VERSION,
        identity_pub: identity.public_key_bytes(),
        address_hint: address_hint.to_string(),
        created_at: now,
        expires_at: now + validity_secs,
        nonce,
        flags,
        candidates,
    };

    // Serialize payload for signing
    let payload_bytes = protocol::serialize(&payload)?;

    // Sign the serialized payload with identity key
    let signature = identity.sign(&payload_bytes);

    let signed_invite = SignedInvite { payload, signature };

    // Serialize the full signed invite
    let invite_bytes = protocol::serialize(&signed_invite)?;

    // Encode as base64url and prepend protocol prefix
    let encoded = URL_SAFE_NO_PAD.encode(&invite_bytes);
    let invite_string = format!("{}{}", INVITE_PREFIX, encoded);

    if invite_string.len() > MAX_INVITE_LENGTH {
        return Err(IdentityError::InviteFormatInvalid(
            "serialized invite exceeds maximum length".to_string(),
        ));
    }

    Ok(invite_string)
}

/// Parse and validate an invite string.
/// Returns the validated invite data on success.
///
/// Validation steps:
/// 1. Decode base64url
/// 2. Deserialize MessagePack
/// 3. Check version
/// 4. Check expiry
/// 5. Check clock skew
/// 6. Check validity window
/// 7. Verify Ed25519 signature
/// 8. Validate address hint
pub fn validate_invite(invite_str: &str) -> Result<SignedInvite, IdentityError> {
    // Step 0: Check prefix
    let encoded = invite_str.strip_prefix(INVITE_PREFIX).ok_or_else(|| {
        IdentityError::InviteFormatInvalid("missing m2m:// prefix".to_string())
    })?;

    if invite_str.len() > MAX_INVITE_LENGTH {
        return Err(IdentityError::InviteFormatInvalid(
            "invite string too long".to_string(),
        ));
    }

    // Step 1: Decode base64url
    let invite_bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| IdentityError::InviteFormatInvalid(format!("base64 decode error: {e}")))?;

    // Step 2: Deserialize
    let signed: SignedInvite = protocol::deserialize(&invite_bytes)?;

    // Step 3: Check version
    if signed.payload.version != PROTOCOL_VERSION {
        return Err(IdentityError::InviteFormatInvalid(format!(
            "unsupported version: {:#04x}",
            signed.payload.version
        )));
    }

    let now = now_unix_secs();

    // Step 4: Check expiry
    if signed.payload.expires_at <= now {
        return Err(IdentityError::InviteExpired);
    }

    // Step 5: Check clock skew (reject if created too far in the future)
    if signed.payload.created_at > now + CLOCK_SKEW_TOLERANCE_SECS {
        return Err(IdentityError::InviteFutureTimestamp);
    }

    // Step 6: Check validity window
    let validity_window = signed
        .payload
        .expires_at
        .saturating_sub(signed.payload.created_at);
    if validity_window > MAX_INVITE_VALIDITY_SECS {
        return Err(IdentityError::InviteValidityTooLarge);
    }

    // Step 7: Verify signature
    let payload_bytes =
        protocol::serialize(&signed.payload)?;
    crypto::verify_signature(
        &signed.payload.identity_pub,
        &payload_bytes,
        &signed.signature,
    )
    .map_err(|_| IdentityError::InviteSignatureInvalid)?;

    // Step 8: Validate address hint
    if signed.payload.address_hint.is_empty() {
        return Err(IdentityError::InviteFormatInvalid(
            "empty address hint".to_string(),
        ));
    }
    if signed.payload.address_hint.len() > MAX_ADDRESS_HINT_LENGTH {
        return Err(IdentityError::AddressHintTooLong);
    }

    Ok(signed)
}

/// Check if an invite is one-time use.
pub fn is_one_time(invite: &SignedInvite) -> bool {
    invite.payload.flags & INVITE_FLAG_ONE_TIME != 0
}

/// Check if the inviter is the TCP listener.
#[allow(dead_code)]
pub fn is_listener(invite: &SignedInvite) -> bool {
    invite.payload.flags & INVITE_FLAG_LISTENER != 0
}
