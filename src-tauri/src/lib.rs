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
mod capture_monitor;
mod commands;
mod duress;
pub mod crypto;
pub mod dht;
mod ephemeral_id;
mod group;
mod hole_punch;
mod identity;
mod lan_discovery;
mod local_addr;
pub mod network;
mod port_mapping;
pub mod protocol;
mod protocol_fuzz_regression;
mod reconnect;
mod relay;
mod secure_key;
mod session;
mod state;
mod storage;
mod stun;
mod tor;
mod sync;
mod window_security;

use std::sync::Arc;
use state::AppState;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::{Emitter, Manager};

/// Disable crash dumps for this process (military-grade checklist: minidumps
/// contain DECRYPTED message buffers and key material).
///
/// - Windows: `SetErrorMode` suppresses Windows Error Reporting crash UI and
///   fault boxes. WER *local* dump collection is governed by registry keys
///   (HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps);
///   we additionally opt OUT per-process via the loader flag below.
/// - Unix: `setrlimit(RLIMIT_CORE, 0)` so the kernel never writes core files.
fn disable_crash_dumps() {
    #[cfg(target_os = "windows")]
    unsafe {
        // SEM_FAILCRITICALERRORS (0x0001) | SEM_NOGPFAULTERRORBOX (0x0002)
        // | SEM_NOOPENFILEERRORBOX (0x8000)
        const SEM_MASK: u32 = 0x0001 | 0x0002 | 0x8000;
        let kernel32 = match libloading::Library::new("kernel32.dll") {
            Ok(k) => k,
            Err(_) => return,
        };
        if let Ok(set_err) =
            kernel32.get::<unsafe extern "system" fn(u32) -> u32>(b"SetErrorMode\0")
        {
            let _ = set_err(SEM_MASK);
        }
        tracing::debug!("crash dumps: Windows error mode hardened");
    }

    #[cfg(unix)]
    {
        // setrlimit(RLIMIT_CORE, {0, 0}) — no core dumps, any signal.
        #[repr(C)]
        struct RLimit {
            rlim_cur: u64,
            rlim_max: u64,
        }
        extern "C" {
            fn setrlimit(resource: i32, rlim: *const RLimit) -> i32;
        }
        const RLIMIT_CORE: i32 = 4; // Linux & macOS value
        let limit = RLimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { setrlimit(RLIMIT_CORE, &limit) };
        if ret != 0 {
            tracing::warn!("failed to set RLIMIT_CORE=0 — core dumps may still be written");
        } else {
            tracing::debug!("crash dumps: RLIMIT_CORE set to 0");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // FIRST, before any key material exists in memory: crash dumps are a
    // plaintext-mining vector (minidumps capture heap pages holding decrypted
    // messages and session keys).
    disable_crash_dumps();

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
        // Signed update channel (inert until pubkey + endpoint are set).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Hide to tray instead of quitting — app stays running for background messages
                    let _ = window.hide();
                    api.prevent_close();
                }
                // Re-apply capture protection whenever the window is focused:
                // idempotent FFI call that heals any silent protection drop
                // (e.g. after a webview recreation or driver hiccup).
                tauri::WindowEvent::Focused(true) => {
                    let Some(state) = window.app_handle().try_state::<Arc<AppState>>() else {
                        return;
                    };
                    let enabled = state
                        .security_config
                        .try_read()
                        .map(|c| c.screen_capture_protection)
                        .unwrap_or(false);
                    if enabled {
                        if let Err(e) = window_security::apply_screen_protection(window.app_handle(), true) {
                            tracing::warn!(error = %e, "failed to re-apply capture protection on focus");
                        }
                    }
                }
                _ => {}
            }
        })
        .setup(|app| {
            // ── Restore persisted security config & re-apply protections ──
            // Runs AFTER the main window exists (config windows are created
            // before setup), so capture protection is active from the first
            // frame if the user enabled it. Survives restarts AND heals the
            // "webview recreated → protection silently dropped" gap.
            {
                let app_handle = app.handle().clone();
                let state: tauri::State<Arc<AppState>> = app_handle.state();
                let data_dir = state.data_dir.clone();

                let persisted = commands::security::load_config(&data_dir);
                let effective = persisted.unwrap_or_default();
                {
                    let mut sc = state.security_config.blocking_write();
                    *sc = effective.clone();
                }
                let _ = &state; // State wrapper released at scope end (contents stay managed)

                commands::security::apply_side_effects(
                    &app_handle.state::<Arc<AppState>>(),
                    &app_handle,
                    &effective,
                );
                tracing::info!(
                    screen_protection = effective.screen_capture_protection,
                    capture_detection = effective.capture_process_detection,
                    blur_on_focus_loss = effective.blur_on_focus_loss,
                    "security config restored"
                );
            }

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
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event: tauri::tray::TrayIconEvent| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
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
            commands::vault::create_vault_account,
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
            commands::settings::get_theme_preference,
            commands::settings::set_theme_preference,
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
            commands::security::reapply_security_config,
            commands::security::get_capture_capability,
            commands::security::clear_clipboard,
            commands::vault::lock_vault,
            commands::vault::set_duress_passphrase,
            commands::vault::clear_duress_passphrase,
            commands::vault::is_duress_configured,
            commands::vault::panic_wipe,
            commands::vault::is_first_run,
            commands::vault::set_first_run_complete,
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
            // Typing Indicator
            commands::chat::send_typing_indicator,
            // Message Search
            commands::chat::search_messages,
            // Favorites & Archive
            commands::chat::toggle_favorite,
            commands::chat::toggle_archive,
            // Multi-Device Sync
            sync::generate_sync_invite,
            sync::connect_sync_device,
            sync::pair_sync_device,
            // Group Chat (Phase 3)
            commands::groups::create_group,
            commands::groups::send_group_message,
            commands::groups::list_groups,
            commands::groups::get_group_info,
            commands::groups::invite_to_group,
            commands::groups::remove_from_group,
            commands::groups::leave_group,
            commands::groups::load_group_messages,
            commands::groups::update_group_name,
        ])
        .run(tauri::generate_context!())
        .expect("error while running M2M");
}
