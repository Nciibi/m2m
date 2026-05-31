//! M2M — Peer-to-peer encrypted messenger
//!
//! A privacy-first, metadata-minimizing secure communications tool
//! for journalists and high-risk users.
//!
//! Architecture:
//! - crypto: Ed25519 + X25519 + XChaCha20-Poly1305 via libsodium
//! - protocol: versioned, length-framed MessagePack packets
//! - network: TCP with timeouts, heartbeats, rate limiting
//! - identity: signed invites, fingerprint verification
//! - session: encrypted messaging with replay protection
//! - storage: SQLCipher encrypted local database
//! - commands: Tauri IPC bridge (no secrets exposed to UI)

mod commands;
mod crypto;
mod identity;
mod network;
mod protocol;
mod session;
mod state;
mod storage;

use std::sync::Arc;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize structured logging — no secrets in output
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("m2m=info")),
        )
        .with_target(false)
        .init();

    tracing::info!("M2M starting");

    // Determine data directory
    let data_dir = storage::data_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".m2m".to_string());

    let app_state = Arc::new(AppState::new(data_dir));

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::init_identity,
            commands::get_identity,
            commands::create_invite,
            commands::validate_invite,
            commands::start_listening,
            commands::connect_to_peer,
            commands::send_message,
            commands::get_connection_state,
            commands::verify_peer,
            commands::disconnect_peer,
            commands::list_peers,
            commands::load_messages,
            commands::send_file,
            commands::accept_file_transfer,
            commands::reject_file_transfer,
            commands::get_listen_address,
        ])
        .run(tauri::generate_context!())
        .expect("error while running M2M");
}
