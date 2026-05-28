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
use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
use sodiumoxide::crypto::hash::sha256;
use sodiumoxide::crypto::kx;
use sodiumoxide::crypto::sign;
use sodiumoxide::randombytes;
use zeroize::Zeroize;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum size of data that can be encrypted in a single operation (16 MiB).
const MAX_ENCRYPT_SIZE: usize = 16 * 1024 * 1024;

/// Context string for HKDF session key derivation.
const SESSION_KEY_CONTEXT: &[u8] = b"m2m-v1-session-key";

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
    let sig =
        sign::Signature::from_slice(signature).ok_or(CryptoError::SignatureInvalid)?;
    if sign::verify_detached(&sig, message, &pk) {
        Ok(())
    } else {
        Err(CryptoError::SignatureInvalid)
    }
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

/// Session keys derived from key exchange.
/// Separate keys for sending and receiving (directional).
pub struct SessionKeys {
    rx_key: [u8; 32],
    tx_key: [u8; 32],
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

/// Initialize the sodiumoxide library. Must be called once at startup.
pub fn init() -> Result<(), CryptoError> {
    sodiumoxide::init().map_err(|_| CryptoError::InitFailed)
}
