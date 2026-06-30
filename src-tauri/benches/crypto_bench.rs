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
    let ik_a = X25519IdentityKeypair::generate();
    let ek_a = EphemeralKeypair::generate();
    let ek_b = EphemeralKeypair::generate();

    // X3DH output using random keys — not a valid X3DH, but produces valid DR state
    let x3dh = X3DHSessionKeys {
        root_key: *b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        chain_key: *b"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
    };
    DoubleRatchet::new(x3dh, ek_a, ek_b.public_key_bytes(), true)
}

/// Helper: create an X25519 keypair for storage encryption benchmarks.
fn make_storage_key() -> [u8; 32] {
    let buf = m2m_lib::crypto::random_bytes(32);
    let mut key = [0u8; 32];
    key.copy_from_slice(&buf);
    key
}

// ── Benchmarks ─────────────────────────────────────────────────────────

fn bench_dr_encrypt(c: &mut Criterion) {
    let mut dr = make_dr();
    let plaintext = b"Hello, M2M! This is a benchmark message for measuring DR encrypt latency.";
    let aad = b"encrypted-message\x01";

    c.bench_function("dr_encrypt_100B_no_ratchet", |b| {
        b.iter(|| {
            let _ = black_box(
                dr.encrypt(black_box(plaintext), black_box(aad), false),
            );
        })
    });
}

fn bench_dr_encrypt_with_ratchet(c: &mut Criterion) {
    let mut dr = make_dr();
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
    let mut dr_receiver = {
        let ek_b = EphemeralKeypair::generate();
        let x3dh = X3DHSessionKeys {
            root_key: *b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            chain_key: *b"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        };
        // The receiver's DR is the responder: its DH keypair is ek_b, peer's is ek_a's pub
        let ek_a_pub = EphemeralKeypair::generate().public_key_bytes();
        DoubleRatchet::new(x3dh, ek_b, ek_a_pub, false)
    };
    let plaintext = b"Hello, M2M! This is a benchmark message for measuring DR decrypt latency.";
    let aad = b"encrypted-message\x01";

    // Pre-encrypt a message so we can benchmark just the decrypt path
    let (ratchet_key, msg_num, nonce, ciphertext) = dr_sender.encrypt(plaintext, aad, false).unwrap();

    c.bench_function("dr_decrypt_100B_no_ratchet", |b| {
        b.iter(|| {
            let _ = black_box(
                dr_receiver.decrypt(
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

fn bench_storage_encrypt(c: &mut Criterion) {
    let key_bytes = make_storage_key();
    let key = m2m_lib::secure_key::StorageKey::from_bytes_for_test(&key_bytes);
    let plaintext = b"Hello, M2M! This is a benchmark message for storage encryption.";
    let aad = b"msg_store_v1";

    c.bench_function("storage_encrypt_100B", |b| {
        b.iter(|| {
            let _ = black_box(
                m2m_lib::commands::util::crypto_encrypt_storage(
                    black_box(plaintext),
                    black_box(&key),
                    black_box(aad),
                ),
            );
        })
    });
}

fn bench_storage_decrypt(c: &mut Criterion) {
    let key_bytes = make_storage_key();
    let key = m2m_lib::secure_key::StorageKey::from_bytes_for_test(&key_bytes);
    let plaintext = b"Hello, M2M! This is a benchmark message for storage decryption.";
    let aad = b"msg_store_v1";

    let (nonce, ciphertext) =
        m2m_lib::commands::util::crypto_encrypt_storage(plaintext, &key, aad).unwrap();

    c.bench_function("storage_decrypt_100B", |b| {
        b.iter(|| {
            let _ = black_box(
                m2m_lib::commands::util::crypto_decrypt_storage(
                    black_box(&ciphertext),
                    black_box(&nonce),
                    black_box(&key),
                    black_box(aad),
                ),
            );
        })
    });
}

fn bench_pad_message(c: &mut Criterion) {
    let small = b"hi";
    let medium = b"Hello, M2M! How are you today? This is a longer message for variable padding.";
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
    let small = pad_message_variable(b"hi");
    let medium = pad_message_variable(b"Hello, M2M! How are you today?");
    let large = pad_message_variable(&b"A".repeat(5000));

    c.bench_function("unpad_message_small", |b| {
        b.iter(|| {
            let _ = black_box(unpad_message_variable(black_box(&small)));
        })
    });

    c.bench_function("unpad_message_medium", |b| {
        b.iter(|| {
            let _ = black_box(unpad_message_variable(black_box(&medium)));
        })
    });

    c.bench_function("unpad_message_large", |b| {
        b.iter(|| {
            let _ = black_box(unpad_message_variable(black_box(&large)));
        })
    });
}

// ── Key generation benchmarks ──

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

    let x3dh = m2m_lib::crypto::x3dh_initiate(
        &ik_a,
        &ek_a,
        &ik_b.public_key_bytes(),
        &spk_b.public_key_bytes(),
        Some(&opk_b.public_key_bytes()),
    )
    .unwrap();

    c.bench_function("x3dh_respond", |b| {
        b.iter(|| {
            let _ = black_box(
                m2m_lib::crypto::x3dh_respond(
                    black_box(&ik_b),
                    black_box(&spk_b),
                    black_box(&ik_a.public_key_bytes()),
                    black_box(&ek_a.public_key_bytes()),
                    black_box(Some(&opk_b.secret_key_bytes())),
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
        bench_storage_encrypt,
        bench_storage_decrypt,
        bench_pad_message,
        bench_unpad_message,
        bench_keypair_generation,
        bench_x3dh_initiate,
        bench_x3dh_respond,
}
criterion_main!(crypto_benches);
