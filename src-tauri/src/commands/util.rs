//! Shared helper functions used across command modules.

/// Decode a 64-char hex string into a 32-byte peer key.
/// Returns an error if the hex string is malformed or wrong length.
pub fn decode_peer_key(hex_str: &str) -> Result<[u8; 32], String> {
    if hex_str.len() != 64 {
        return Err(format!(
            "invalid peer key hex length: expected 64 chars, got {}",
            hex_str.len()
        ));
    }
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid peer key hex: {e}"))?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Decode a peer hex key, logging an error and returning `None` on failure.
/// Prevents silent database corruption from malformed hex strings.
pub fn decode_peer_key_logged(hex_str: &str) -> Option<[u8; 32]> {
    match decode_peer_key(hex_str) {
        Ok(key) => Some(key),
        Err(e) => {
            tracing::error!(hex_len = hex_str.len(), error = %e, "decode_peer_key failed — skipping store operation");
            None
        }
    }
}

/// Resolve the local (non-loopback) IP address used for internet connectivity.
pub fn resolve_local_ip() -> Option<std::net::IpAddr> {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .ok()
        .map(|addr| addr.ip())
}

/// Estimate the entropy of a passphrase in bits.
///
/// Uses a simplified character-pool model: counts the size of the
/// character set used, then computes log2(pool^length).
///
/// This is a rough estimate — actual entropy depends on the randomness
/// of the passphrase generation process. It catches the worst cases
/// (single-word, all-lowercase, short passphrases) while being
/// deliberately lenient for diceware-style multi-word phrases.
pub fn estimate_passphrase_entropy(passphrase: &str) -> f64 {
    let bytes = passphrase.as_bytes();

    // Detect which character classes are present.
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    let mut has_special = false;
    let mut has_unicode = false;

    for &b in bytes {
        if b.is_ascii_lowercase() {
            has_lower = true;
        } else if b.is_ascii_uppercase() {
            has_upper = true;
        } else if b.is_ascii_digit() {
            has_digit = true;
        } else if b.is_ascii_punctuation() || b.is_ascii_graphic() {
            has_special = true;
        } else if !b.is_ascii() {
            has_unicode = true;
        }
    }

    let mut pool_size = 0u32;
    if has_lower {
        pool_size += 26;
    }
    if has_upper {
        pool_size += 26;
    }
    if has_digit {
        pool_size += 10;
    }
    if has_special {
        pool_size += 32;
    }
    if has_unicode {
        pool_size += 100; // rough estimate for Unicode charset
    }

    if pool_size == 0 {
        return 0.0;
    }

    // Entropy = length * log2(pool_size)
    let pool_f = pool_size as f64;
    let len = passphrase.len() as f64;
    len * pool_f.log2()
}

/// Derive a storage encryption key from a user-supplied passphrase using Argon2id.
/// Returns a `StorageKey` which is locked in physical RAM (mlock/VirtualLock)
/// and automatically zeroized on drop.
/// The `salt` should be unique per identity (we use the public key).
pub fn derive_storage_key_from_passphrase(passphrase: &str, salt: &[u8]) -> Result<crate::secure_key::StorageKey, String> {
    use argon2::{Argon2, Algorithm, Version, Params};

    let params = Params::new(
        65536, // 64 MiB memory
        3,     // 3 iterations
        4,     // 4 parallelism lanes
        Some(32),
    ).map_err(|e| format!("argon2 params error: {e}"))?;

    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon.hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("argon2 hash failed: {e}"))?;
    Ok(crate::secure_key::StorageKey::new(key))
}

/// Legacy fallback: derive a storage encryption key from the public key.
/// Used when no vault passphrase has been set (migration / first-run).
pub fn derive_storage_key(public_key: &[u8]) -> [u8; 32] {
    use sodiumoxide::crypto::hash::sha256;
    let context = b"m2m-storage-key-v1";
    let mut input = Vec::with_capacity(context.len() + public_key.len());
    input.extend_from_slice(context);
    input.extend_from_slice(public_key);
    let hash = sha256::hash(&input);
    hash.0
}

/// Encrypt data for storage using XChaCha20-Poly1305.
pub fn crypto_encrypt_storage(
    plaintext: &[u8],
    key: &crate::secure_key::StorageKey,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::gen_nonce();
    let aead_key = aead::Key::from_slice(key.as_bytes()).ok_or("invalid key length")?;
    let ciphertext = aead::seal(plaintext, None, &nonce, &aead_key);
    Ok((nonce.0.to_vec(), ciphertext))
}

/// Decrypt storage-encrypted data.
pub fn crypto_decrypt_storage(
    ciphertext: &[u8],
    nonce_bytes: &[u8],
    key: &crate::secure_key::StorageKey,
) -> Result<Vec<u8>, String> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::Nonce::from_slice(nonce_bytes).ok_or("invalid nonce")?;
    let aead_key = aead::Key::from_slice(key.as_bytes()).ok_or("invalid key length")?;
    aead::open(ciphertext, None, &nonce, &aead_key).map_err(|_| "decryption failed".to_string())
}

/// Create a temporary file pre-allocated to the given size.
/// Returns (Option<File>, Option<PathBuf>) — either both Some or both None.
/// The file is created in the OS temp directory with a unique name.
pub fn create_temp_file(size: u64) -> std::io::Result<(std::fs::File, std::path::PathBuf)> {
    let mut path = std::env::temp_dir();
    path.push(format!("m2m_{}", uuid::Uuid::new_v4()));

    let file = std::fs::File::create(&path)?;
    // Pre-allocate the file to the full expected size.
    // This ensures we have enough disk space and avoids fragmentation.
    file.set_len(size)?;

    Ok((file, path))
}
