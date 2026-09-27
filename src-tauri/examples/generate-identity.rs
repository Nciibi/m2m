//! Offline identity generator.
//!
//! Useful for air-gapped setups: generate a keypair on a machine that never
//! touches the network, print the fingerprint, and carry the secret across.
//!
//! Uses `m2m_lib::crypto` rather than a direct dependency. It previously
//! imported `sodiumoxide`, which the project migrated away from — so this
//! target had not compiled since the migration, and `cargo test` /
//! `cargo clippy --all-targets` both failed on it.

use m2m_lib::crypto::IdentityKeypair;

/// Format an Ed25519 public key as a colon-separated fingerprint.
///
/// MUST match `Session::peer_fingerprint` / `IdentityKeypair::fingerprint`
/// byte for byte, or a user comparing fingerprints out of band would be
/// comparing two different derivations and could be fooled by a mismatch that
/// is actually cosmetic.
fn fingerprint_from_public_key(public_key: &[u8; 32]) -> String {
    let full = IdentityKeypair::fingerprint_hex(public_key);
    full.as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or("????"))
        .collect::<Vec<&str>>()
        .join(":")
}

fn main() {
    let kp = IdentityKeypair::generate().expect("OS RNG unavailable");

    let public_key_hex = hex::encode(kp.public_key_bytes());
    let secret_key_hex = hex::encode(kp.secret_key_bytes());
    let fingerprint = fingerprint_from_public_key(&kp.public_key_bytes());

    println!("=== M2M Identity Generated ===");
    println!();
    println!("Fingerprint:     {}", fingerprint);
    println!("Public Key:      {}", public_key_hex);
    println!("Private Key:     {}", secret_key_hex);
    println!();
    println!("WARNING: The private key is shown ONCE. Store it securely.");
    println!("   This is the key that controls your identity.");
    println!();
    println!("=== Save this somewhere safe ===");
}
