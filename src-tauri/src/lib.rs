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
//! - storage: Application-level encrypted local database
//! - stun: STUN client for NAT traversal (public IP discovery)
//! - tor: SOCKS5 proxy support for Tor onion routing
//! - commands: Tauri IPC bridge (no secrets exposed to UI)

mod candidate;
mod commands;
mod crypto;
mod hole_punch;
mod identity;
mod local_addr;
mod network;
mod port_mapping;
mod protocol;
mod session;
mod state;
mod storage;
mod stun;
mod tor;

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::vault::init_identity,
            commands::vault::get_identity,
            commands::vault::unlock_vault,
            commands::vault::get_vault_status,
            commands::network::create_invite,
            commands::network::validate_invite,
            commands::network::start_listening,
            commands::network::connect_to_peer,
            commands::network::get_connection_state,
            commands::network::verify_peer,
            commands::network::disconnect_peer,
            commands::network::list_peers,
            commands::network::get_listen_address,
            commands::chat::send_message,
            commands::chat::load_messages,
            commands::chat::list_conversations,
            commands::chat::rename_conversation,
            commands::chat::delete_conversation_cmd,
            commands::chat::set_conversation_retention,
            commands::chat::send_conversation_names,
            commands::chat::export_conversation,
            commands::files::send_file,
            commands::files::accept_file_transfer,
            commands::files::reject_file_transfer,
            commands::settings::discover_public_ip,
            commands::settings::get_stun_config,
            commands::settings::set_stun_servers,
            commands::settings::set_private_mode,
            commands::settings::check_connectivity,
            commands::settings::get_network_diagnostics,
            commands::settings::get_network_settings,
            commands::settings::set_tor_enabled,
            commands::forwards::list_manual_forwards,
            commands::forwards::add_manual_forward,
            commands::forwards::remove_manual_forward,
            commands::forwards::reorder_manual_forwards,
        ])
        .run(tauri::generate_context!())
        .expect("error while running M2M");
}
