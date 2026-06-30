use sodiumoxide::crypto::sign;
use sodiumoxide::crypto::hash::sha256;

fn fingerprint_from_public_key(public_key: &[u8; 32]) -> String {
    let hash = sha256::hash(public_key);
    let hex_str = hex::encode_upper(&hash.0[..16]);
    hex_str
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or("????"))
        .collect::<Vec<&str>>()
        .join(":")
}

fn main() {
    sodiumoxide::init().expect("sodiumoxide init failed");

    // Generate Ed25519 keypair
    let (pk, sk) = sign::gen_keypair();

    let public_key_hex = hex::encode(pk.0);
    let secret_key_hex = hex::encode(sk.0);
    let fingerprint = fingerprint_from_public_key(&pk.0);

    println!("=== M2M Identity Generated ===");
    println!();
    println!("Fingerprint:     {}", fingerprint);
    println!("Public Key:      {}", public_key_hex);
    println!("Private Key:     {}", secret_key_hex);
    println!();
    println!("⚠️  The private key is shown ONCE. Store it securely.");
    println!("   This is the key that controls your identity.");
    println!();
    println!("Passkey:         {}", &secret_key_hex[..32]);
    println!();
    println!("=== Save this somewhere safe ===");
}
