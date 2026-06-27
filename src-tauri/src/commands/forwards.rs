//! Manual port forwarding CRUD commands.
//!
//! These commands let the user manage port forwarding rules they've
//! configured in their router admin panel. Each forward becomes a
//! reliable candidate in invites.

use std::net::SocketAddr;
use std::sync::Arc;

use tauri::State;

use crate::state::{AppState, ManualForward};

/// List all user-configured manual port forwards.
#[tauri::command]
pub async fn list_manual_forwards(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ManualForward>, String> {
    let forwards = state.manual_forwards.read().await;
    Ok(forwards.clone())
}

/// Add a manual port forward.
///
/// The user configures this in their router admin panel. M2M stores it
/// so it can be included as a candidate in invites.
#[tauri::command]
pub async fn add_manual_forward(
    state: State<'_, Arc<AppState>>,
    public_addr: String,
    listen_port: u16,
    label: String,
) -> Result<Vec<ManualForward>, String> {
    // Validate the address parses as a SocketAddr.
    let _: SocketAddr = public_addr
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let mut forwards = state.manual_forwards.write().await;
    // Assign an order higher than any existing one.
    let order = forwards.iter().map(|f| f.order).max().unwrap_or(0) + 1;

    forwards.push(ManualForward {
        public_addr,
        listen_port,
        label,
        order,
    });
    // Keep sorted by order ascending.
    forwards.sort_by_key(|f| f.order);
    Ok(forwards.clone())
}

/// Remove a manual port forward by its order (index).
#[tauri::command]
pub async fn remove_manual_forward(
    state: State<'_, Arc<AppState>>,
    order: u32,
) -> Result<Vec<ManualForward>, String> {
    let mut forwards = state.manual_forwards.write().await;
    forwards.retain(|f| f.order != order);
    Ok(forwards.clone())
}

/// Reorder manual forwards. `orders` is the desired sequence of existing
/// order values, in the new priority order (first = highest priority).
#[tauri::command]
pub async fn reorder_manual_forwards(
    state: State<'_, Arc<AppState>>,
    orders: Vec<u32>,
) -> Result<Vec<ManualForward>, String> {
    let mut forwards = state.manual_forwards.write().await;
    // Build lookup.
    let old: std::collections::HashMap<u32, ManualForward> =
        forwards.drain(..).map(|f| (f.order, f)).collect();

    for (i, order_id) in orders.iter().enumerate() {
        if let Some(mut f) = old.get(order_id).cloned() {
            f.order = i as u32;
            forwards.push(f);
        }
    }
    // Also append any entries not in `orders` (edge case).
    for (_, f) in old {
        if !forwards.iter().any(|x| x.order == f.order) {
            forwards.push(f);
        }
    }
    forwards.sort_by_key(|f| f.order);
    Ok(forwards.clone())
}
