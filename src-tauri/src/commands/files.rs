//! File transfer commands.
//!
//! Handles initiating outgoing file transfers, accepting/rejecting
//! incoming ones, and the async chunk-sending loop.

use std::sync::Arc;

use tauri::State;

use crate::protocol;
use crate::state::{AppState, PeerConnection};

use super::util;
use super::FileTransferInfo;

/// Initiate a file transfer to a peer.
#[tauri::command]
pub async fn send_file(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    file_path: String,
) -> Result<FileTransferInfo, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err("file not found".to_string());
    }

    let metadata = std::fs::metadata(path).map_err(|e| format!("cannot read file: {e}"))?;
    let total_size = metadata.len();
    let filename = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let file_data = std::fs::read(path).map_err(|e| format!("failed to read file: {e}"))?;
    let file_hash = sodiumoxide::crypto::hash::sha256::hash(&file_data);
    let total_chunks = ((total_size as usize + protocol::MAX_FILE_CHUNK_SIZE - 1) / protocol::MAX_FILE_CHUNK_SIZE) as u32;
    let transfer_id = uuid::Uuid::new_v4().to_string();

    // Store for later chunk sending
    state.outgoing_transfers.write().await.insert(transfer_id.clone(), file_path);

    // Send the request
    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;
    session.send_file_request(
        &mut *write_half,
        &transfer_id, &filename, total_size, total_chunks, file_hash.0.to_vec(),
    ).await.map_err(|e| format!("failed to send file request: {e}"))?;

    Ok(FileTransferInfo {
        transfer_id,
        filename,
        total_size,
        peer_key_hex,
    })
}

/// Accept an incoming file transfer.
#[tauri::command]
pub async fn accept_file_transfer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    transfer_id: String,
    save_dir: String,
) -> Result<(), String> {
    // Store the save_dir into the incoming transfer state so the
    // FileTransferComplete handler knows where to write the reassembled file.
    {
        let transfers = state.incoming_transfers.read().await;
        if !transfers.contains_key(&transfer_id) {
            // The transfer metadata arrives via a FileTransferRequest event.
            // If it hasn't been stored yet we create a placeholder entry here;
            // the real metadata (filename, hash, etc.) will be patched in by
            // the receive loop.
            drop(transfers);
            let mut w = state.incoming_transfers.write().await;
            w.entry(transfer_id.clone()).or_insert_with(|| {
                let save_path = std::path::PathBuf::from(&save_dir);
                crate::state::IncomingFileTransfer {
                    filename: String::new(),
                    total_size: 0,
                    total_chunks: 0,
                    file_hash: Vec::new(),
                    save_path,
                    temp_file: None,
                    temp_path: None,
                    chunks_received: 0,
                    chunks_bitmask: Vec::new(),
                }
            });
        } else {
            drop(transfers);
            // Patch save_path into existing entry
            let mut w = state.incoming_transfers.write().await;
            if let Some(t) = w.get_mut(&transfer_id) {
                t.save_path = std::path::PathBuf::from(&save_dir);
            }
        }
    }

    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;

    session.send_file_accept(&mut *write_half, &transfer_id)
        .await.map_err(|e| format!("failed to send accept: {e}"))?;

    Ok(())
}

/// Reject an incoming file transfer.
#[tauri::command]
pub async fn reject_file_transfer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    transfer_id: String,
) -> Result<(), String> {
    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;

    session.send_file_reject(&mut *write_half, &transfer_id)
        .await.map_err(|e| format!("failed to send reject: {e}"))?;

    Ok(())
}

/// Send file chunks to a peer after they've accepted the transfer.
async fn send_file_chunks(
    state: Arc<AppState>,
    peer_key_hex: &str,
    transfer_id: &str,
    file_path: &str,
) -> Result<(), String> {
    let file_data = std::fs::read(file_path).map_err(|e| format!("read failed: {e}"))?;
    let chunks: Vec<&[u8]> = file_data.chunks(protocol::MAX_FILE_CHUNK_SIZE).collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_hash = sodiumoxide::crypto::hash::sha256::hash(chunk);
        let conns = state.connections.read().await;
        let conn_arc = conns.get(peer_key_hex)
            .ok_or("peer disconnected during transfer")?.clone();
        let mut conn = conn_arc.lock().await;
        let PeerConnection { session, write_half, .. } = &mut *conn;
        session.send_file_chunk(
            &mut *write_half,
            transfer_id, i as u32, chunk.to_vec(), chunk_hash.0.to_vec(),
        ).await.map_err(|e| format!("chunk send failed: {e}"))?;
    }

    // Send completion
    let conns = state.connections.read().await;
    let conn_arc = conns.get(peer_key_hex)
        .ok_or("peer disconnected during transfer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;
    session.send_file_complete(&mut *write_half, transfer_id)
        .await.map_err(|e| format!("complete send failed: {e}"))?;

    // Clean up
    state.outgoing_transfers.write().await.remove(transfer_id);
    Ok(())
}
