//! Background storage maintenance: self-destruct expiry and the storage cap.
//!
//! # Why this exists
//!
//! Both policies were originally driven from the UI. `ChatView` ran a
//! `setInterval` invoking `cleanup_expired_messages` every 10s and again every
//! 60s, so expiry happened *only while a chat screen was mounted* — and this
//! app hides to the tray and keeps running, so for most of its life "auto-delete
//! after 24h" was a promise the app was not keeping. A self-destruct timer
//! enforced only while a particular window is focused is not a self-destruct
//! timer.
//!
//! The storage cap was worse for the same reason, and additionally because it
//! was enforced at exactly one write site (the inbound text handler). Outbound
//! sends and group messages wrote to the store with no ceiling at all, so
//! a store already near the cap could be pushed over it at leisure. Both gaps
//! are now closed by [`enforce_cap`], which every write path calls; the task
//! below is the backstop, not the primary mechanism.
//!
//! # What the task does
//!
//! Runs [`MessageStore::sweep`] on a fixed interval. `sweep` holds no policy of
//! its own — it is the code the tests exercise — so the only thing here that can
//! be wrong is the scheduling.
//!
//! The first tick fires immediately (`tokio::time::interval` semantics). The
//! store is opened lazily by `ensure_message_store`, so a tick that finds no
//! store is the normal pre-first-message state rather than an error; the expiry
//! half of the guarantee is already covered at `MessageStore::open`, which is
//! what covers the app-closed case.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::state::AppState;
use crate::storage::{EvictionReport, MessageStore, SweepOutcome};

/// How often the sweep runs.
///
/// Long enough not to matter on battery, short enough that a self-destruct
/// timer set in hours does not overshoot by much. The per-tick work is indexed
/// (`idx_messages_expires_at`, `idx_messages_oldest`) and does no destructive
/// work at all when nothing has expired and the store is under the cap.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Enforce the storage cap on a write path, and tell the user if it destroyed
/// anything.
///
/// **The single entry point for "the cap is enforced".** Every path that puts a
/// message on disk calls this — inbound text, inbound group frames, outbound
/// sends, outbound group sends. It was one path of four before, and the other
/// three were the gap: a cap that most write paths bypass is not a cap.
///
/// Takes the cap as a value so the caller reads `security_config` *before*
/// taking the store lock. The reverse nesting is a deadlock cycle, which is the
/// shape both of this codebase's historical deadlocks took.
///
/// `cap_bytes` comes from [`crate::state::SecurityConfig::effective_storage_cap`],
/// never from `storage_cap_bytes` directly — the raw field is 0 on a
/// default-constructed config, which is what a fresh install has, and reading
/// it raw would mean the cap is off exactly where it matters most.
pub fn enforce_cap(app_handle: &AppHandle, store: &MessageStore, cap_bytes: u64) {
    match store.enforce_storage_cap(cap_bytes) {
        Ok(Some(report)) => emit_storage_evicted(app_handle, &report),
        // Under the cap, or over it with nothing evictable (a cap smaller than
        // one message). No report, no event, nothing for the user to read.
        Ok(None) => {}
        // Not swallowed. A failed eviction means the ceiling is not being
        // enforced on this path, and the user has no other way to find out.
        Err(e) => tracing::error!(
            error = %e,
            "storage-cap enforcement failed — the cap is not being applied"
        ),
    }
}

/// Tell the user that stored history was permanently destroyed.
///
/// Emitted for a background pass as well as a write-path eviction. The point of
/// the cap is that the user learns history is gone; a user who never sends a
/// message would otherwise discover it by noticing missing messages.
pub fn emit_storage_evicted(app_handle: &AppHandle, report: &EvictionReport) {
    if let Err(e) = app_handle.emit(
        "m2m://storage-evicted",
        serde_json::json!({
            "messages_evicted": report.messages_evicted,
            "group_messages_evicted": report.group_messages_evicted,
            "bytes_freed": report.bytes_freed,
            "overrode_retention": report.overrode_retention,
        }),
    ) {
        tracing::warn!(error = %e, "could not emit storage-evicted");
    }
}

/// Start the maintenance task. Returns immediately; the task lives for the
/// process.
///
/// Called from `lib.rs` `setup`, after the security config has been restored —
/// the first tick reads the cap, and reading a default cap that the persisted
/// config is about to override would be a first tick enforcing the wrong number.
pub fn spawn(app_handle: AppHandle, state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        // A laptop suspended for a day must not come back and run the sweep
        // once per missed 15-minute period.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            sweep_once(&app_handle, &state).await;
        }
    });
}

/// Run one sweep pass: elapsed self-destruct timers first, then the cap.
pub async fn sweep_once(app_handle: &AppHandle, state: &Arc<AppState>) {
    // Read before the store lock; the guard is released at the end of the
    // statement. Reading the cap inside the `message_store` scope would nest
    // `security_config` under `message_store`.
    let cap = state.security_config.read().await.effective_storage_cap();

    // The store is opened lazily by `ensure_message_store` (called from ~9
    // command sites, never at startup), so "no store yet" is the normal state
    // of a freshly launched app.
    let outcome = {
        let ms = state.message_store.lock().await;
        let Some(store) = ms.as_ref() else {
            return;
        };
        match store.sweep(cap) {
            Ok(outcome) => outcome,
            // Logged, not swallowed: a sweep that can never succeed means
            // neither self-destruct nor the cap is being enforced.
            Err(e) => {
                tracing::error!(error = %e, "storage maintenance sweep failed");
                return;
            }
        }
    };

    // Reported outside the store lock — the event handler re-enters the
    // frontend, which may call `get_storage_usage` and take the lock just
    // released.
    report_sweep(&outcome, app_handle);
}

/// Announce a sweep, but only the eviction half.
///
/// Elapsed self-destruct timers are deliberately not an event. The user set
/// that timer, and re-notifying them on every sweep would train them to ignore
/// the one channel that carries the message that matters — that the cap took
/// messages a retention policy was protecting.
fn report_sweep(outcome: &SweepOutcome, app_handle: &AppHandle) {
    let report = &outcome.evicted;
    if report.messages_evicted == 0 && report.group_messages_evicted == 0 {
        return;
    }
    emit_storage_evicted(app_handle, report);
}
