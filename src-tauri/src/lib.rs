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
pub mod crypto;
mod dht;
mod ephemeral_id;
mod hole_punch;
mod identity;
mod lan_discovery;
mod local_addr;
mod network;
mod port_mapping;
pub mod protocol;
mod reconnect;
mod relay;
mod secure_key;
mod session;
mod state;
mod storage;
mod stun;
mod tor;
mod window_security;

use std::sync::Arc;
use state::AppState;

use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::Manager;

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
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide to tray instead of quitting — app stays running for background messages
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // ── System Tray ──
            let show_item = MenuItemBuilder::with_id("show", "Show M2M").build(app)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let new_conv = MenuItemBuilder::with_id("new_conv", "New Conversation").build(app)?;
            let settings = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
            let quit_sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit M2M").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&separator)
                .item(&new_conv)
                .item(&settings)
                .item(&quit_sep)
                .item(&quit)
                .build()?;

            // Use the PNG icon (icon.ico in .ico format may not decode in all tray impls)
            let icon_bytes: &[u8] = include_bytes!("../icons/icon.png");
            let icon = tauri::image::Image::from_bytes(icon_bytes)
                .unwrap_or_else(|_| tauri::image::Image::new(&[], 1, 1));

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("M2M Secure Messenger")
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        "new_conv" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit("m2m://navigate", "hub");
                            }
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit("m2m://navigate", "settings");
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
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
            commands::files::cancel_file_transfer,
            commands::files::pause_file_transfer,
            commands::files::resume_file_transfer,
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
            commands::relay::get_relay_config,
            commands::relay::set_relay_config,
            commands::relay::get_relay_state,
            commands::discovery::get_discovery_config,
            commands::discovery::set_discovery_config,
            commands::discovery::get_discovered_peers,
            commands::discovery::connect_discovered_peer,
            commands::discovery::refresh_discovery,
            // Security
            commands::security::get_security_config,
            commands::security::set_security_config,
            commands::security::clear_clipboard,
            commands::vault::lock_vault,
            // Family
            commands::vault::list_family,
            commands::vault::add_family_member,
            commands::vault::remove_family_member,
            commands::vault::set_family_nickname,
            commands::vault::connect_family_member,
            commands::vault::update_family_member,
            // Export/Import
            commands::vault::export_identity,
            commands::vault::import_identity,
            // Reactions
            commands::chat::send_reaction,
            commands::chat::remove_reaction,
            // Read Receipts
            commands::chat::mark_messages_read,
            // Message features (self-destruct, edit, delete)
            commands::chat::send_message_with_timer,
            commands::chat::edit_message,
            commands::chat::delete_message,
            commands::chat::cleanup_expired_messages,
            // Mute
            commands::chat::mute_conversation,
            commands::chat::unmute_conversation,
            commands::chat::get_muted_conversations,
            // Reconnection
            commands::attempt_reconnect,
            commands::list_pending_reconnects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running M2M");
}
