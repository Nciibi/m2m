//! M2M — shared command helpers
//!
//! ## Lock order
//!
//! When a code path needs both the vault storage key and the message store,
//! acquire **`storage_key` first, then `message_store`** — never the reverse.
//!
//! `storage_key` is a `tokio::sync::RwLock` (write-preferring: a queued writer
//! blocks subsequent readers) and `message_store` is a `tokio::sync::Mutex`.
//! Sixteen read paths took them in the documented order; `search_messages` and
//! the sync-resend path took them the other way. That is a lock-order cycle: a
//! holder of `message_store` waits for `storage_key.read()`, which is blocked
//! by a queued `storage_key` writer, which is blocked by a reader holding the
//! lock and waiting for `message_store`. A permanent hang, with no timeout and
//! no error — on the vault-lock path, since `lock_vault` and `unlock_vault` are
//! exactly the queued writers.
//!
//! The same rule applies to `AppState::our_peer_key_hex` and
//! `group_manager`: snapshot the value out of `identity` before touching
//! `group_manager`.
//! Shared helper functions used across command modules.

use crate::error::AppError;
/// AAD context for key store encryption (identity keys, peer keys).
/// Domain-separates keys.db ciphertext from messages.db ciphertext.
pub const AAD_KEY_STORE: &[u8] = b"m2m-keys-v1";

/// AAD domain for message content ciphertext.
///
/// Re-exported from [`crate::storage`], which owns the definition. This
/// constant previously had a second, hand-synchronised copy in `storage.rs`
/// next to its only use — a silent-drift hazard for a security-critical domain
/// separator, documented in a comment that the adjacent constant immediately
/// violated.
pub use crate::storage::AAD_MSG_STORE;

// NOTE: the per-message content-key AAD (`m2m-msg-cek-v1`) lives in
// `storage.rs`, next to the only code that uses it. It used to be declared
// here as well, producing a second, dead copy of a security-critical domain
// separator. Two copies of an AAD constant is a silent-drift hazard: if they
// ever diverged, a wrapped content key could be accepted in the wrong domain.
// Keep exactly one definition, next to its use.

/// AAD context for conversation export encryption.
/// Domain-separates export files from on-disk storage.
pub const AAD_EXPORT: &[u8] = b"m2m-export-v1";

/// AAD context for identity export/import encryption.
/// Domain-separates identity backup files from other ciphertext.
pub const AAD_EXPORT_V2: &[u8] = b"m2m-export-v2";

/// Decode a 64-char hex string into a 32-byte peer key.
/// Returns an error if the hex string is malformed or wrong length.
pub fn decode_peer_key(hex_str: &str) -> Result<[u8; 32], AppError> {
    if hex_str.len() != 64 {
        return Err(AppError::invalid(format!(
            "invalid peer key hex length: expected 64 chars, got {}",
            hex_str.len()
        )));
    }
    let bytes = hex::decode(hex_str)
        .map_err(|e| AppError::invalid(format!("invalid peer key hex: {e}")))?;
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

/// Truncate a string for display so the result is at most `max_total`
/// characters, appending `suffix` when truncation occurs.
///
/// Operates on Unicode scalar values (`char`s), never raw byte indices, so
/// multi-byte UTF-8 input (CJK, emoji, accented text) cannot panic on a
/// non-boundary index or split a codepoint (H2). The audit found remote-
/// reachable panics where peer-supplied messages were sliced at fixed byte
/// offsets (`&content_str[..80]`).
///
/// If `suffix` alone would fill `max_total`, the result is just the first
/// `max_total` characters of `s` without a suffix.
pub fn truncate_utf8(s: &str, max_total: usize, suffix: &str) -> String {
    if s.chars().count() <= max_total {
        return s.to_string();
    }
    let suffix_len = suffix.chars().count();
    let body_len = if suffix_len < max_total {
        max_total - suffix_len
    } else {
        max_total
    };
    let mut out: String = s.chars().take(body_len).collect();
    if suffix_len < max_total {
        out.push_str(suffix);
    }
    out
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
/// Which passphrase is being validated, for error-message wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphraseKind {
    /// The vault passphrase (unlock, account creation, identity import).
    Vault,
    /// The duress passphrase that unlocks the decoy vault.
    Duress,
}

impl PassphraseKind {
    fn label(self) -> &'static str {
        match self {
            PassphraseKind::Vault => "passphrase",
            PassphraseKind::Duress => "duress passphrase",
        }
    }
}

/// Minimum acceptable length for any passphrase.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// Minimum acceptable estimated entropy, in bits.
pub const MIN_PASSPHRASE_BITS: f64 = 40.0;

/// Enforce the passphrase strength policy in one place.
///
/// This check existed in five places — `unlock_vault`, `create_vault_account`,
/// `export_identity`, `import_identity` and `set_duress_passphrase` — and had
/// already drifted: `export_identity` and `set_duress_passphrase` dropped the
/// "longer is more secure" suffix, and the duress variant diverged further. A
/// policy change needed five edits and one miss would have been silent.
///
/// Centralising it means the gate cannot drift again, and the duress passphrase
/// is held to the same bar as the vault one — which it must be, because
/// `unlock_vault` runs the identical check first, so a weaker duress passphrase
/// could never have triggered.
pub fn validate_passphrase(passphrase: &str, kind: PassphraseKind) -> Result<(), AppError> {
    if passphrase.len() < MIN_PASSPHRASE_CHARS {
        return Err(AppError::invalid(format!(
            "{} must be at least {} characters — longer is more secure",
            kind.label(),
            MIN_PASSPHRASE_CHARS
        )));
    }
    let entropy = estimate_passphrase_entropy(passphrase);
    if entropy < MIN_PASSPHRASE_BITS {
        return Err(AppError::invalid(format!(
            "{} too weak: ~{:.0} bits. Use a stronger passphrase (aim for 60+).",
            kind.label(),
            entropy
        )));
    }
    Ok(())
}

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
        pool_size += 100;
    }

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
    let sub_penalty =
        detect_substitution_penalty(&has_lower, &has_upper, &has_digit, &has_special, len);

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
    let floor = if len >= 12 {
        20.0
    } else if len >= 8 {
        14.0
    } else {
        8.0
    };
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
    has_lower: &bool,
    has_upper: &bool,
    has_digit: &bool,
    has_special: &bool,
    len: usize,
) -> f64 {
    let classes = [*has_lower, *has_upper, *has_digit, *has_special];
    let active_count = classes.iter().filter(|&&c| c).count();

    if active_count <= 1 {
        // Single-class passphrase — weak, especially if short
        return 0.6;
    }

    // If only 2 classes active and one is dominant (e.g., lowercase + few digits):
    // this looks like "password123" — heavy penalty for short ones
    if active_count == 2 && len < 16 {
        return 0.7;
    }

    1.0 // 3+ classes is probably intentional
}

/// Derive a storage encryption key from a user-supplied passphrase using Argon2id.
///
/// Returns a `StorageKey` that is zeroized on drop. It is **not** yet pinned in
/// physical RAM: `mlock` works on an address, and this value is about to be
/// moved into `state.storage_key`, so the caller must call
/// [`mlock_storage_key`] once it is installed there. Every installation site in
/// `vault.rs` does.
///
/// The `salt` should be unique per identity (we use the public key).
pub fn derive_storage_key_from_passphrase(
    passphrase: &str,
    salt: &[u8],
) -> Result<crate::secure_key::StorageKey, AppError> {
    use argon2::{Algorithm, Argon2, Params, Version};

    let params = Params::new(
        65536, // 64 MiB memory
        3,     // 3 iterations
        4,     // 4 parallelism lanes
        Some(32),
    )
    .map_err(|e| AppError::invalid(format!("argon2 params error: {e}")))?;

    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::invalid(format!("argon2 hash failed: {e}")))?;
    Ok(crate::secure_key::StorageKey::new(key))
}

/// Legacy fallback: derive a storage encryption key from the public key.
///
/// ⚠️ DEPRECATED for new data: the resulting key is publicly computable from
/// the (public) Ed25519 identity key, so data sealed under it is NOT protected
/// at rest. This exists ONLY so that pre-vault profiles and imported identities
/// can be decrypted once and migrated to a passphrase-derived key
/// (`derive_storage_key_from_passphrase`) on the next unlock. Do NOT call this
/// from any code path that writes new secrets to disk.
pub fn derive_storage_key(public_key: &[u8]) -> crate::secure_key::StorageKey {
    use sha2::Digest;
    let context = b"m2m-storage-key-v1";
    let mut input = Vec::with_capacity(context.len() + public_key.len());
    input.extend_from_slice(context);
    input.extend_from_slice(public_key);
    let hash = sha2::Sha256::digest(&input);
    crate::secure_key::StorageKey::new(hash.into())
}

/// Pin the vault storage key once it is installed in `AppState`.
///
/// `mlock` works on an *address*, and `StorageKey::new` deliberately does not
/// lock: its `self` is a local that the caller moves, so locking there pinned a
/// stack page that was abandoned while leaving the live key — now behind
/// `state.storage_key` — pageable. That also leaked `RLIMIT_MEMLOCK`, so after
/// enough unlocks every subsequent `mlock` failed and only warned.
///
/// Call this immediately after `*state.storage_key.write().await = Some(key)`,
/// while the write guard still holds the value in place. Every installation
/// site in `vault.rs` goes through here.
///
/// Best-effort by design: see [`crate::secure_key::StorageKey::lock_memory`].
pub fn mlock_storage_key(slot: &Option<crate::secure_key::StorageKey>) {
    if let Some(k) = slot {
        k.lock_memory();
    }
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
) -> Result<(Vec<u8>, Vec<u8>), AppError> {
    // XChaCha20-Poly1305-IETF: 24-byte nonce, ciphertext||tag (RustCrypto).
    use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
    let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key.as_bytes()));
    let nonce_bytes = crate::crypto::random_bytes(24);
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AppError::new("storage.encryption_failed", "encryption failed"))?;
    Ok((nonce_bytes, ciphertext))
}

/// Decrypt storage-encrypted data.
///
/// `aad` must match the AAD used during encryption, or decryption will fail.
pub fn crypto_decrypt_storage(
    ciphertext: &[u8],
    nonce_bytes: &[u8],
    key: &crate::secure_key::StorageKey,
    aad: &[u8],
) -> Result<Vec<u8>, AppError> {
    use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
    if nonce_bytes.len() != 24 {
        return Err(AppError::invalid("invalid nonce"));
    }
    let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key.as_bytes()));
    cipher
        .decrypt(
            chacha20poly1305::XNonce::from_slice(nonce_bytes),
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| {
            AppError::new(
                "storage.decryption_failed",
                "decryption failed — wrong key, or the data has been modified",
            )
        })
}

/// Create a temporary file for an incoming transfer.
/// Returns (Option<File>, Option<PathBuf>) — either both Some or both None.
/// The file is created in the OS temp directory with a unique name.
///
/// The file is NOT pre-allocated: it grows as chunks are written. Never call
/// `set_len` with a peer-declared size — callers must validate sizes against
/// `protocol::MAX_FILE_SIZE` before accepting a transfer at all.
pub fn create_temp_file() -> std::io::Result<(std::fs::File, std::path::PathBuf)> {
    let mut path = std::env::temp_dir();
    path.push(format!("m2m_{}", uuid::Uuid::new_v4()));

    let file = std::fs::File::create(&path)?;

    Ok((file, path))
}

/// Move a file, falling back to copy + delete across filesystem boundaries.
///
/// `std::fs::rename` is not a copy: it fails with `EXDEV` /
/// `ERROR_NOT_SAME_DEVICE` when source and destination are on different
/// filesystems. That is the *normal* case for a received download — `/tmp` is a
/// separate tmpfs mount on most Linux systems, and `%LOCALAPPDATA%\Temp` is very
/// often a different volume from the user's Downloads folder.
///
/// The receiving path treated that failure as fatal and deleted the temp file,
/// so a fully received, per-chunk-verified, whole-file-SHA-256-verified
/// download was destroyed with nothing saved and nothing reported.
///
/// Blockingly: `read`/`write`/`sync_all` are syscalls that can stall on a slow
/// or full volume, so this must be called off the async runtime
/// (`spawn_blocking`). The `rename` fast path is kept first because it is atomic
/// and free.
pub fn move_across_filesystems(
    from: &std::path::Path,
    to: &std::path::Path,
) -> std::io::Result<()> {
    use std::io::{Read, Write};

    match std::fs::rename(from, to) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() != std::io::ErrorKind::CrossesDevices => return Err(e),
        Err(_) => {} // genuinely cross-device: fall through to copy
    }

    let mut src = std::fs::File::open(from)?;
    // Write to a sibling temp name and rename into place, so a crash mid-copy
    // cannot leave a truncated file at the destination path looking complete.
    // `set_file_name`, not `push`: `push` with a separator-free string *appends* a
    // component rather than replacing the last one, which would turn
    // `/home/u/Downloads/report.pdf` into `.../report.pdf/report.pdf.m2m-partial`
    // and fail `File::create` with ENOTDIR — i.e. the fallback would be dead on
    // arrival, which is the exact failure it exists to fix.
    let stem = to
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let mut dest_tmp = to.to_path_buf();
    dest_tmp.set_file_name(format!("{stem}.m2m-partial"));

    {
        let mut dst = std::fs::File::create(&dest_tmp)?;
        let mut buf = vec![0u8; crate::protocol::MAX_FILE_CHUNK_SIZE];
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n])?;
        }
        // Flush before the rename so a crash cannot leave a renamed-but-empty
        // destination.
        dst.sync_all()?;
    }
    std::fs::rename(&dest_tmp, to)?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

#[cfg(test)]
mod entropy_tests {
    use super::*;

    #[test]
    fn test_diceware_phrase_high_entropy() {
        // Five random diceware words should score 60+ bits
        let e = estimate_passphrase_entropy("correct-horse-battery-staple-clock");
        assert!(
            e >= 40.0,
            "diceware phrase should score >= 40 bits, got {e}"
        );
    }

    #[test]
    fn test_short_passphrase_low_entropy() {
        let e = estimate_passphrase_entropy("abc123");
        assert!(
            e < 30.0,
            "short simple passphrase should score < 30 bits, got {e}"
        );
    }

    #[test]
    fn test_single_word_low_entropy() {
        let e = estimate_passphrase_entropy("password");
        assert!(
            e < 25.0,
            "single common word should score < 25 bits, got {e}"
        );
    }

    #[test]
    fn test_sequential_penalty() {
        let e = estimate_passphrase_entropy("abcdefgh12345678");
        // Sequential characters should be penalized
        let base_entropy = estimate_passphrase_entropy("xzhfmkqg94736281"); // random-looking
        assert!(
            e < base_entropy,
            "sequential passphrase {e} should be lower than random {base_entropy}"
        );
    }

    #[test]
    fn test_repeating_penalty() {
        let e = estimate_passphrase_entropy("aaaabbbbcccc");
        assert!(
            e < 30.0,
            "repeating pattern should score < 30 bits, got {e}"
        );
    }

    #[test]
    fn test_keyboard_penalty() {
        let e = estimate_passphrase_entropy("qwerty1234");
        assert!(e < 28.0, "keyboard pattern should score < 28 bits, got {e}");
    }

    #[test]
    fn test_unicode_mixed_high_entropy() {
        let e = estimate_passphrase_entropy("κρυπτό-密码-パスワード-123!");
        assert!(
            e >= 40.0,
            "unicode passphrase should score >= 40 bits, got {e}"
        );
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
        assert!(
            e >= 60.0,
            "strong passphrase should score >= 60 bits, got {e}"
        );
    }
}

#[cfg(test)]
mod truncate_tests {
    use super::truncate_utf8;

    /// H2 regression: byte-index slicing at a fixed offset panicked on
    /// multi-byte input. 81 CJK chars = 243 bytes; slicing at byte 80 used
    /// to panic. This is the remote-reachable network.rs attack case.
    #[test]
    fn test_multibyte_input_does_not_panic() {
        let cjk = "消".repeat(100); // 300 bytes, 100 chars
        let out = truncate_utf8(&cjk, 80, "...");
        assert_eq!(out.chars().count(), 80);
        assert!(out.ends_with("..."));
        // The result must be valid UTF-8 with no split codepoints.
        assert_eq!(out.chars().filter(|c| *c != '.').count(), 77);
    }

    /// Emoji are 4-byte codepoints: byte 80 lands mid-codepoint.
    #[test]
    fn test_emoji_boundary_does_not_panic() {
        let emoji = "🔒".repeat(100); // 400 bytes, 100 chars
        let out = truncate_utf8(&emoji, 80, "...");
        assert_eq!(out.chars().count(), 80);
        assert!(out.ends_with("..."));
        // No split codepoints: every char except the ASCII dots is the emoji.
        assert_eq!(out.chars().filter(|c| *c != '.').count(), 77);
    }

    /// Short input passes through untouched, no suffix appended.
    #[test]
    fn test_short_input_unchanged() {
        assert_eq!(truncate_utf8("hello", 80, "..."), "hello");
        // Exactly at the limit: still unchanged.
        let exact = "a".repeat(80);
        assert_eq!(truncate_utf8(&exact, 80, "..."), exact);
    }

    /// ASCII truncation keeps total length within max_total.
    #[test]
    fn test_ascii_truncation_total_length() {
        let long = "a".repeat(200);
        let out = truncate_utf8(&long, 80, "...");
        assert_eq!(out.len(), 80); // 77 chars + "..." (ASCII: bytes == chars)
        assert!(out.ends_with("..."));
    }

    /// Mixed multibyte + ASCII content stays valid UTF-8.
    #[test]
    fn test_mixed_content_valid_utf8() {
        let mixed = format!("{}héllo wörld {}", "é".repeat(50), "🎉".repeat(20));
        let out = truncate_utf8(&mixed, 80, "…");
        assert!(out.chars().count() <= 80);
        assert!(out.ends_with('…'));
        // Re-encode round-trip proves no codepoint was split.
        let _bytes = out.as_bytes();
    }

    /// Degenerate suffix: longer than max_total → plain cut without suffix.
    #[test]
    fn test_suffix_longer_than_limit() {
        let long = "a".repeat(50);
        let out = truncate_utf8(&long, 5, "......");
        assert_eq!(out, "aaaaa");
    }
}
