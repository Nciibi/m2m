//! MIGRATION STEP 1 — Golden vector capture (TEMPORARY, pre-swap).
//!
//! Run with libsodium still in place:
//!   cargo test --lib migration_vector_capture -- --nocapture
//! Paste the printed constants into `crypto.rs` tests module as the
//! permanent golden vectors, THEN swap implementations. After the swap the
//! same vectors must pass — proving byte-level wire/DB compatibility.

#[test]
fn migration_vector_capture() {
    use crate::crypto::{self};

    // ── AEAD golden: fixed key/nonce/AAD/plaintext → ciphertext ──
    let key = [0x42u8; 32];
    let nonce: [u8; 24] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    ];
    let aad = b"m2m-golden-aad";
    let plaintext = b"M2M golden vector plaintext";
    let key = crypto::GoldenAead::new(key);
    let ct = crypto::GoldenAead::seal(&key, &nonce, plaintext, aad);
    println!("AEAD_CT = {:?}", ct);

    // ── Ed25519 golden: fixed seed → signature over fixed message ──
    let seed: [u8; 32] = core::array::from_fn(|i| i as u8);
    let kp = crate::crypto::IdentityKeypair::from_bytes(
        &[0u8; 32], // pub ignored by from_bytes? verify below
        {
            let mut sk = [0u8; 64];
            // derive via generate then overwrite is wrong; instead sign with seed-derived pair:
            sk
        },
    );
    drop(kp);
    // Simpler: generate pair FROM seed through libsodium-compatible path.
    let kp2 = crypto::golden_identity_from_seed(&seed);
    let sig = kp2.sign(b"m2m golden message");
    println!("ED_PUB = {:?}", kp2.public_key_bytes());
    println!("ED_SIG = {:?}", sig);

    // ── X25519 golden: RFC7748-style scalar×base → shared ──
    let alice_sk: [u8; 32] = core::array::from_fn(|i| (i as u8) ^ 0xA5);
    let bob_pk: [u8; 32] = core::array::from_fn(|i| (i as u8) ^ 0x5A);
    let xk = crypto::golden_x25519_shared(&alice_sk, &bob_pk);
    println!("X25519_SHARED = {:?}", xk);
}
