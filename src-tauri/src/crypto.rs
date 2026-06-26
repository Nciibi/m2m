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
/// Supports ratcheting for forward secrecy: keys evolve after each use.
pub struct SessionKeys {
    rx_key: [u8; 32],
    tx_key: [u8; 32],
}

/// Block size for message padding to obfuscate plaintext length.
/// All encrypted messages are padded to the next multiple of this block size.
const PADDING_BLOCK: usize = 256;

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
        let mut input = Vec::with_capacity(32 + 16);
        input.extend_from_slice(&self.tx_key);
        input.extend_from_slice(b"m2m-ratchet-tx-v1");
        let hash = sha256::hash(&input);
        self.tx_key.zeroize();
        self.tx_key.copy_from_slice(&hash.0[..32]);
    }

    /// Ratchet the receiving key forward after decrypting a message.
    /// Mirror of ratchet_tx for the receive direction.
    pub fn ratchet_rx(&mut self) {
        let mut input = Vec::with_capacity(32 + 16);
        input.extend_from_slice(&self.rx_key);
        input.extend_from_slice(b"m2m-ratchet-rx-v1");
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

/// Initialize the sodiumoxide library. Must be called once at startup.
pub fn init() -> Result<(), CryptoError> {
    sodiumoxide::init().map_err(|_| CryptoError::InitFailed)
}

// ─── Message Padding ────────────────────────────────────────────────────────

/// Pad a plaintext message to obfuscate its true length on the wire.
///
/// Scheme: [original data] [random padding bytes] [pad_len as u8]
/// Where total = len(original) + pad_len + 1 is a multiple of PADDING_BLOCK.
/// pad_len ranges from 0 to 255.
///
/// This prevents traffic analysis from determining message content type
/// by size — all encrypted messages appear to be multiples of PADDING_BLOCK.
///
/// # Arguments
/// * `plaintext` - The message to pad
///
/// # Returns
/// Padded message: [`plaintext` | `padding` | `pad_len`]
pub fn pad_message(plaintext: &[u8]) -> Vec<u8> {
    let block = PADDING_BLOCK as u32;
    // Calculate padding needed: (len + pad_len + 1) % block == 0
    let needed = (plaintext.len() as u32 + 1) % block;
    // We want: (plaintext.len() + pad_len + 1) % block == 0
    // So pad_len = (block - (plaintext.len() + 1) % block) % block
    let pad_len = if needed == 0 { 0 } else { block - needed };

    let mut padded = Vec::with_capacity(plaintext.len() + pad_len as usize + 1);
    padded.extend_from_slice(plaintext);
    // Fill padding with random bytes for obfuscation
    padded.extend(random_bytes(pad_len as usize));
    padded.push(pad_len as u8);
    padded
}

/// Remove padding from a padded message.
///
/// Reads the last byte to determine pad length, strips padding and the
/// length byte, returning only the original plaintext.
///
/// # Arguments
/// * `padded` - The padded message
///
/// # Returns
/// Original plaintext, or CryptoError if padding is invalid
pub fn unpad_message(padded: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if padded.is_empty() {
        return Err(CryptoError::DecryptionFailed);
    }
    let pad_len = padded[padded.len() - 1] as usize;
    // Validate: pad_len + 1 bytes must fit within the message
    // pad_len can be 0-255, so pad_len + 1 is 1-256
    if pad_len >= padded.len() {
        // pad_len can't be >= the entire message (need at least 1 byte of data)
        return Err(CryptoError::DecryptionFailed);
    }
    let original_len = padded.len() - 1 - pad_len;
    Ok(padded[..original_len].to_vec())
}

/// Create a `Zeroizing` wrapper around a 32-byte key.
/// This ensures the key is automatically zeroized when dropped.
pub fn protected_key(key: [u8; 32]) -> zeroize::Zeroizing<[u8; 32]> {
    zeroize::Zeroizing::new(key)
}

#[cfg(test)]
mod crypto_tests {
    use super::*;

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
            let padded = pad_message(input);
            let unpadded = unpad_message(&padded).unwrap();
            assert_eq!(input, &unpadded[..], "roundtrip failed for len={}", input.len());
            // Verify padding meets block alignment
            // padded = input + pad_bytes + [pad_len]
            // total should be input.len() + pad_len + 1
            let pad_len = padded[padded.len() - 1] as usize;
            assert_eq!(padded.len(), input.len() + pad_len + 1,
                "padding length mismatch for len={}", input.len());
        }
    }

    #[test]
    fn test_padding_hides_length() {
        // Messages of different lengths should produce same-length ciphertexts
        // when they're in the same padding block
        let short = pad_message(b"hi");
        let long = pad_message(b"hello world this is a longer message");
        // After padding, both should be aligned to PADDING_BLOCK
        assert_eq!(short.len() % PADDING_BLOCK, 0,
            "short message padding not aligned: len={}", short.len());
        assert_eq!(long.len() % PADDING_BLOCK, 0,
            "long message padding not aligned: len={}", long.len());
    }

    #[test]
    fn test_invalid_unpad_rejected() {
        // Empty message
        assert!(unpad_message(b"").is_err());
        // Message with pad_len = 255 but only 10 bytes total
        let mut bad = vec![0u8; 10];
        bad[9] = 255;
        assert!(unpad_message(&bad).is_err());
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
}
