//! File transfer commands.
//!
//! Handles initiating outgoing file transfers, accepting/rejecting
//! incoming ones, and the async chunk-sending loop with ACK tracking.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::protocol;
use crate::state::{AppState, IncomingFileTransfer, OutgoingFileTransfer, PeerConnection, TransferState};
use crate::storage;

use super::{FileTransferInfo, TransferProgressEvent};

// ─── Public Commands ───────────────────────────────────────────

/// Initiate a file transfer to a peer.
///
/// Reads the file in a streaming fashion to compute per-chunk SHA-256
/// hashes and the full-file hash, then sends the request and enqueues
/// the transfer for the chunk-sending loop.
#[tauri::command]
pub async fn send_file(
    #[allow(unused_variables)] app_handle: AppHandle,
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

    let transfer_id = uuid::Uuid::new_v4().to_string();

    // Determine adaptive chunk size from the peer's connection strategy.
    let chunk_size = {
        let conns = state.connections.read().await;
        let strategy = conns.get(&peer_key_hex)
            .and_then(|c| {
                let cg = c.try_lock().ok()?;
                Some(cg.strategy_name.clone())
            })
            .unwrap_or_default();
        compute_chunk_size(&strategy)
    };

    let total_chunks = (total_size as usize).div_ceil(chunk_size) as u32;

    // ── Streaming hash pass ──────────────────────────────────
    // Read the file once, computing both per-chunk hashes and the full-file hash.
    // Uses a fixed buffer sized to the adaptive chunk size — never loads the entire
    // file into RAM.
    let (file_hash, chunk_hashes) = compute_file_hashes(&file_path, total_chunks, chunk_size)
        .map_err(|e| format!("failed to read file: {e}"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // ── Store outgoing transfer state ────────────────────────
    {
        let mut outgoing = state.outgoing_transfers.write().await;
        outgoing.insert(transfer_id.clone(), OutgoingFileTransfer {
            transfer_id: transfer_id.clone(),
            peer_key_hex: peer_key_hex.clone(),
            file_path: path.to_path_buf(),
            filename: filename.clone(),
            total_size,
            total_chunks,
            file_hash,
            chunk_hashes: chunk_hashes.clone(),
            peer_protocol_version: 0, // will be detected from peer's ACK behavior
            state: TransferState::Pending,
            chunks_sent: 0,
            chunks_acked: 0,
            last_acked_index: 0,
            created_at: now,
            last_activity_at: now,
        });
    }

    // ── Send the file transfer request ───────────────────────
    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    drop(conns); // release read lock before send

    // Serialize chunk_hashes as Vec<Vec<u8>> for the wire (convert from Vec<[u8; 32]>)
    let wire_chunk_hashes: Vec<Vec<u8>> = chunk_hashes.iter()
        .map(|h| h.to_vec())
        .collect();

    let result = {
        let mut conn = conn_arc.lock().await;
        let PeerConnection { session, write_half, .. } = &mut *conn;
        session.send_file_request_v2(
            &mut *write_half,
            &transfer_id,
            &filename,
            total_size,
            total_chunks,
            file_hash.to_vec(),
            wire_chunk_hashes,
        ).await
    };

    match result {
        Ok(_) => {
            // Enqueue the transfer (won't start until peer accepts)
            {
                let mut queue = state.transfer_queue.write().await;
                let _ = queue.enqueue(transfer_id.clone());
            }

            // Persist initial transfer record
            {
                let ts = state.transfer_store.lock().await;
                if let Some(ref store) = *ts {
                    let _ = store.store_transfer(
                        &transfer_id, &peer_key_hex, &filename,
                        total_size, "sent", "pending", total_chunks,
                    );
                }
            }

            tracing::info!(
                transfer_id = %transfer_id,
                filename = %filename,
                size = total_size,
                chunks = total_chunks,
                "file transfer request sent"
            );
            Ok(FileTransferInfo {
                transfer_id,
                filename,
                total_size,
                peer_key_hex,
            })
        }
        Err(e) => {
            // Clean up state on send failure
            state.outgoing_transfers.write().await.remove(&transfer_id);
            Err(format!("failed to send file request: {e}"))
        }
    }
}

/// Accept an incoming file transfer.
#[tauri::command]
pub async fn accept_file_transfer(
    #[allow(unused_variables)] app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    transfer_id: String,
    save_dir: String,
) -> Result<(), String> {
    // Store the save_dir and update state
    {
        let transfers = state.incoming_transfers.read().await;
        if let Some(_t) = transfers.get(&transfer_id) {
            // Already registered from the request handler. Patch save_path.
            drop(transfers);
            let mut w = state.incoming_transfers.write().await;
            if let Some(t) = w.get_mut(&transfer_id) {
                t.save_path = std::path::PathBuf::from(&save_dir);
                t.state = TransferState::Transferring;
            }
        } else {
            drop(transfers);
            // Create placeholder entry
            let mut w = state.incoming_transfers.write().await;
            w.entry(transfer_id.clone()).or_insert_with(|| {
                let save_path = std::path::PathBuf::from(&save_dir);
                IncomingFileTransfer {
                    transfer_id: transfer_id.clone(),
                    peer_key_hex: peer_key_hex.clone(),
                    filename: String::new(),
                    total_size: 0,
                    total_chunks: 0,
                    file_hash: Vec::new(),
                    chunk_hashes: Vec::new(),
                    peer_protocol_version: 0,
                    save_path,
                    temp_file: None,
                    temp_path: None,
                    chunks_received: 0,
                    bytes_received: 0,
                    chunks_bitmask: Vec::new(),
                    state: TransferState::Transferring,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    error: None,
                }
            });
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

    // Clean up local state
    state.incoming_transfers.write().await.remove(&transfer_id);

    Ok(())
}

/// Cancel an in-progress file transfer (send or receive).
/// Sends a cancel packet to the peer and cleans up local state.
#[tauri::command]
#[allow(dead_code)]
pub async fn cancel_file_transfer(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    transfer_id: String,
) -> Result<(), String> {
    // Send cancel to peer if connected
    let conns = state.connections.read().await;
    if let Some(conn_arc) = conns.get(&peer_key_hex) {
        let mut conn = conn_arc.lock().await;
        let PeerConnection { session, write_half, .. } = &mut *conn;
        let _ = session.send_file_cancel(&mut *write_half, &transfer_id).await;
    }
    drop(conns);

    // Clean up outgoing state
    {
        let mut outgoing = state.outgoing_transfers.write().await;
        if let Some(t) = outgoing.get_mut(&transfer_id) {
            t.state = TransferState::Cancelled;
        }
        outgoing.remove(&transfer_id);
    }

    // Clean up incoming state (temp file removal)
    {
        let mut incoming = state.incoming_transfers.write().await;
        if let Some(t) = incoming.remove(&transfer_id) {
            drop(t.temp_file);
            if let Some(ref path) = t.temp_path {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    // Remove from queue
    {
        let mut queue = state.transfer_queue.write().await;
        queue.queue.retain(|id| id != &transfer_id);
        queue.active.remove(&transfer_id);
    }

    // Notify frontend
    let _ = app_handle.emit("m2m://transfer-cancelled", serde_json::json!({
        "transfer_id": transfer_id,
    }));

    Ok(())
}

// ─── Queue-Aware Transfer Lifecycle ────────────────────────────

/// Try to start an outgoing transfer. Called when the peer accepts.
/// Checks queue concurrency limits; if a slot is available, runs
/// the chunk sender inline (on a spawned task). Otherwise leaves
/// it queued for later.
pub(super) fn try_start_outgoing_transfer(
    app_handle: AppHandle,
    state: Arc<AppState>,
    peer_key_hex: String,
    transfer_id: String,
) {
    tokio::spawn(async move {
        let filepath = {
            let outgoing = state.outgoing_transfers.read().await;
            outgoing.get(&transfer_id).map(|t| t.file_path.to_string_lossy().to_string())
        };

        let filepath = match filepath {
            Some(fp) => fp,
            None => {
                tracing::warn!(transfer_id = %transfer_id, "outgoing transfer not found");
                return;
            }
        };

        // Check queue slot
        let should_start = {
            let mut queue = state.transfer_queue.write().await;
            if queue.active.contains(&transfer_id) {
                true
            } else if queue.can_start() {
                queue.active.insert(transfer_id.clone());
                true
            } else {
                tracing::info!(
                    transfer_id = %transfer_id,
                    active = queue.active.len(),
                    max = queue.max_concurrent,
                    "transfer queued, waiting for slot"
                );
                false
            }
        };

        if !should_start { return; }

        // Mark transferring + persist
        {
            let mut outgoing = state.outgoing_transfers.write().await;
            if let Some(t) = outgoing.get_mut(&transfer_id) {
                t.state = TransferState::Transferring;
            }
        }
        {
            let ts = state.transfer_store.lock().await;
            if let Some(ref store) = *ts {
                let _ = store.update_state(&transfer_id, "transferring", None, None);
            }
        }

        // ── Run chunk sender ──
        let result = send_file_chunks_inner(
            &app_handle, &state, &peer_key_hex, &transfer_id, &filepath,
        ).await;

        // ── Handle result and chain next queued ──
        finish_and_chain(
            &app_handle, &state, &transfer_id, result,
        ).await;
    });
}

/// After a transfer finishes, update state, persist, emit events,
/// remove from active, and start the next queued transfer.
async fn finish_and_chain(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    transfer_id: &str,
    result: Result<(), String>,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match &result {
        Ok(()) => {
            let mut outgoing = state.outgoing_transfers.write().await;
            if let Some(t) = outgoing.get_mut(transfer_id) {
                t.state = TransferState::Completed;
            }

            let ts = state.transfer_store.lock().await;
            if let Some(ref store) = *ts {
                let _ = store.update_state(transfer_id, "completed", Some(now as i64), None);
            }

            let _ = app_handle.emit("m2m://transfer-completed", serde_json::json!({
                "transfer_id": transfer_id,
            }));
        }
        Err(e) => {
            let mut outgoing = state.outgoing_transfers.write().await;
            if let Some(t) = outgoing.get_mut(transfer_id) {
                t.state = TransferState::Failed;
            }

            let ts = state.transfer_store.lock().await;
            if let Some(ref store) = *ts {
                let _ = store.update_state(transfer_id, "failed", Some(now as i64), Some(e));
            }

            let _ = app_handle.emit("m2m://transfer-error", serde_json::json!({
                "transfer_id": transfer_id,
                "error": e,
            }));
        }
    }

    // Dequeue next transfer and recurse
    let next_id = {
        let mut queue = state.transfer_queue.write().await;
        queue.active.remove(transfer_id);
        queue.dequeue()
    };

    if let Some(next_tid) = next_id {
        let (next_fp, next_pk) = {
            let outgoing = state.outgoing_transfers.read().await;
            match outgoing.get(&next_tid) {
                Some(t) => (t.file_path.to_string_lossy().to_string(), t.peer_key_hex.clone()),
                None => return,
            }
        };

        tracing::info!(transfer_id = %next_tid, "starting next queued transfer after previous finished");

        let result = send_file_chunks_inner(
            app_handle, state, &next_pk, &next_tid, &next_fp,
        ).await;

        // Recurse for the chain
        Box::pin(finish_and_chain(app_handle, state, &next_tid, result)).await;
    }
}

/// Inner implementation of the chunk-sending loop. Returns Ok/Err so
/// the caller handles state transitions consistently.
async fn send_file_chunks_inner(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    peer_key_hex: &str,
    transfer_id: &str,
    file_path: &str,
) -> Result<(), String> {
    let total_chunks: u32;
    let chunk_hashes: Vec<[u8; 32]>;
    let is_v2_protocol: bool;

    // Read transfer metadata from state, and determine adaptive chunk size
    let chunk_size = {
        let outgoing = state.outgoing_transfers.read().await;
        let t = outgoing.get(transfer_id)
            .ok_or("transfer not found in state")?;
        total_chunks = t.total_chunks;
        chunk_hashes = t.chunk_hashes.clone();
        is_v2_protocol = t.peer_protocol_version >= 2;

        // Look up peer's connection strategy for adaptive chunk size
        let conns = state.connections.read().await;
        let strategy = conns.get(&t.peer_key_hex)
            .and_then(|c| {
                let cg = c.try_lock().ok()?;
                Some(cg.strategy_name.clone())
            })
            .unwrap_or_default();
        drop(conns);
        compute_chunk_size(&strategy)
    };

    // Throttle: emit progress every N chunks to avoid flooding the frontend
    let progress_interval = std::cmp::max(1, total_chunks / 20);

    // ACK timeout: if we don't get an ACK within 10s, retry the chunk
    let ack_timeout = std::time::Duration::from_secs(10);
    let max_retries: u32 = 3;

    for chunk_index in 0..total_chunks {
        let mut retries = 0;
        let chunk_success = loop {
            // Read this chunk from disk (seeking each time to avoid holding the file open)
            let mut buf = vec![0u8; chunk_size];
            let mut f = std::fs::File::open(file_path)
                .map_err(|e| format!("failed to re-open file: {e}"))?;

            use std::io::{Read, Seek};
            let offset = (chunk_index as u64) * (chunk_size as u64);
            f.seek(std::io::SeekFrom::Start(offset))
                .map_err(|e| format!("seek failed: {e}"))?;
            let n = f.read(&mut buf)
                .map_err(|e| format!("read failed: {e}"))?;
            buf.truncate(n);

            // Verify chunk hash (integrity check against pre-computed hash)
            let expected_hash = chunk_hashes.get(chunk_index as usize)
                .cloned()
                .unwrap_or([0u8; 32]);
            let actual_hash = sodiumoxide::crypto::hash::sha256::hash(&buf);
            if actual_hash.0 != expected_hash {
                return Err(format!(
                    "chunk {} hash mismatch before send — file may have changed on disk",
                    chunk_index
                ));
            }

            // Send the chunk
            {
                let conns = state.connections.read().await;
                let conn_arc = conns.get(peer_key_hex)
                    .ok_or("peer disconnected during transfer")?.clone();
                let mut conn = conn_arc.lock().await;
                let PeerConnection { session, write_half, .. } = &mut *conn;
                session.send_file_chunk(
                    &mut *write_half,
                    transfer_id, chunk_index, buf, expected_hash.to_vec(),
                ).await.map_err(|e| format!("chunk send failed: {e}"))?;
            }

            // Update chunks_sent
            {
                let mut outgoing = state.outgoing_transfers.write().await;
                if let Some(t) = outgoing.get_mut(transfer_id) {
                    t.chunks_sent += 1;
                    t.last_activity_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            }

            // If v2 protocol, wait for ACK with retry
            if is_v2_protocol {
                let ack_received = wait_for_ack(state, transfer_id, chunk_index, ack_timeout).await;

                if ack_received {
                    break true;
                } else {
                    retries += 1;
                    if retries >= max_retries {
                        return Err(format!(
                            "chunk {} not acked after {} retries",
                            chunk_index, max_retries
                        ));
                    }
                    tracing::warn!(
                        transfer_id = %transfer_id,
                        chunk = chunk_index,
                        retry = retries,
                        "chunk not acked, retrying"
                    );
                    continue;
                }
            } else {
                // v1 protocol: blind send, no ACK
                break true;
            }
        };

        if !chunk_success {
            return Err(format!("failed to send chunk {}", chunk_index));
        }

        // Emit progress periodically
        if chunk_index % progress_interval == 0 || chunk_index == total_chunks - 1 {
            emit_progress(app_handle, state, transfer_id).await;
        }
    }

    // ── All chunks sent — send FileTransferComplete ──
    {
        let conns = state.connections.read().await;
        let conn_arc = conns.get(peer_key_hex)
            .ok_or("peer disconnected during transfer")?.clone();
        let mut conn = conn_arc.lock().await;
        let PeerConnection { session, write_half, .. } = &mut *conn;
        session.send_file_complete(&mut *write_half, transfer_id)
            .await.map_err(|e| format!("complete send failed: {e}"))?;
    }

    Ok(())
}

/// Wait for an ACK for the given chunk index. Returns true if acked, false if timeout.
async fn wait_for_ack(
    state: &Arc<AppState>,
    transfer_id: &str,
    chunk_index: u32,
    timeout: std::time::Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    let poll_interval = std::time::Duration::from_millis(50);

    while tokio::time::Instant::now() < deadline {
        {
            let outgoing = state.outgoing_transfers.read().await;
            if let Some(t) = outgoing.get(transfer_id) {
                if t.chunks_acked > chunk_index
                    || (t.chunks_acked > 0 && t.last_acked_index >= chunk_index)
                {
                    return true;
                }
            }
        }
        tokio::time::sleep(poll_interval).await;
    }

    false
}

/// Emit a transfer progress event to the frontend.
async fn emit_progress(app_handle: &AppHandle, state: &Arc<AppState>, transfer_id: &str) {
    let outgoing = state.outgoing_transfers.read().await;
    if let Some(t) = outgoing.get(transfer_id) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let elapsed = now.saturating_sub(t.created_at).max(1);
        let bytes_completed = t.chunks_acked as u64 * protocol::MAX_FILE_CHUNK_SIZE as u64;
        let speed = bytes_completed / elapsed; // bytes/sec
        let remaining = t.total_size.saturating_sub(bytes_completed);
        let eta = if speed > 0 { remaining / speed } else { 0 };

        let _ = app_handle.emit("m2m://transfer-progress", TransferProgressEvent {
            transfer_id: transfer_id.to_string(),
            peer_key_hex: t.peer_key_hex.clone(),
            filename: t.filename.clone(),
            total_size: t.total_size,
            bytes_transferred: bytes_completed.min(t.total_size),
            chunks_completed: t.chunks_acked,
            chunks_total: t.total_chunks,
            state: t.state.to_string(),
            speed_bytes_per_sec: speed,
            estimated_remaining_secs: eta,
        });
    }
}

// ─── Adaptive Chunk Size ──────────────────────────────────────

/// Compute the best chunk size for the given connection strategy.
///
/// - host, ipv6, port-mapped (local/fast paths): 512 KiB
/// - srflx, prflx (internet hole-punch): 256 KiB
/// - relay (high-latency): 128 KiB
/// - default: 256 KiB
pub(super) fn compute_chunk_size(strategy_name: &str) -> usize {
    match strategy_name {
        "host" | "ipv6" | "port-mapped" => 512 * 1024,
        "srflx" | "prflx" => 256 * 1024,
        "relay" => 128 * 1024,
        _ => 256 * 1024,
    }
}

// ─── Hash Computation (Streaming) ──────────────────────────────

/// Compute per-chunk SHA-256 hashes and the full-file SHA-256 hash in a
/// single streaming pass. Uses a fixed 256 KiB buffer — never loads the
/// entire file into RAM.
fn compute_file_hashes(
    file_path: &str,
    total_chunks: u32,
) -> Result<([u8; 32], Vec<[u8; 32]>), String> {
    use std::io::Read;
    use sodiumoxide::crypto::hash::sha256;

    let mut file = std::fs::File::open(file_path)
        .map_err(|e| format!("failed to open file: {e}"))?;

    let mut full_hasher = sha256::State::new();
    let mut chunk_hashes = Vec::with_capacity(total_chunks as usize);
    let mut buf = vec![0u8; protocol::MAX_FILE_CHUNK_SIZE];

    loop {
        let n = file.read(&mut buf)
            .map_err(|e| format!("read error during hash computation: {e}"))?;
        if n == 0 { break; }

        let chunk = &buf[..n];
        full_hasher.update(chunk);

        let chunk_hash = sha256::hash(chunk);
        chunk_hashes.push(chunk_hash.0);
    }

    let full_hash = full_hasher.finalize();
    Ok((full_hash.0, chunk_hashes))
}
