//! M2M — Cryptography Benchmarks
//!
//! Measures critical cryptographic operations for performance regression detection.
//! Uses Criterion.rs for statistically rigorous measurement.
//!
//! Run with: `cargo bench --bench crypto_bench`
//! Quick smoke test: `cargo bench --bench crypto_bench -- --quick`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use m2m_lib::crypto::{
    self, pad_message_variable, unpad_message_variable, DoubleRatchet, EphemeralKeypair,
    X3DHSessionKeys, X25519IdentityKeypair,
};

/// Helper: create a DoubleRatchet in a known state for benchmarking.
fn make_dr() -> DoubleRatchet {
    let ek_a = EphemeralKeypair::generate();
    let ek_b = EphemeralKeypair::generate();

    // X3DH output using fixed keys — not a valid X3DH derivation, but produces valid DR state
    let x3dh = X3DHSessionKeys {
        root_key: *b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        chain_key: *b"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    };
    DoubleRatchet::new(x3dh, ek_a, ek_b.public_key_bytes(), true)
}

/// Helper: create a receiver DR from the same starting state.
fn make_dr_receiver() -> (DoubleRatchet, [u8; 32]) {
    let ek_a_pub = {
        let ek_a = EphemeralKeypair::generate();
        ek_a.public_key_bytes()
    };
    let ek_b = EphemeralKeypair::generate();
    let x3dh = X3DHSessionKeys {
        root_key: *b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        chain_key: *b"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    };
    (DoubleRatchet::new(x3dh, ek_b, ek_a_pub, false), ek_a_pub)
}

/// Inline storage-style encrypt (XChaCha20-Poly1305) so we don't need
/// private module access to `commands::util`.
fn storage_encrypt(plaintext: &[u8], key: &[u8; 32], aad: &[u8]) -> (Vec<u8>, Vec<u8>) {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::gen_nonce();
    let key_bytes = aead::Key::from_slice(key).unwrap();
    let ct = aead::seal(plaintext, Some(aad), &nonce, &key_bytes);
    (nonce.0.to_vec(), ct)
}

/// Inline storage-style decrypt.
fn storage_decrypt(ciphertext: &[u8], nonce: &[u8], key: &[u8; 32], aad: &[u8]) -> Vec<u8> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce_bytes = aead::Nonce::from_slice(nonce).unwrap();
    let key_bytes = aead::Key::from_slice(key).unwrap();
    aead::open(ciphertext, Some(aad), &nonce_bytes, &key_bytes).unwrap()
}

// ── Benchmarks ─────────────────────────────────────────────────────────

fn bench_dr_encrypt(c: &mut Criterion) {
    let plaintext = b"Hello, M2M! This is a benchmark message for measuring DR encrypt latency.";
    let aad = b"encrypted-message\x01";

    c.bench_function("dr_encrypt_100B_no_ratchet", |b| {
        b.iter(|| {
            let mut dr = make_dr();
            let _ = black_box(
                dr.encrypt(black_box(plaintext), black_box(aad), false),
            );
        })
    });
}

fn bench_dr_encrypt_with_ratchet(c: &mut Criterion) {
    let plaintext = b"Hello, M2M! This is a benchmark message for measuring DR encrypt latency.";
    let aad = b"encrypted-message\x01";

    c.bench_function("dr_encrypt_100B_with_ratchet", |b| {
        b.iter(|| {
            let mut local_dr = make_dr();
            let _ = black_box(
                local_dr.encrypt(black_box(plaintext), black_box(aad), true),
            );
        })
    });
}

fn bench_dr_decrypt(c: &mut Criterion) {
    let mut dr_sender = make_dr();
    let plaintext = b"Hello, M2M! This is a DR decrypt benchmark message.";
    let aad = b"encrypted-message\x01";

    // Pre-encrypt a message so we benchmark just the decrypt path
    let (ratchet_key, msg_num, nonce, ciphertext) =
        dr_sender.encrypt(plaintext, aad, false).unwrap();

    c.bench_function("dr_decrypt_100B_no_ratchet", |b| {
        b.iter(|| {
            let (mut dr, _) = make_dr_receiver();
            let _ = black_box(
                dr.decrypt(
                    black_box(&ciphertext),
                    black_box(&nonce),
                    black_box(aad),
                    black_box(msg_num),
                    black_box(ratchet_key.as_ref()),
                ),
            );
        })
    });
}

fn bench_dr_roundtrip(c: &mut Criterion) {
    let plaintext = b"Hello, M2M! DR roundtrip benchmark.";
    let aad = b"encrypted-message\x01";

    c.bench_function("dr_encrypt_decrypt_roundtrip", |b| {
        b.iter(|| {
            let mut sender = make_dr();
            let (mut receiver, _) = make_dr_receiver();
            let (rk, mn, nonce, ct) = sender.encrypt(black_box(plaintext), black_box(aad), false).unwrap();
            let decrypted = receiver.decrypt(
                black_box(&ct), black_box(&nonce), black_box(aad),
                black_box(mn), black_box(rk.as_ref()),
            ).unwrap();
            black_box(decrypted);
        })
    });
}

fn bench_storage_encrypt(c: &mut Criterion) {
    let key = m2m_lib::crypto::random_bytes(32);
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    let plaintext = b"Hello, M2M! This is a storage encryption benchmark message.";
    let aad = b"msg_store_v1";

    c.bench_function("storage_encrypt_100B", |b| {
        b.iter(|| {
            let _ = black_box(storage_encrypt(black_box(plaintext), black_box(&key_arr), black_box(aad)));
        })
    });
}

fn bench_storage_decrypt(c: &mut Criterion) {
    let key = m2m_lib::crypto::random_bytes(32);
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    let plaintext = b"Hello, M2M! This is a storage decryption benchmark message.";
    let aad = b"msg_store_v1";

    let (nonce, ciphertext) = storage_encrypt(plaintext, &key_arr, aad);

    c.bench_function("storage_decrypt_100B", |b| {
        b.iter(|| {
            let _ = black_box(storage_decrypt(
                black_box(&ciphertext), black_box(&nonce), black_box(&key_arr), black_box(aad),
            ));
        })
    });
}

fn bench_pad_message(c: &mut Criterion) {
    let small = b"hi";
    let medium = b"Hello, M2M! How are you today? This is a longer message.";
    let large = &b"A".repeat(5000);

    c.bench_function("pad_message_small_2B", |b| {
        b.iter(|| {
            let _ = black_box(pad_message_variable(black_box(small)));
        })
    });

    c.bench_function("pad_message_medium_80B", |b| {
        b.iter(|| {
            let _ = black_box(pad_message_variable(black_box(medium)));
        })
    });

    c.bench_function("pad_message_large_5KB", |b| {
        b.iter(|| {
            let _ = black_box(pad_message_variable(black_box(large)));
        })
    });
}

fn bench_unpad_message(c: &mut Criterion) {
    let small_padded = pad_message_variable(b"hi");
    let medium_padded = pad_message_variable(b"Hello, M2M! How are you today?");
    let large_padded = pad_message_variable(&b"A".repeat(5000));

    c.bench_function("unpad_message_small", |b| {
        b.iter(|| {
            let _ = black_box(unpad_message_variable(black_box(&small_padded)));
        })
    });

    c.bench_function("unpad_message_medium", |b| {
        b.iter(|| {
            let _ = black_box(unpad_message_variable(black_box(&medium_padded)));
        })
    });

    c.bench_function("unpad_message_large", |b| {
        b.iter(|| {
            let _ = black_box(unpad_message_variable(black_box(&large_padded)));
        })
    });
}

fn bench_keypair_generation(c: &mut Criterion) {
    c.bench_function("generate_x25519_keypair", |b| {
        b.iter(|| {
            let _ = black_box(X25519IdentityKeypair::generate());
        })
    });

    c.bench_function("generate_ephemeral_keypair", |b| {
        b.iter(|| {
            let _ = black_box(EphemeralKeypair::generate());
        })
    });
}

fn bench_x3dh_initiate(c: &mut Criterion) {
    let ik_a = X25519IdentityKeypair::generate();
    let ek_a = EphemeralKeypair::generate();
    let ik_b = X25519IdentityKeypair::generate();
    let spk_b = EphemeralKeypair::generate();
    let opk_b = EphemeralKeypair::generate();

    c.bench_function("x3dh_initiate", |b| {
        b.iter(|| {
            let _ = black_box(
                m2m_lib::crypto::x3dh_initiate(
                    black_box(&ik_a),
                    black_box(&ek_a),
                    black_box(&ik_b.public_key_bytes()),
                    black_box(&spk_b.public_key_bytes()),
                    black_box(Some(&opk_b.public_key_bytes())),
                ),
            );
        })
    });
}

fn bench_x3dh_respond(c: &mut Criterion) {
    let ik_a = X25519IdentityKeypair::generate();
    let ek_a = EphemeralKeypair::generate();
    let ik_b = X25519IdentityKeypair::generate();
    let spk_b = EphemeralKeypair::generate();
    let opk_b = EphemeralKeypair::generate();

    c.bench_function("x3dh_respond", |b| {
        b.iter(|| {
            let _ = black_box(
                m2m_lib::crypto::x3dh_respond(
                    black_box(&ik_b),
                    black_box(&spk_b),
                    black_box(Some(&opk_b)),
                    black_box(&ek_a.public_key_bytes()),
                    black_box(&ik_a.public_key_bytes()),
                ),
            );
        })
    });
}

criterion_group! {
    name = crypto_benches;
    config = Criterion::default().significance_level(0.02).sample_size(100);
    targets =
        bench_dr_encrypt,
        bench_dr_encrypt_with_ratchet,
        bench_dr_decrypt,
        bench_dr_roundtrip,
        bench_storage_encrypt,
        bench_storage_decrypt,
        bench_pad_message,
        bench_unpad_message,
        bench_keypair_generation,
        bench_x3dh_initiate,
        bench_x3dh_respond,
}
criterion_main!(crypto_benches);
