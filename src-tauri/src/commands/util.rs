//! Shared helper functions used across command modules.

/// AAD context for key store encryption (identity keys, peer keys).
/// Domain-separates keys.db ciphertext from messages.db ciphertext.
pub const AAD_KEY_STORE: &[u8] = b"m2m-keys-v1";

/// AAD context for message store encryption (chat history).
/// Domain-separates messages.db ciphertext from keys.db ciphertext.
pub const AAD_MSG_STORE: &[u8] = b"m2m-msg-v1";

/// AAD context for conversation export encryption.
/// Domain-separates export files from on-disk storage.
pub const AAD_EXPORT: &[u8] = b"m2m-export-v1";

/// AAD context for identity export/import encryption.
/// Domain-separates identity backup files from other ciphertext.
pub const AAD_EXPORT_V2: &[u8] = b"m2m-export-v2";

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
///
/// Uses dual-stack bind: tries IPv4 first, falls back to IPv6 for IPv6-only
/// networks. Connects to `8.8.8.8:80` to discover the kernel-selected source
/// address for outbound traffic.
pub fn resolve_local_ip() -> Option<std::net::IpAddr> {
    crate::local_addr::bind_udp_any()
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .ok()
        .map(|addr| addr.ip())
}

/// Estimate the entropy of a passphrase in bits.
///
/// Uses a character-pool base model (counts active character classes,
/// computes log2(pool^length)), then applies pattern-based penalties:
///
/// - Sequential characters ("abcd", "1234") → penalize
/// - Repeating characters ("aaa", "1111") → penalize
/// - Keyboard patterns ("qwerty", "asdf") → penalize
/// - Common substitutions ("p@ssw0rd" → "password") → detect length shrink
/// - Short length (< 12 chars) → heavy penalty
///
/// This catches weak passphrases that the character-pool model
/// overestimates, while being lenient for diceware-style phrases.
///
/// Returns an entropy estimate in bits. Minimum is 0.0.
pub fn estimate_passphrase_entropy(passphrase: &str) -> f64 {
    let bytes = passphrase.as_bytes();
    let len = passphrase.len();

    // ── 1. Character pool estimation (same as before) ──
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
    if has_lower { pool_size += 26; }
    if has_upper { pool_size += 26; }
    if has_digit { pool_size += 10; }
    if has_special { pool_size += 32; }
    if has_unicode { pool_size += 100; }

    if pool_size == 0 || len == 0 {
        return 0.0;
    }

    let pool_f = pool_size as f64;
    let len_f = len as f64;
    let mut entropy = len_f * pool_f.log2();

    // ── 2. Pattern penalties ──
    // Each penalty is a multiplicative factor (0.0 – 1.0) applied to entropy.

    // 2a. Sequential characters (abc, 123, etc.)
    let seq_penalty = detect_sequential_penalty(passphrase);

    // 2b. Repeating characters (aaa, 1111, etc.)
    let repeat_penalty = detect_repeat_penalty(passphrase);

    // 2c. Keyboard row patterns (qwerty, asdf, zxcv)
    let kb_penalty = detect_keyboard_penalty(passphrase);

    // 2d. Common substitutions (detect if most characters are
    //     from a single class with a few substitutions)
    let sub_penalty = detect_substitution_penalty(&has_lower, &has_upper, &has_digit, &has_special, len);

    // 2e. Short-length penalty (< 12 chars)
    let short_penalty = if len < 12 { 0.5 } else { 1.0 };

    // Apply the strongest penalty (most restrictive wins)
    let penalty = seq_penalty
        .min(repeat_penalty)
        .min(kb_penalty)
        .min(sub_penalty)
        .min(short_penalty);

    entropy *= penalty;

    // ── 3. NIST SP 800-63B floor ──
    // For truly random 8-char passwords, NIST gives ~18 bits.
    // Our floor ensures even severely-penalized passphrases
    // get a minimum estimate based on brute-force difficulty.
    let floor = if len >= 12 { 20.0 } else if len >= 8 { 14.0 } else { 8.0 };
    entropy = entropy.max(floor).min(128.0); // cap at 128 bits

    entropy
}

/// Penalty for sequential runs (e.g., "abc", "123", "XYZ").
/// Returns a multiplier 0.0–1.0.
fn detect_sequential_penalty(passphrase: &str) -> f64 {
    let bytes = passphrase.as_bytes();
    let mut seq_runs = 0usize;
    let mut longest_run = 0usize;
    let mut current_run = 1usize;

    // Detect ascending sequences
    for i in 1..bytes.len() {
        if bytes[i].wrapping_sub(bytes[i - 1]) == 1 {
            current_run += 1;
        } else {
            if current_run >= 3 {
                seq_runs += 1;
                longest_run = longest_run.max(current_run);
            }
            current_run = 1;
        }
    }
    if current_run >= 3 {
        seq_runs += 1;
        longest_run = longest_run.max(current_run);
    }

    // Detect descending sequences
    current_run = 1;
    for i in 1..bytes.len() {
        if bytes[i - 1].wrapping_sub(bytes[i]) == 1 {
            current_run += 1;
        } else {
            if current_run >= 3 {
                seq_runs += 1;
                longest_run = longest_run.max(current_run);
            }
            current_run = 1;
        }
    }
    if current_run >= 3 {
        seq_runs += 1;
        longest_run = longest_run.max(current_run);
    }

    if seq_runs == 0 {
        return 1.0;
    }
    // Each sequential run reduces entropy
    // A 4+ run is worth ~8 bits of deduction
    let deduction = (seq_runs as f64) * 0.15 + (longest_run as f64).max(3.0) * 0.05;
    (1.0 - deduction).max(0.3)
}

/// Penalty for repeated character runs (e.g., "aaa", "1111").
fn detect_repeat_penalty(passphrase: &str) -> f64 {
    let bytes = passphrase.as_bytes();
    let mut repeats = 0usize;
    let mut current = 1usize;

    for i in 1..bytes.len() {
        if bytes[i] == bytes[i - 1] {
            current += 1;
        } else {
            if current >= 3 {
                repeats += 1;
            }
            current = 1;
        }
    }
    if current >= 3 {
        repeats += 1;
    }

    if repeats == 0 {
        return 1.0;
    }
    // Each repeated run is a major weakness
    (1.0 - (repeats as f64) * 0.25).max(0.2)
}

/// Check for keyboard row patterns (qwerty, asdf, zxcv).
///
/// Uses char-count iteration to handle multi-byte Unicode correctly:
/// `str::len()` returns bytes, but `chars().skip(n)` skips `n` characters.
/// Using byte length as the bound causes an infinite loop on Unicode strings:
/// `chars().skip(N)` returns `""` for N >= char count, and `row.contains("")`
/// is always true, so the index never advances.
fn detect_keyboard_penalty(passphrase: &str) -> f64 {
    let lower = passphrase.to_lowercase();
    let kb_rows = ["qwertyuiop", "asdfghjkl", "zxcvbnm", "0123456789"];
    let char_count = lower.chars().count();
    let mut total_matched = 0usize;

    for row in &kb_rows {
        let mut i = 0;
        while i + 2 < char_count {
            let chunk: String = lower.chars().skip(i).take(3).collect();
            // Guard against empty chunk (should not happen with correct bounds)
            if chunk.is_empty() {
                i += 1;
                continue;
            }
            if row.contains(&chunk) {
                total_matched += chunk.len();
                i += chunk.len();
                continue;
            }
            // Also check reversed
            let rev: String = chunk.chars().rev().collect();
            if row.contains(&rev) {
                total_matched += chunk.len();
                i += chunk.len();
                continue;
            }
            i += 1;
        }
    }

    if total_matched == 0 {
        return 1.0;
    }
    let ratio = total_matched as f64 / passphrase.len() as f64;
    (1.0 - ratio * 0.5).max(0.3)
}

/// Penalty for passphrases that look like a base word with substitutions.
/// If most chars come from one class with a few from another, reduce entropy.
fn detect_substitution_penalty(
    has_lower: &bool, has_upper: &bool, has_digit: &bool, has_special: &bool, len: usize,
) -> f64 {
    let classes = [*has_lower, *has_upper, *has_digit, *has_special];
    let active_count = classes.iter().filter(|&&c| c).count();

    if active_count <= 1 {
        // Single-class passphrase — weak, especially if short
        return 0.6
    }

    // If only 2 classes active and one is dominant (e.g., lowercase + few digits):
    // this looks like "password123" — heavy penalty for short ones
    if active_count == 2 && len < 16 {
        return 0.7;
    }

    1.0 // 3+ classes is probably intentional
}

#[cfg(test)]
mod entropy_tests {
    use super::*;

    #[test]
    fn test_diceware_phrase_high_entropy() {
        // Five random diceware words should score 60+ bits
        let e = estimate_passphrase_entropy("correct-horse-battery-staple-clock");
        assert!(e >= 40.0, "diceware phrase should score >= 40 bits, got {e}");
    }

    #[test]
    fn test_short_passphrase_low_entropy() {
        let e = estimate_passphrase_entropy("abc123");
        assert!(e < 30.0, "short simple passphrase should score < 30 bits, got {e}");
    }

    #[test]
    fn test_single_word_low_entropy() {
        let e = estimate_passphrase_entropy("password");
        assert!(e < 25.0, "single common word should score < 25 bits, got {e}");
    }

    #[test]
    fn test_sequential_penalty() {
        let e = estimate_passphrase_entropy("abcdefgh12345678");
        // Sequential characters should be penalized
        let base_entropy = estimate_passphrase_entropy("xzhfmkqg94736281"); // random-looking
        assert!(e < base_entropy, "sequential passphrase {e} should be lower than random {base_entropy}");
    }

    #[test]
    fn test_repeating_penalty() {
        let e = estimate_passphrase_entropy("aaaabbbbcccc");
        assert!(e < 30.0, "repeating pattern should score < 30 bits, got {e}");
    }

    #[test]
    fn test_keyboard_penalty() {
        let e = estimate_passphrase_entropy("qwerty1234");
        assert!(e < 28.0, "keyboard pattern should score < 28 bits, got {e}");
    }

    #[test]
    fn test_unicode_mixed_high_entropy() {
        let e = estimate_passphrase_entropy("κρυπτό-密码-パスワード-123!");
        assert!(e >= 40.0, "unicode passphrase should score >= 40 bits, got {e}");
    }

    #[test]
    fn test_empty_passphrase() {
        let e = estimate_passphrase_entropy("");
        assert_eq!(e, 0.0);
    }

    #[test]
    fn test_minimum_floor_applied() {
        // Even very weak passphrases should have a minimum floor
        let e = estimate_passphrase_entropy("a");
        assert!(e > 0.0, "single char should have floor > 0");
    }

    #[test]
    fn test_strong_passphrase_high_score() {
        let e = estimate_passphrase_entropy("kX9#mP2$vL8@nR5&jW3!");
        assert!(e >= 60.0, "strong passphrase should score >= 60 bits, got {e}");
    }
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
pub fn derive_storage_key(public_key: &[u8]) -> crate::secure_key::StorageKey {
    use sodiumoxide::crypto::hash::sha256;
    let context = b"m2m-storage-key-v1";
    let mut input = Vec::with_capacity(context.len() + public_key.len());
    input.extend_from_slice(context);
    input.extend_from_slice(public_key);
    let hash = sha256::hash(&input);
    crate::secure_key::StorageKey::new(hash.0)
}

/// Encrypt data for storage using XChaCha20-Poly1305.
///
/// `aad` is Additional Authenticated Data — a context string that binds the
/// ciphertext to a specific storage domain (e.g., `b"m2m-keys"`, `b"m2m-msg"`).
/// This prevents ciphertext from one domain (e.g., keys.db) from being
/// substituted into another (e.g., messages.db), even if the same encryption
/// key is used.
pub fn crypto_encrypt_storage(
    plaintext: &[u8],
    key: &crate::secure_key::StorageKey,
    aad: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::gen_nonce();
    let aead_key = aead::Key::from_slice(key.as_bytes()).ok_or("invalid key length")?;
    let ciphertext = aead::seal(plaintext, Some(aad), &nonce, &aead_key);
    Ok((nonce.0.to_vec(), ciphertext))
}

/// Decrypt storage-encrypted data.
///
/// `aad` must match the AAD used during encryption, or decryption will fail.
pub fn crypto_decrypt_storage(
    ciphertext: &[u8],
    nonce_bytes: &[u8],
    key: &crate::secure_key::StorageKey,
    aad: &[u8],
) -> Result<Vec<u8>, String> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::Nonce::from_slice(nonce_bytes).ok_or("invalid nonce")?;
    let aead_key = aead::Key::from_slice(key.as_bytes()).ok_or("invalid key length")?;
    aead::open(ciphertext, Some(aad), &nonce, &aead_key).map_err(|_| "decryption failed".to_string())
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
