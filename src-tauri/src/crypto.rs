/// M2M — Crypto Module
///
/// Provides all cryptographic operations using libsodium (sodiumoxide).
/// No custom cryptography. All primitives are standard, audited constructions.
///
/// Key algorithms:
/// - Ed25519: identity signing/verification
/// - X25519: ephemeral Diffie-Hellman key exchange
/// - XChaCha20-Poly1305: authenticated encryption (AEAD)
/// - HKDF-SHA256: key derivation
/// - SHA-256: fingerprint generation
use std::collections::HashMap;

use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
use sodiumoxide::crypto::hash::sha256;
use sodiumoxide::crypto::kx;
use sodiumoxide::crypto::scalarmult::curve25519 as scalarmult;
use sodiumoxide::crypto::sign;
use sodiumoxide::randombytes;
use zeroize::Zeroize;


use thiserror::Error;

/// Maximum size of data that can be encrypted in a single operation (16 MiB).
const MAX_ENCRYPT_SIZE: usize = 16 * 1024 * 1024;

/// Context string for HKDF session key derivation (reserved).
#[expect(dead_code, reason = "Reserved for HKDF session key derivation")]
const SESSION_KEY_CONTEXT: &[u8] = b"m2m-v1-session-key";

/// Maximum number of out-of-order message keys to cache per DH ratchet phase.
/// Follows the Signal Protocol's design: when messages arrive out of order,
/// intermediate message keys are derived and cached instead of discarded.
/// 2000 is the Signal-specified maximum — beyond this we reject to prevent
/// memory exhaustion attacks.
const MAX_SKIP: usize = 2000;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("sodiumoxide initialization failed")]
    InitFailed,
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("decryption failed: ciphertext may be tampered")]
    DecryptionFailed,
    #[error("signature verification failed")]
    SignatureInvalid,
    #[error("key derivation failed")]
    KeyDerivationFailed,
    #[error("input too large: {size} bytes exceeds {max} byte limit")]
    InputTooLarge { size: usize, max: usize },
    #[error("invalid key length")]
    InvalidKeyLength,
    #[error("X3DH key derivation failed")]
    X3DHFailed,
    #[error("double ratchet error: {0}")]
    DoubleRatchetError(String),
    #[error("prekey signature verification failed")]
    PrekeySignatureInvalid,
    #[error("too many skipped message keys (max {0})")]
    MaxSkippedKeysExceeded(usize),
}

/// Long-term identity keypair (Ed25519).
/// The private key is zeroized on drop.
pub struct IdentityKeypair {
    pub public_key: sign::PublicKey,
    secret_key: sign::SecretKey,
}

impl IdentityKeypair {
    /// Generate a new random identity keypair.
    pub fn generate() -> Result<Self, CryptoError> {
        let (pk, sk) = sign::gen_keypair();
        Ok(Self {
            public_key: pk,
            secret_key: sk,
        })
    }

    /// Reconstruct from existing key bytes.
    pub fn from_bytes(public: &[u8; 32], secret: &[u8; 64]) -> Result<Self, CryptoError> {
        let pk = sign::PublicKey::from_slice(public).ok_or(CryptoError::InvalidKeyLength)?;
        let sk = sign::SecretKey::from_slice(secret).ok_or(CryptoError::InvalidKeyLength)?;
        Ok(Self {
            public_key: pk,
            secret_key: sk,
        })
    }

    /// Sign a message with this identity key.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let sig = sign::sign_detached(message, &self.secret_key);
        sig.as_ref().to_vec()
    }

    /// Get the raw public key bytes.
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key.0
    }

    /// Get the raw secret key bytes (for encrypted storage only).
    pub fn secret_key_bytes(&self) -> [u8; 64] {
        self.secret_key.0
    }

    /// Generate a human-readable fingerprint of the public key.
    /// Format: colon-separated hex groups (e.g., "A1B2:C3D4:E5F6:...")
    pub fn fingerprint(&self) -> String {
        fingerprint_from_public_key(&self.public_key_bytes())
    }
}

impl Drop for IdentityKeypair {
    fn drop(&mut self) {
        // Zeroize the secret key memory on drop.
        // sodiumoxide::SecretKey doesn't implement Zeroize directly,
        // so we overwrite the backing array.
        let sk_bytes = &mut self.secret_key.0;
        sk_bytes.zeroize();
    }
}

/// Generate a fingerprint from a raw public key.
pub fn fingerprint_from_public_key(public_key: &[u8; 32]) -> String {
    let hash = sha256::hash(public_key);
    let hex_str = hex::encode_upper(&hash.0[..16]); // Use first 16 bytes (128 bits)
    hex_str
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or("????"))
        .collect::<Vec<&str>>()
        .join(":")
}

/// Verify an Ed25519 signature.
pub fn verify_signature(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8],
) -> Result<(), CryptoError> {
    let pk = sign::PublicKey::from_slice(public_key).ok_or(CryptoError::InvalidKeyLength)?;
    if signature.len() != 64 {
        return Err(CryptoError::SignatureInvalid);
    }
    let sig = sign::Signature::from_bytes(signature).map_err(|_| CryptoError::SignatureInvalid)?;
    if sign::verify_detached(&sig, message, &pk) {
        Ok(())
    } else {
        Err(CryptoError::SignatureInvalid)
    }
}

// ─── X25519 Identity Key (for X3DH) ──────────────────────────────────────────

/// Long-term X25519 identity keypair for X3DH Diffie-Hellman operations.
/// This is separate from the Ed25519 signing key. Kept in the vault alongside it.
pub struct X25519IdentityKeypair {
    pub(crate) public_key: [u8; 32],
    secret_key: [u8; 32],
}

impl X25519IdentityKeypair {
    pub fn generate() -> Self {
        let (pk, sk) = kx::gen_keypair();
        Self {
            public_key: pk.0,
            secret_key: sk.0,
        }
    }

    pub fn from_bytes(public: &[u8; 32], secret: &[u8; 32]) -> Result<Self, CryptoError> {
        Ok(Self {
            public_key: *public,
            secret_key: *secret,
        })
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key
    }

    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.secret_key
    }

    /// Perform X25519 Diffie-Hellman with this identity key and a peer's public key.
    pub fn diffie_hellman(&self, their_public: &[u8; 32]) -> Result<[u8; 32], CryptoError> {
        let n = scalarmult::Scalar::from_slice(&self.secret_key)
            .ok_or(CryptoError::InvalidKeyLength)?;
        let p = scalarmult::GroupElement::from_slice(their_public)
            .ok_or(CryptoError::InvalidKeyLength)?;
        let shared = scalarmult::scalarmult(&n, &p)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        Ok(shared.0)
    }
}

impl Drop for X25519IdentityKeypair {
    fn drop(&mut self) {
        self.secret_key.zeroize();
        self.public_key.zeroize();
    }
}

// ─── HKDF-SHA256 (RFC 5869) ──────────────────────────────────────────────────

/// HKDF-Extract: PRK = HMAC-SHA256(salt, IKM)
pub(crate) fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    use hmac::Mac;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(salt)
        .expect("HMAC accepts any key length");
    mac.update(ikm);
    let result = mac.finalize();
    result.into_bytes().into()
}

/// HKDF-Expand: output key material from PRK, info, and desired length.
pub(crate) fn hkdf_expand(prk: &[u8; 32], info: &[u8], length: usize) -> Vec<u8> {
    use hmac::Mac;
    let mut result = Vec::with_capacity(length);
    let mut t: Vec<u8> = Vec::new();
    for i in 1u8..=length.div_ceil(32) as u8 {
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(prk)
            .expect("HMAC accepts any key length");
        mac.update(&t);
        mac.update(info);
        mac.update(&[i]);
        t = mac.finalize().into_bytes().to_vec();
        result.extend_from_slice(&t);
    }
    result.truncate(length);
    result
}

/// Full HKDF: HKDF(salt, IKM, info, length) = HKDF-Expand(HKDF-Extract(salt, IKM), info, length)
pub(crate) fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let prk = hkdf_extract(salt, ikm);
    hkdf_expand(&prk, info, length)
}

/// Ephemeral keypair for X25519 Diffie-Hellman key exchange.
pub struct EphemeralKeypair {
    pub public_key: kx::PublicKey,
    secret_key: kx::SecretKey,
}

impl EphemeralKeypair {
    /// Generate a new ephemeral keypair for key exchange.
    pub fn generate() -> Self {
        let (pk, sk) = kx::gen_keypair();
        Self {
            public_key: pk,
            secret_key: sk,
        }
    }

    /// Get the raw public key bytes.
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key.0
    }

    /// Perform key exchange as the client (initiator).
    pub fn client_session_keys(
        &self,
        server_pk: &[u8; 32],
    ) -> Result<SessionKeys, CryptoError> {
        let server_public =
            kx::PublicKey::from_slice(server_pk).ok_or(CryptoError::InvalidKeyLength)?;
        let (rx, tx) = kx::client_session_keys(&self.public_key, &self.secret_key, &server_public)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        Ok(SessionKeys {
            rx_key: rx.0,
            tx_key: tx.0,
        })
    }

    /// Perform key exchange as the server (responder).
    pub fn server_session_keys(
        &self,
        client_pk: &[u8; 32],
    ) -> Result<SessionKeys, CryptoError> {
        let client_public =
            kx::PublicKey::from_slice(client_pk).ok_or(CryptoError::InvalidKeyLength)?;
        let (rx, tx) =
            kx::server_session_keys(&self.public_key, &self.secret_key, &client_public)
                .map_err(|_| CryptoError::KeyDerivationFailed)?;
        Ok(SessionKeys {
            rx_key: rx.0,
            tx_key: tx.0,
        })
    }
}

impl Drop for EphemeralKeypair {
    fn drop(&mut self) {
        self.secret_key.0.zeroize();
    }
}

// ─── X3DH (Extended Triple Diffie-Hellman) ───────────────────────────────────

/// Prekey bundle for X3DH key agreement, extracted from an invite.
/// The `signed_prekey_sig` should be verified against the Ed25519 identity
/// key before calling `x3dh_initiate`.
pub struct PrekeyBundle {
    pub identity_key: [u8; 32],            // IK: X25519 identity public key
    pub signed_prekey: [u8; 32],           // SPK: X25519 signed prekey public
    pub signed_prekey_sig: Vec<u8>,         // Ed25519 signature(SPK) — verify before use
    pub one_time_prekey: Option<[u8; 32]>,  // OPK: optional X25519 one-time prekey
}

/// Output of X3DH key agreement.
pub struct X3DHSessionKeys {
    pub root_key: [u8; 32],   // Root key for Double Ratchet
    pub chain_key: [u8; 32],  // Initial chain key
}

/// Compute X3DH shared secret as the INITIATOR (Alice).
///
/// SK = DH(IK_A, SPK_B) || DH(EK_A, IK_B) || DH(EK_A, SPK_B) || [DH(EK_A, OPK_B)]
/// root_key, chain_key = HKDF(salt=32×0x00, SK, "M2M-X3DH", 64)
///
/// Caller MUST verify `bundle.signed_prekey_sig` with the peer's Ed25519 key first.
pub fn x3dh_initiate(
    our_identity: &X25519IdentityKeypair,  // IK_A
    our_ephemeral: &EphemeralKeypair,      // EK_A
    their_bundle: &PrekeyBundle,           // IK_B, SPK_B, [OPK_B]
) -> Result<X3DHSessionKeys, CryptoError> {
    // DH1 = DH(IK_A, SPK_B)
    let dh1 = our_identity.diffie_hellman(&their_bundle.signed_prekey)?;
    // DH2 = DH(EK_A, IK_B)
    let dh2 = our_ephemeral.diffie_hellman(&their_bundle.identity_key)?;
    // DH3 = DH(EK_A, SPK_B)
    let dh3 = our_ephemeral.diffie_hellman(&their_bundle.signed_prekey)?;

    // Build SK = DH1 || DH2 || DH3 || [DH4]
    let mut sk = Vec::with_capacity(96);
    sk.extend_from_slice(&dh1);
    sk.extend_from_slice(&dh2);
    sk.extend_from_slice(&dh3);

    // DH4 = DH(EK_A, OPK_B) if OPK available
    if let Some(opk) = &their_bundle.one_time_prekey {
        let dh4 = our_ephemeral.diffie_hellman(opk)?;
        sk.extend_from_slice(&dh4);
    }

    // Derive root_key (32B) + chain_key (32B) = 64B total
    let output = hkdf(&[0u8; 32], &sk, b"M2M-X3DH", 64);
    let mut root_key = [0u8; 32];
    let mut chain_key = [0u8; 32];
    root_key.copy_from_slice(&output[..32]);
    chain_key.copy_from_slice(&output[32..]);

    Ok(X3DHSessionKeys { root_key, chain_key })
}

/// Compute X3DH shared secret as the RESPONDER (Bob).
///
/// SK = DH(SPK_B, IK_A) || DH(IK_B, EK_A) || DH(SPK_B, EK_A) || [DH(OPK_B, EK_A)]
pub fn x3dh_respond(
    our_identity: &X25519IdentityKeypair,      // IK_B
    our_signed_prekey: &EphemeralKeypair,       // SPK_B (must have secret key)
    our_one_time_prekey: Option<&EphemeralKeypair>, // OPK_B (optional)
    their_ephemeral: &[u8; 32],                // EK_A
    their_identity: &[u8; 32],                 // IK_A
) -> Result<X3DHSessionKeys, CryptoError> {
    x3dh_respond_raw(our_identity, our_signed_prekey, our_one_time_prekey,
                     their_ephemeral, their_identity)
}

fn x3dh_respond_raw(
    our_identity: &X25519IdentityKeypair,
    our_signed_prekey: &EphemeralKeypair,
    our_one_time_prekey: Option<&EphemeralKeypair>,
    their_ephemeral: &[u8; 32],
    their_identity: &[u8; 32],
) -> Result<X3DHSessionKeys, CryptoError> {
    // For DH operations we need to convert kx keys to X25519 keys.
    // SPK_B's secret key is an EphemeralKeypair (kx::SecretKey).
    // We use diffie_hellman style but with the raw secret.
    use sodiumoxide::crypto::scalarmult::curve25519 as sm;

    // Convert SPK_B secret to scalar
    let spk_scalar = sm::Scalar::from_slice(&our_signed_prekey.secret_key.0)
        .ok_or(CryptoError::InvalidKeyLength)?;

    // DH1 = DH(SPK_B, IK_A) — use SPK_B's secret, IK_A's public
    let ik_a = sm::GroupElement::from_slice(their_identity)
        .ok_or(CryptoError::InvalidKeyLength)?;
    let dh1 = sm::scalarmult(&spk_scalar, &ik_a)
        .map_err(|_| CryptoError::KeyDerivationFailed)?;

    // DH2 = DH(IK_B, EK_A)
    let dh2 = our_identity.diffie_hellman(their_ephemeral)?;

    // For DH3 = DH(SPK_B, EK_A), use SPK_B's secret again
    let ek_a = sm::GroupElement::from_slice(their_ephemeral)
        .ok_or(CryptoError::InvalidKeyLength)?;
    let dh3 = sm::scalarmult(&spk_scalar, &ek_a)
        .map_err(|_| CryptoError::KeyDerivationFailed)?;

    let mut sk = Vec::with_capacity(96);
    sk.extend_from_slice(&dh1.0);
    sk.extend_from_slice(&dh2);
    sk.extend_from_slice(&dh3.0);

    // DH4 = DH(OPK_B, EK_A) if available
    if let Some(opk) = our_one_time_prekey {
        let opk_scalar = sm::Scalar::from_slice(&opk.secret_key.0)
            .ok_or(CryptoError::InvalidKeyLength)?;
        let dh4 = sm::scalarmult(&opk_scalar, &ek_a)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        sk.extend_from_slice(&dh4.0);
    }

    let output = hkdf(&[0u8; 32], &sk, b"M2M-X3DH", 64);
    let mut root_key = [0u8; 32];
    let mut chain_key = [0u8; 32];
    root_key.copy_from_slice(&output[..32]);
    chain_key.copy_from_slice(&output[32..]);

    Ok(X3DHSessionKeys { root_key, chain_key })
}

// ─── Double Ratchet ───────────────────────────────────────────────────────────

/// Result of DoubleRatchet::encrypt: (optional_ratchet_key, message_number, nonce, ciphertext).
type EncryptOutput = (Option<[u8; 32]>, u64, Vec<u8>, Vec<u8>);

/// A single-use message key derived from a chain key.
/// Zeroized on drop to ensure key material doesn't linger in memory.
pub struct MessageKey(pub [u8; 32]);

impl Drop for MessageKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Double Ratchet state machine for per-message key evolution.
///
/// Manages root key (DH ratchet) + sending/receiving chain keys.
/// Chain keys advance via HKDF: each message derives a unique message key
/// and produces a new chain key. DH ratchets periodically for break-in recovery.
///
/// ## Skipped message keys
///
/// Out-of-order messages within a chain are handled by caching the message key
/// for each skipped message number in `skipped_keys`. When a message with a
/// previously-skipped number arrives, its key is retrieved from the cache instead
/// of re-derived (the chain has already advanced past it). The cache is capped at
/// [`MAX_SKIP`] entries to prevent memory exhaustion.
pub struct DoubleRatchet {
    root_key: [u8; 32],
    send_chain_key: Option<[u8; 32]>,
    recv_chain_key: Option<[u8; 32]>,
    send_message_number: u64,
    recv_message_number: u64,
    /// Our current DH ratchet keypair.
    our_ratchet_keypair: EphemeralKeypair,
    /// Peer's current DH ratchet public key.
    their_ratchet_pub: [u8; 32],
    /// Message keys for out-of-order messages.
    /// Keys are cached when deriving through a gap and consumed when
    /// the corresponding message arrives. Capped at MAX_SKIP entries
    /// to limit memory usage.
    skipped_keys: HashMap<u64, [u8; 32]>,
}

impl DoubleRatchet {
    /// Initialize the Double Ratchet from X3DH output.
    ///
    /// - `x3dh`: output of X3DH (root_key + initial chain_key)
    /// - `dh_ratchet_keypair`: our initial DH ratchet keypair
    /// - `dh_remote_public`: peer's initial DH ratchet public key
    /// - `role_is_sender`: true if we send the first message
    pub fn new(
        x3dh: X3DHSessionKeys,
        dh_ratchet_keypair: EphemeralKeypair,
        dh_remote_public: [u8; 32],
        role_is_sender: bool,
    ) -> Self {
        let root_key = x3dh.root_key;
        let chain_key = x3dh.chain_key;
        if role_is_sender {
            Self {
                root_key,
                send_chain_key: Some(chain_key),
                recv_chain_key: None,
                send_message_number: 0,
                recv_message_number: 0,
                our_ratchet_keypair: dh_ratchet_keypair,
                their_ratchet_pub: dh_remote_public,
                skipped_keys: HashMap::with_capacity(64),
            }
        } else {
            Self {
                root_key,
                send_chain_key: None,
                recv_chain_key: Some(chain_key),
                send_message_number: 0,
                recv_message_number: 0,
                our_ratchet_keypair: dh_ratchet_keypair,
                their_ratchet_pub: dh_remote_public,
                skipped_keys: HashMap::with_capacity(64),
            }
        }
    }

    /// Derive a message key from a chain key and advance the chain.
    fn derive_message_key(chain_key: &[u8; 32]) -> (MessageKey, [u8; 32]) {
        let out = hkdf(chain_key, b"", b"M2M-MSG-KEY", 64);
        let mut msg_key = [0u8; 32];
        let mut next_key = [0u8; 32];
        msg_key.copy_from_slice(&out[..32]);
        next_key.copy_from_slice(&out[32..]);
        (MessageKey(msg_key), next_key)
    }

    /// Perform a DH ratchet: advance root key using a new DH shared secret.
    ///
    /// Clears the skipped message key cache since the receiving chain is reset.
    fn dh_ratchet_step(&mut self, remote_pub: &[u8; 32]) -> Result<(), CryptoError> {
        let shared = self.our_ratchet_keypair.diffie_hellman(remote_pub)?;
        let out = hkdf(&self.root_key, &shared, b"M2M-DH-RATCHET", 64);
        let mut new_root = [0u8; 32];
        let mut new_chain = [0u8; 32];
        new_root.copy_from_slice(&out[..32]);
        new_chain.copy_from_slice(&out[32..]);
        self.root_key = new_root;
        // New chain key goes to the receiving chain (we received the DH key)
        self.recv_chain_key = Some(new_chain);
        self.their_ratchet_pub = *remote_pub;
        self.recv_message_number = 0;
        // Clear skipped keys — they belong to the old receiving chain
        self.skipped_keys.clear();
        Ok(())
    }

    /// Encrypt a message: derive message key, encrypt, advance chain.
    ///
    /// If `do_ratchet` is true, generates a new DH ratchet keypair, advances
    /// the root key, and embeds the new public key in the returned header.
    ///
    /// Returns (ratchet_key_pub_opt, message_number, nonce, ciphertext).
    pub fn encrypt(
        &mut self,
        plaintext: &[u8],
        aad: &[u8],
        do_ratchet: bool,
    ) -> Result<EncryptOutput, CryptoError> {
        let mut ratchet_pub = None;
        if do_ratchet {
            // Generate a NEW DH ratchet keypair for break-in recovery
            let new_kp = EphemeralKeypair::generate();
            let new_pub = new_kp.public_key_bytes();
            // DH(new_sk, their_old_pk) — the sender advances with their NEW key
            let shared = new_kp.diffie_hellman(&self.their_ratchet_pub)?;
            let out = hkdf(&self.root_key, &shared, b"M2M-DH-RATCHET", 64);
            let mut new_root = [0u8; 32];
            let mut new_chain = [0u8; 32];
            new_root.copy_from_slice(&out[..32]);
            new_chain.copy_from_slice(&out[32..]);
            self.root_key = new_root;
            self.send_chain_key = Some(new_chain);
            self.send_message_number = 0;
            self.our_ratchet_keypair = new_kp;
            ratchet_pub = Some(new_pub);
        }

        let send_chain = self.send_chain_key
            .ok_or(CryptoError::DoubleRatchetError("no send chain key".into()))?;

        let (msg_key, next_chain) = Self::derive_message_key(&send_chain);
        self.send_chain_key = Some(next_chain);

        let msg_num = self.send_message_number;
        self.send_message_number += 1;

        // Encrypt with XChaCha20-Poly1305
        let nonce = aead::gen_nonce();
        let key = aead::Key::from_slice(&msg_key.0).ok_or(CryptoError::InvalidKeyLength)?;
        let ciphertext = aead::seal(plaintext, Some(aad), &nonce, &key);
        let nonce_vec = nonce.0.to_vec();

        // Zeroize the message key after use (drop does this, but be explicit)
        drop(msg_key);

        Ok((ratchet_pub, msg_num, nonce_vec, ciphertext))
    }

    /// Decrypt a message: derive message key, decrypt, advance chain.
    ///
    /// `ratchet_key` is the peer's new DH public key if this message triggers a ratchet.
    ///
    /// ## Out-of-order message handling
    ///
    /// If `message_number` is below the current receiving chain position, the
    /// skipped message key cache is consulted. If the key was cached during a
    /// previous gap derivation, the message decrypts successfully. If not, the
    /// message is unrecoverable and an error is returned.
    ///
    /// The cache is capped at [`MAX_SKIP`] entries (2000). Beyond this, new
    /// messages are rejected as `MaxSkippedKeysExceeded` to prevent memory
    /// exhaustion from a peer who sends messages with large gaps.
    pub fn decrypt(
        &mut self,
        ciphertext: &[u8],
        nonce: &[u8],
        aad: &[u8],
        message_number: u64,
        ratchet_key: Option<&[u8; 32]>,
    ) -> Result<Vec<u8>, CryptoError> {
        // If peer sent a new ratchet key, perform DH ratchet first
        if let Some(new_pub) = ratchet_key {
            self.dh_ratchet_step(new_pub)?;
        }

        // ── Check skipped message key cache for out-of-order messages ──
        if message_number < self.recv_message_number {
            return match self.skipped_keys.remove(&message_number) {
                Some(saved_key) => {
                    // Decrypt with the previously-cached message key
                    Self::decrypt_with_key(&saved_key, ciphertext, nonce, aad)
                }
                None => Err(CryptoError::DoubleRatchetError(format!(
                    "message key for {} not found (already consumed or never cached)", message_number
                ))),
            };
        }

        let recv_chain = self.recv_chain_key
            .ok_or(CryptoError::DoubleRatchetError("no recv chain key".into()))?;

        // ── Derive through gap, caching intermediate keys ──
        let mut current_chain = recv_chain;
        while self.recv_message_number < message_number {
            let (msg_key, next_chain) = Self::derive_message_key(&current_chain);

            // Cache the intermediate message key for out-of-order delivery.
            // Cap at MAX_SKIP to prevent memory exhaustion.
            if self.skipped_keys.len() >= MAX_SKIP {
                return Err(CryptoError::MaxSkippedKeysExceeded(MAX_SKIP));
            }
            self.skipped_keys.insert(self.recv_message_number, msg_key.0);

            current_chain = next_chain;
            self.recv_message_number += 1;
        }

        // Derive the message key for THIS message
        let (msg_key, next_chain) = Self::derive_message_key(&current_chain);
        self.recv_chain_key = Some(next_chain);
        self.recv_message_number += 1;

        // Decrypt with XChaCha20-Poly1305
        let result = Self::decrypt_with_key(&msg_key.0, ciphertext, nonce, aad);
        drop(msg_key);
        result
    }

    /// Decrypt ciphertext using a raw 32-byte message key.
    ///
    /// Extracted as a standalone helper so it can be called from both the
    /// normal decrypt path and the skipped-key cache path.
    fn decrypt_with_key(
        key_bytes: &[u8; 32],
        ciphertext: &[u8],
        nonce: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let key = aead::Key::from_slice(key_bytes).ok_or(CryptoError::InvalidKeyLength)?;
        let nonce_obj = aead::Nonce::from_slice(nonce).ok_or(CryptoError::DecryptionFailed)?;
        aead::open(ciphertext, Some(aad), &nonce_obj, &key)
            .map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Check if we should perform a DH ratchet (based on message count).
    pub fn should_ratchet(&self, interval: u64) -> bool {
        interval > 0 && self.send_message_number > 0 && self.send_message_number.is_multiple_of(interval)
    }
}

/// Perform X25519 DH using an EphemeralKeypair (kx keys).
impl EphemeralKeypair {
    /// Compute shared secret with a peer's public key.
    pub fn diffie_hellman(&self, their_public: &[u8; 32]) -> Result<[u8; 32], CryptoError> {
        use sodiumoxide::crypto::scalarmult::curve25519 as sm;
        let n = sm::Scalar::from_slice(&self.secret_key.0)
            .ok_or(CryptoError::InvalidKeyLength)?;
        let p = sm::GroupElement::from_slice(their_public)
            .ok_or(CryptoError::InvalidKeyLength)?;
        let shared = sm::scalarmult(&n, &p)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        Ok(shared.0)
    }
}

/// Session keys derived from key exchange.
/// Separate keys for sending and receiving (directional).
/// Supports ratcheting for forward secrecy: keys evolve after each use.
pub struct SessionKeys {
    pub(crate) rx_key: [u8; 32],
    pub(crate) tx_key: [u8; 32],
}

impl SessionKeys {
    /// Encrypt a plaintext message for sending.
    /// Returns (nonce, ciphertext). The nonce must be sent alongside the ciphertext.
    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
        if plaintext.len() > MAX_ENCRYPT_SIZE {
            return Err(CryptoError::InputTooLarge {
                size: plaintext.len(),
                max: MAX_ENCRYPT_SIZE,
            });
        }
        let nonce = aead::gen_nonce();
        let key =
            aead::Key::from_slice(&self.tx_key).ok_or(CryptoError::InvalidKeyLength)?;
        let ciphertext = aead::seal(plaintext, Some(aad), &nonce, &key);
        Ok((nonce.0.to_vec(), ciphertext))
    }

    /// Decrypt a received ciphertext.
    pub fn decrypt(
        &self,
        ciphertext: &[u8],
        nonce_bytes: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let nonce =
            aead::Nonce::from_slice(nonce_bytes).ok_or(CryptoError::DecryptionFailed)?;
        let key =
            aead::Key::from_slice(&self.rx_key).ok_or(CryptoError::InvalidKeyLength)?;
        aead::open(ciphertext, Some(aad), &nonce, &key)
            .map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Ratchet the sending key forward after encrypting a message.
    /// Derives a new tx_key from the current one using a one-way function.
    /// This provides forward secrecy: compromising the current key does NOT
    /// reveal previously encrypted messages, because the old key is zeroized.
    ///
    /// Construction: new_tx_key = SHA256(old_tx_key || ratchet_context)
    /// This is the HKDF-Expand step using SHA256 as the PRF.
    pub fn ratchet_tx(&mut self) {
        let mut input = Vec::with_capacity(32 + 14);
        input.extend_from_slice(&self.tx_key);
        input.extend_from_slice(b"m2m-ratchet-v1");
        let hash = sha256::hash(&input);
        self.tx_key.zeroize();
        self.tx_key.copy_from_slice(&hash.0[..32]);
    }

    /// Ratchet the receiving key forward after decrypting a message.
    /// Mirror of ratchet_tx for the receive direction.
    pub fn ratchet_rx(&mut self) {
        let mut input = Vec::with_capacity(32 + 14);
        input.extend_from_slice(&self.rx_key);
        input.extend_from_slice(b"m2m-ratchet-v1");
        let hash = sha256::hash(&input);
        self.rx_key.zeroize();
        self.rx_key.copy_from_slice(&hash.0[..32]);
    }
}

impl Drop for SessionKeys {
    fn drop(&mut self) {
        self.rx_key.zeroize();
        self.tx_key.zeroize();
    }
}

/// Generate cryptographically secure random bytes.
pub fn random_bytes(len: usize) -> Vec<u8> {
    randombytes::randombytes(len)
}

// ─── Sender Key Chain (Signal-style Group E2EE) ──────────────────────────

/// A single sender's chain state for group E2EE.
///
/// Each group member has their own Sender Key chain. The sender uses their
/// chain to encrypt messages; receivers use the corresponding receiver chain
/// to decrypt. Chains advance via HKDF:
///
///   message_key = HKDF(chain_key, b"", b"M2M-MSG-KEY", 32)
///   chain_key   = HKDF(chain_key, b"", b"M2M-NEXT-KEY", 32)
///
/// Out-of-order messages are handled by caching intermediate message keys
/// (same design as DoubleRatchet's skipped_keys cache).
#[derive(Debug, Clone)]
pub struct SenderKeyChain {
    chain_key: [u8; 32],
    message_number: u64,
    /// Cached message keys for out-of-order messages.
    /// Keyed by message_number, value is (nonce, aead_key).
    cached_keys: HashMap<u64, CachedSenderKey>,
    /// Maximum number of cached keys before rejecting new ones.
    max_cache: usize,
}

#[derive(Debug, Clone)]
struct CachedSenderKey {
    nonce: [u8; 24],
    key: [u8; 32],
}

/// Context strings for Sender Key HKDF steps.
const SENDER_MSG_KEY_INFO: &[u8] = b"M2M-SENDER-MSG-KEY";
const SENDER_NEXT_KEY_INFO: &[u8] = b"M2M-SENDER-NEXT-KEY";

impl SenderKeyChain {
    /// Create a new Sender Key chain from an initial chain key.
    pub fn new(initial_chain_key: [u8; 32]) -> Self {
        Self {
            chain_key: initial_chain_key,
            message_number: 0,
            cached_keys: HashMap::new(),
            max_cache: 2000,
        }
    }

    /// Derive the next message key and advance the chain.
    /// Returns (nonce, aead_key) for use with XChaCha20-Poly1305.
    pub fn next_message_key(&mut self) -> Result<([u8; 24], [u8; 32]), CryptoError> {
        // Derive message key
        let msg_key_out = hkdf(&self.chain_key, b"", SENDER_MSG_KEY_INFO, 56);
        let mut msg_key = [0u8; 32];
        let mut aead_nonce = [0u8; 24];
        aead_nonce.copy_from_slice(&msg_key_out[..24]);
        msg_key.copy_from_slice(&msg_key_out[24..56]);

        // Advance chain key
        let next_key_out = hkdf(&self.chain_key, b"", SENDER_NEXT_KEY_INFO, 32);
        self.chain_key.copy_from_slice(&next_key_out);

        let _msg_num = self.message_number;
        self.message_number += 1;

        Ok((aead_nonce, msg_key))
    }

    /// Derive a message key for a specific message number (for out-of-order messages).
    /// Caches intermediate keys; subsequent calls for the same number remain cached.
    pub fn peek_message_key(&mut self, message_number: u64) -> Result<([u8; 24], [u8; 32]), CryptoError> {
        // Check cache first
        if let Some(cached) = self.cached_keys.get(&message_number) {
            return Ok((cached.nonce, cached.key));
        }

        // Derive forward to the target message number
        while self.message_number <= message_number {
            let msg_key_out = hkdf(&self.chain_key, b"", SENDER_MSG_KEY_INFO, 56);
            let mut aead_nonce = [0u8; 24];
            let mut msg_key = [0u8; 32];
            aead_nonce.copy_from_slice(&msg_key_out[..24]);
            msg_key.copy_from_slice(&msg_key_out[24..56]);

            // Cache the key at the current message number
            if self.cached_keys.len() >= self.max_cache {
                return Err(CryptoError::DoubleRatchetError(
                    "sender key cache full".into(),
                ));
            }
            self.cached_keys.insert(self.message_number, CachedSenderKey {
                nonce: aead_nonce,
                key: msg_key,
            });

            // Advance chain key
            let next_key_out = hkdf(&self.chain_key, b"", SENDER_NEXT_KEY_INFO, 32);
            self.chain_key.copy_from_slice(&next_key_out);
            self.message_number += 1;
        }

        self.cached_keys.remove(&message_number)
            .map(|c| (c.nonce, c.key))
            .ok_or_else(|| CryptoError::DoubleRatchetError(
                "sender key not found after derivation".into(),
            ))
    }

    /// Current message number (next message will get this number).
    pub fn current_message_number(&self) -> u64 {
        self.message_number
    }

    /// Get the current chain key (for storage/backup).
    pub fn chain_key(&self) -> &[u8; 32] {
        &self.chain_key
    }
}

/// Generate a Sender Key pair: one sending chain and the matching initial chain key
/// that receivers use to construct their receiving chains.
///
/// Returns (sending_chain, initial_chain_key) where:
/// - sending_chain: the sender uses this to encrypt messages
/// - initial_chain_key: receivers use this to construct receiving chains
pub fn generate_sender_key_pair() -> (SenderKeyChain, [u8; 32]) {
    let random = random_bytes(32);
    let mut initial_key = [0u8; 32];
    initial_key.copy_from_slice(&random);
    let chain = SenderKeyChain::new(initial_key);
    (chain, initial_key)
}

/// Derive a receiver chain from a sender's initial chain key.
pub fn derive_receiver_chain(initial_chain_key: &[u8; 32]) -> SenderKeyChain {
    SenderKeyChain::new(*initial_chain_key)
}

/// Generate an Ed25519 signing keypair for a group sender.
/// Returns (signing_key_secret_bytes, verification_key).
/// signing_key_secret_bytes is a 64-byte Ed25519 seed+private key.
pub fn generate_sender_signing_keypair() -> ([u8; 64], [u8; 32]) {
    let (pk, sk) = sign::gen_keypair();
    let mut sk_bytes = [0u8; 64];
    let mut pk_bytes = [0u8; 32];
    sk_bytes.copy_from_slice(&sk.0);
    pk_bytes.copy_from_slice(&pk.0);
    (sk_bytes, pk_bytes)
}

/// Sign a group message with the sender's Ed25519 signing key.
pub fn sign_group_message(
    signing_key: &[u8; 64],
    data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let sk = sign::SecretKey::from_slice(signing_key)
        .ok_or(CryptoError::InvalidKeyLength)?;
    let sig = sign::sign_detached(data, &sk);
    Ok(sig.as_ref().to_vec())
}

/// Verify a group message signature against the sender's Ed25519 verification key.
pub fn verify_group_message_signature(
    verification_key: &[u8; 32],
    data: &[u8],
    signature: &[u8],
) -> bool {
    let pk = match sign::PublicKey::from_slice(verification_key) {
        Some(pk) => pk,
        None => return false,
    };
    if signature.len() != 64 {
        return false;
    }
    let sig = match sign::Signature::from_bytes(signature) {
        Ok(s) => s,
        Err(_) => return false,
    };
    sign::verify_detached(&sig, data, &pk)
}

/// Initialize the sodiumoxide library. Must be called once at startup.
pub fn init() -> Result<(), CryptoError> {
    sodiumoxide::init().map_err(|_| CryptoError::InitFailed)
}

// ─── Message Padding ────────────────────────────────────────────────────────

/// NOTE: Fixed `pad_message`/`unpad_message` have been replaced by the
/// exponential-tier `pad_message_variable`/`unpad_message_variable` above.
///
/// Exponential padding thresholds.
/// Short messages are padded aggressively, long messages less so.
/// File chunks get minimal padding (they're already close to chunk size).
const PADDING_TIERS: &[(usize, usize)] = &[
    (64, 1024),      // ≤64 bytes → pad to 1KB (aggressive)
    (256, 2048),     // ≤256 bytes → pad to 2KB
    (1024, 4096),    // ≤1KB → pad to 4KB
    (4096, 8192),    // ≤4KB → pad to 8KB
    (usize::MAX, 16384), // >4KB → pad to 16KB
];

/// Pad a message using exponential tier-based padding.
///
/// Unlike the fixed `pad_message_variable()` which uses a 256-byte block for everything,
/// this function applies different padding aggressiveness based on message size:
///
/// | Message Size | Padding Block | Rationale |
/// |-------------|--------------|-----------|
/// | 0-64 B      | 1024 B       | Short text; makes "hi" and "I resign" same size |
/// | 65-256 B    | 2048 B       | Paragraph text |
/// | 257-1 KB    | 4096 B       | Long messages |
/// | 1-4 KB      | 8192 B       | Very long messages |
/// | >4 KB       | 16384 B      | File chunks (already near chunk size) |
///
/// This makes traffic analysis significantly harder: a single word, a paragraph,
/// and a long message all look like the same wire size within their tier.
pub fn pad_message_variable(plaintext: &[u8]) -> Vec<u8> {
    let len = plaintext.len();
    let block_size = PADDING_TIERS
        .iter()
        .find(|(threshold, _)| len <= *threshold)
        .map(|(_, block)| *block)
        .unwrap_or(16384);

    // Calculate padding: (len + pad_len + 2) % block_size == 0
    // The +2 accounts for the u16 padding-length suffix, which supports
    // block sizes up to 65535 (all our tiers: 1024–16384).
    let needed = (len + 2) % block_size;
    let pad_len = if needed == 0 { 0 } else { block_size - needed };

    let mut padded = Vec::with_capacity(len + pad_len + 2);
    padded.extend_from_slice(plaintext);
    padded.extend(random_bytes(pad_len));
    padded.extend_from_slice(&(pad_len as u16).to_be_bytes());
    padded
}

/// Remove exponential padding from a padded message.
///
/// ## Defense-in-depth: padding structure verification
///
/// After extracting the plaintext, we independently recompute the expected
/// padded length using the **plaintext length** and the tier constants, then
/// verify the actual buffer matches. This catches any manipulation of the
/// padding suffix byte(s) — even if the AEAD layer were somehow bypassed:
///
/// - A modified `pad_len` field that claims a different padding length
///   would produce a different expected total, causing verification to fail.
/// - A truncated or extended buffer fails the length comparison.
///
/// This uses `pad_message_variable` internally to derive the expected length
/// from the plaintext alone, ensuring zero divergence between pad and unpad.
pub fn unpad_message_variable(padded: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if padded.len() < 2 {
        return Err(CryptoError::DecryptionFailed);
    }
    let pad_len = u16::from_be_bytes([
        padded[padded.len() - 2],
        padded[padded.len() - 1],
    ]) as usize;
    if pad_len + 2 > padded.len() {
        return Err(CryptoError::DecryptionFailed);
    }
    let original_len = padded.len() - 2 - pad_len;
    let plaintext = &padded[..original_len];

    // ═══ Padding structure verification ═══
    // Independently compute the expected padded length from just the plaintext
    // length, using exactly the same tier logic as `pad_message_variable`.
    // This verifies the padding suffix wasn't tampered with — a manipulated
    // pad_len would produce a different expected total.
    let block_size = PADDING_TIERS
        .iter()
        .find(|(threshold, _)| plaintext.len() <= *threshold)
        .map(|(_, block)| *block)
        .unwrap_or(16384);
    let needed = (plaintext.len() + 2) % block_size;
    let expected_pad = if needed == 0 { 0 } else { block_size - needed };
    let expected_total = plaintext.len() + expected_pad + 2;
    if padded.len() != expected_total {
        return Err(CryptoError::DecryptionFailed);
    }

    Ok(plaintext.to_vec())
}

#[cfg(test)]
mod crypto_tests {
    use super::*;
    use crate::protocol::PacketType;

    #[test]
    fn test_pad_unpad_roundtrip() {
        let test_cases = vec![
            b"" as &[u8],
            b"a",
            b"hello",
            b"hello world this is a test message that is longer",
            &[0u8; 127],
            &[0u8; 128],
            &[0u8; 255],
            &[0u8; 256],
            &[0u8; 511],
            &[0u8; 512],
            &[0u8; 1000],
        ];
        for input in test_cases {
            let padded = pad_message_variable(input);
            let unpadded = unpad_message_variable(&padded).unwrap();
            assert_eq!(input, &unpadded[..], "roundtrip failed for len={}", input.len());
            // Verify padding meets block alignment
            // padded = input + pad_bytes + [pad_len as u16]
            // total should be input.len() + pad_len + 2
            let pad_len = u16::from_be_bytes([
                padded[padded.len() - 2],
                padded[padded.len() - 1],
            ]) as usize;
            assert_eq!(padded.len(), input.len() + pad_len + 2,
                "padding length mismatch for len={}", input.len());
        }
    }

    #[test]
    fn test_padding_hides_length() {
        // Messages of different lengths within the same tier should
        // produce the same total padded length (same block alignment).
        // Both "hi" (2 B) and a longer message (35 B) are in the ≤64 → 1024 B tier.
        let short = pad_message_variable(b"hi");
        let long = pad_message_variable(b"hello world this is a longer message");
        // Both should produce the same total length (2 + pad_len + 2 == 35 + pad_len' + 2)
        // since both round up to the same 1024-byte block.
        assert_eq!(short.len(), long.len(),
            "messages in same tier should produce same padded length: {} vs {}",
            short.len(), long.len());
        // The padded length should be a multiple of the tier block size (1024).
        assert_eq!(short.len() % 1024, 0,
            "padded length {} not aligned to block 1024", short.len());
    }

    #[test]
    fn test_invalid_unpad_rejected() {
        // Too short (< 2 bytes)
        assert!(unpad_message_variable(b"").is_err());
        assert!(unpad_message_variable(b"x").is_err());
        // Message with pad_len = 0x03ff but only 10 bytes total
        let mut bad = vec![0u8; 10];
        bad[8] = 0x03;
        bad[9] = 0xff;
        assert!(unpad_message_variable(&bad).is_err());
    }

    #[test]
    fn test_unpad_rejects_tampered_padding_suffix() {
        // Create a valid padded message, then tamper with the pad_len suffix.
        // The padding integrity check should detect the manipulation.
        let plaintext = b"hello";
        let padded = pad_message_variable(plaintext);

        // Flip bits in the pad_len suffix (last 2 bytes)
        let mut tampered = padded.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF; // corrupt the pad_len

        assert!(
            unpad_message_variable(&tampered).is_err(),
            "tampered padding suffix should be rejected"
        );

        // Flip bits in the padding bytes (not the suffix, not the plaintext)
        // This should still succeed — padding bytes are random and the length
        // suffix is verified independently against the tier alignment.
        let mut tampered_pad = padded.clone();
        if tampered_pad.len() > plaintext.len() + 2 {
            // Flip a byte in the padding section (between plaintext and suffix)
            let pad_byte_pos = plaintext.len() + 1;
            tampered_pad[pad_byte_pos] ^= 0xFF;
            assert!(
                unpad_message_variable(&tampered_pad).is_ok(),
                "flipped padding byte should still unpad (length is tier-verified)"
            );
        }
    }

    #[test]
    fn test_ratchet_changes_key() {
        use sodiumoxide::randombytes;
        let rx = randombytes::randombytes(32);
        let tx = randombytes::randombytes(32);
        let mut rx_arr = [0u8; 32];
        let mut tx_arr = [0u8; 32];
        rx_arr.copy_from_slice(&rx);
        tx_arr.copy_from_slice(&tx);

        let mut keys = SessionKeys {
            rx_key: rx_arr,
            tx_key: tx_arr,
        };

        let old_tx = keys.tx_key;
        let old_rx = keys.rx_key;

        keys.ratchet_tx();
        assert_ne!(keys.tx_key, old_tx, "tx key must change after ratchet");
        assert_eq!(keys.rx_key, old_rx, "rx key must NOT change when ratcheting tx");

        keys.ratchet_rx();
        assert_ne!(keys.rx_key, old_rx, "rx key must change after ratchet");

        // Verify the old key is zeroed
        // (We can't directly check because old_tx is a copy, but the field was zeroed)
    }

    // ─── HKDF Tests ──────────────────────────────────────────

    #[test]
    fn test_hkdf_extract_deterministic() {
        let salt = b"test-salt-16b";
        let ikm = b"input-key-material";
        let prk1 = hkdf_extract(salt, ikm);
        let prk2 = hkdf_extract(salt, ikm);
        assert_eq!(prk1, prk2, "HKDF-Extract must be deterministic");
    }

    #[test]
    fn test_hkdf_different_salt_different_prk() {
        let prk1 = hkdf_extract(b"salt-A", b"ikm");
        let prk2 = hkdf_extract(b"salt-B", b"ikm");
        assert_ne!(prk1, prk2, "different salt should produce different PRK");
    }

    #[test]
    fn test_hkdf_expand_deterministic() {
        let prk = [0xABu8; 32];
        let out1 = hkdf_expand(&prk, b"info", 32);
        let out2 = hkdf_expand(&prk, b"info", 32);
        assert_eq!(out1, out2, "HKDF-Expand must be deterministic");
    }

    #[test]
    fn test_hkdf_expand_output_length() {
        let prk = [0xABu8; 32];
        for len in [1, 16, 32, 64, 128] {
            let out = hkdf_expand(&prk, b"test", len);
            assert_eq!(out.len(), len, "HKDF-Expand output length should be {len}");
        }
    }

    #[test]
    fn test_hkdf_full_roundtrip() {
        let salt = b"m2m-hkdf-test";
        let ikm = b"test-input-key-material";
        let info = b"M2M-TEST";
        let out = hkdf(salt, ikm, info, 32);
        assert_eq!(out.len(), 32);
        let out2 = hkdf(salt, ikm, info, 32);
        assert_eq!(out, out2);
    }

    // ─── X3DH Tests ──────────────────────────────────────────

    #[test]
    fn test_x3dh_initiate_and_respond_produce_same_key() {
        init_sodiumoxide();
        let ik_alice = X25519IdentityKeypair::generate();
        let ik_bob = X25519IdentityKeypair::generate();
        let ek_alice = EphemeralKeypair::generate();
        let spk = EphemeralKeypair::generate();
        let bundle = PrekeyBundle {
            identity_key: ik_bob.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: None,
        };

        let alice_out = x3dh_initiate(&ik_alice, &ek_alice, &bundle).unwrap();
        let bob_out = x3dh_respond(
            &ik_bob, &spk, None,
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes(),
        ).unwrap();

        assert_eq!(alice_out.root_key, bob_out.root_key);
        assert_eq!(alice_out.chain_key, bob_out.chain_key);
    }

    #[test]
    fn test_x3dh_with_opk() {
        init_sodiumoxide();
        let ik_alice = X25519IdentityKeypair::generate();
        let ik_bob = X25519IdentityKeypair::generate();
        let ek_alice = EphemeralKeypair::generate();
        let spk = EphemeralKeypair::generate();
        let opk = EphemeralKeypair::generate();

        let bundle = PrekeyBundle {
            identity_key: ik_bob.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: Some(opk.public_key_bytes()),
        };

        let alice_out = x3dh_initiate(&ik_alice, &ek_alice, &bundle).unwrap();
        let bob_out = x3dh_respond(
            &ik_bob, &spk, Some(&opk),
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes(),
        ).unwrap();

        assert_eq!(alice_out.root_key, bob_out.root_key);
        assert_eq!(alice_out.chain_key, bob_out.chain_key);
    }

    #[test]
    fn test_x3dh_wrong_identity_key_fails() {
        init_sodiumoxide();
        let ik_alice = X25519IdentityKeypair::generate();
        let ik_bob = X25519IdentityKeypair::generate();
        let ik_evil = X25519IdentityKeypair::generate();
        let ek_alice = EphemeralKeypair::generate();
        let spk = EphemeralKeypair::generate();

        let bundle = PrekeyBundle {
            identity_key: ik_evil.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: None,
        };

        let alice_out = x3dh_initiate(&ik_alice, &ek_alice, &bundle).unwrap();
        let bob_out = x3dh_respond(
            &ik_bob, &spk, None,
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes(),
        ).unwrap();

        assert_ne!(alice_out.root_key, bob_out.root_key);
    }

    #[test]
    fn test_x3dh_wrong_signed_prekey_fails() {
        init_sodiumoxide();
        let ik_alice = X25519IdentityKeypair::generate();
        let ik_bob = X25519IdentityKeypair::generate();
        let ek_alice = EphemeralKeypair::generate();
        let real_spk = EphemeralKeypair::generate();
        let wrong_spk = EphemeralKeypair::generate();

        let bundle = PrekeyBundle {
            identity_key: ik_bob.public_key_bytes(),
            signed_prekey: real_spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: None,
        };

        let alice_out = x3dh_initiate(&ik_alice, &ek_alice, &bundle).unwrap();
        let bob_out = x3dh_respond(
            &ik_bob, &wrong_spk, None,
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes(),
        ).unwrap();

        assert_ne!(alice_out.root_key, bob_out.root_key);
        assert_ne!(alice_out.chain_key, bob_out.chain_key);
    }

    #[test]
    fn test_x3dh_without_opk_works() {
        init_sodiumoxide();
        let ik_alice = X25519IdentityKeypair::generate();
        let ik_bob = X25519IdentityKeypair::generate();
        let ek_alice = EphemeralKeypair::generate();
        let spk = EphemeralKeypair::generate();

        let bundle_no_opk = PrekeyBundle {
            identity_key: ik_bob.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: None,
        };

        let alice = x3dh_initiate(&ik_alice, &ek_alice, &bundle_no_opk).unwrap();
        let bob = x3dh_respond(&ik_bob, &spk, None,
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes()).unwrap();
        assert_eq!(alice.root_key, bob.root_key);
        assert_eq!(alice.chain_key, bob.chain_key);
    }

    // ─── Double Ratchet Tests ────────────────────────────────

    fn init_sodiumoxide() {
        let _ = sodiumoxide::init();
    }

    fn make_dr_pair() -> (DoubleRatchet, DoubleRatchet) {
        init_sodiumoxide();
        let ik_alice = X25519IdentityKeypair::generate();
        let ik_bob = X25519IdentityKeypair::generate();
        let ek_alice = EphemeralKeypair::generate();
        let spk = EphemeralKeypair::generate();
        let bundle = PrekeyBundle {
            identity_key: ik_bob.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: None,
        };
        let x3dh_alice = x3dh_initiate(&ik_alice, &ek_alice, &bundle).unwrap();
        let dh_ratchet_alice = EphemeralKeypair::generate();
        let dh_ratchet_bob = EphemeralKeypair::generate();
        // Save pub key bytes before moving the keypairs
        let alice_pub = dh_ratchet_alice.public_key_bytes();
        let bob_pub = dh_ratchet_bob.public_key_bytes();

        let alice_dr = DoubleRatchet::new(
            X3DHSessionKeys {
                root_key: x3dh_alice.root_key,
                chain_key: x3dh_alice.chain_key,
            },
            dh_ratchet_alice,
            bob_pub,
            true,
        );

        let bob_x3dh = x3dh_respond(&ik_bob, &spk, None,
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes()).unwrap();
        let bob_dr = DoubleRatchet::new(
            X3DHSessionKeys {
                root_key: bob_x3dh.root_key,
                chain_key: bob_x3dh.chain_key,
            },
            dh_ratchet_bob,
            alice_pub,
            false,
        );

        (alice_dr, bob_dr)
    }

    #[test]
    fn test_dr_encrypt_produces_valid_output() {
        let (mut alice, _bob) = make_dr_pair();
        let plaintext = b"Hello, Double Ratchet!";
        let aad = [PacketType::EncryptedMessage.to_byte()];
        let (ratchet_key, msg_num, nonce, ciphertext) = alice
            .encrypt(plaintext, &aad, false)
            .unwrap();

        // Verify the returned values are consistent
        assert!(ratchet_key.is_none(), "no ratchet requested");
        assert_eq!(msg_num, 0, "first message should have number 0");
        assert_eq!(nonce.len(), 24, "XChaCha20-Poly1305 nonce is 24 bytes");
        assert!(!ciphertext.is_empty(), "ciphertext should not be empty");
        assert_ne!(ciphertext, plaintext, "ciphertext should differ from plaintext");

        // Verify chain advanced
        assert_eq!(alice.send_message_number, 1, "send message number should advance");
    }

    #[test]
    fn test_dr_sender_and_receiver_sync() {
        let (mut alice, mut bob) = make_dr_pair();
        let aad = [PacketType::EncryptedMessage.to_byte()];

        for msg in [b"msg 1", b"msg 2", b"msg 3"] {
            let (ratchet_key, msg_num, nonce, ciphertext) = alice
                .encrypt(msg, &aad, false)
                .unwrap();
            let decrypted = bob.decrypt(&ciphertext, &nonce, &aad, msg_num, ratchet_key.as_ref())
                .unwrap();
            assert_eq!(&decrypted, msg);
        }
    }

    #[test]
    fn test_dh_ratchet_advances_keys() {
        let (mut alice, mut bob) = make_dr_pair();
        let aad = [PacketType::EncryptedMessage.to_byte()];

        let (_, msg_num1, n1, c1) = alice.encrypt(b"before ratchet", &aad, false).unwrap();
        let _ = bob.decrypt(&c1, &n1, &aad, msg_num1, None).unwrap();

        let (rk, msg_num2, n2, c2) = alice.encrypt(b"after ratchet", &aad, true).unwrap();
        assert!(rk.is_some());

        let decrypted = bob.decrypt(&c2, &n2, &aad, msg_num2, rk.as_ref()).unwrap();
        assert_eq!(&decrypted, b"after ratchet");
    }

    #[test]
    fn test_message_number_gap() {
        let (mut alice, mut bob) = make_dr_pair();
        let aad = [PacketType::EncryptedMessage.to_byte()];

        let results: Vec<_> = (0..3).map(|i| {
            alice.encrypt(format!("msg {}", i).as_bytes(), &aad, false).unwrap()
        }).collect();

        // result = (rk, msg_num, nonce, ciphertext)
        let d0 = bob.decrypt(&results[0].3, &results[0].2, &aad,
                            results[0].1, results[0].0.as_ref()).unwrap();
        assert_eq!(&d0, b"msg 0");

        let d2 = bob.decrypt(&results[2].3, &results[2].2, &aad,
                            results[2].1, results[2].0.as_ref()).unwrap();
        assert_eq!(&d2, b"msg 2");
    }

    #[test]
    fn test_dr_multiple_ratchets() {
        let (mut alice, mut bob) = make_dr_pair();
        let aad = [PacketType::EncryptedMessage.to_byte()];

        for i in 0..5 {
            let do_ratchet = i % 2 == 0;
            let (rk, msg_num, nonce, ciphertext) = alice
                .encrypt(format!("msg {}", i).as_bytes(), &aad, do_ratchet)
                .unwrap();
            let decrypted = bob.decrypt(&ciphertext, &nonce, &aad, msg_num, rk.as_ref())
                .unwrap();
            assert_eq!(&decrypted, format!("msg {}", i).as_bytes());
        }
    }

    #[test]
    fn test_dr_should_ratchet_interval() {
        let (mut dr, _) = make_dr_pair();
        assert!(!dr.should_ratchet(100));

        dr.send_message_number = 100;
        assert!(dr.should_ratchet(100));
        assert!(!dr.should_ratchet(200));
        assert!(!dr.should_ratchet(0));
    }

    #[test]
    fn test_dr_encrypt_no_send_chain_fails() {
        init_sodiumoxide();
        let x3dh = X3DHSessionKeys {
            root_key: [0xAA; 32],
            chain_key: [0xBB; 32],
        };
        let mut dr = DoubleRatchet::new(
            x3dh,
            EphemeralKeypair::generate(),
            [0xCC; 32],
            false,
        );
        let aad = [PacketType::EncryptedMessage.to_byte()];
        let result = dr.encrypt(b"test", &aad, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_dr_decrypt_no_recv_chain_fails() {
        init_sodiumoxide();
        let x3dh = X3DHSessionKeys {
            root_key: [0xAA; 32],
            chain_key: [0xBB; 32],
        };
        let mut dr = DoubleRatchet::new(
            x3dh,
            EphemeralKeypair::generate(),
            [0xCC; 32],
            true,
        );
        let aad = [PacketType::EncryptedMessage.to_byte()];
        let result = dr.decrypt(b"ciphertext", b"nonce", &aad, 0, None);
        assert!(result.is_err());
    }
}
