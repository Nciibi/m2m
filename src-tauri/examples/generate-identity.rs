//! Offline identity generator.
//!
//! Useful for air-gapped setups: generate a keypair on a machine that never
//! touches the network, print the fingerprint, and carry the secret across.
//!
//! Uses `m2m_lib::crypto` rather than a direct dependency. It previously
//! imported `sodiumoxide`, which the project migrated away from — so this
//! target had not compiled since the migration, and `cargo test` /
//! `cargo clippy --all-targets` both failed on it.

use m2m_lib::crypto::{fingerprint_from_public_key, IdentityKeypair};

fn main() {
    let kp = IdentityKeypair::generate().expect("OS RNG unavailable");

    let public_key_hex = hex::encode(kp.public_key_bytes());
    let secret_key_hex = hex::encode(kp.secret_key_bytes());
    // Use the crate's own derivation rather than reimplementing it — a second
    // implementation that drifted by a single byte would produce a fingerprint
    // that never matches the app's, and a user comparing them out of band would
    // have no way to tell the difference between "not my contact" and "this
    // tool is broken".
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
