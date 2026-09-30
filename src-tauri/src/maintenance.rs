//! Background storage maintenance: self-destruct expiry and the storage cap.
//!
//! # Why this exists
//!
//! Both policies were originally driven from the UI. `ChatView` ran a
//! `setInterval` that invoked `cleanup_expired_messages` every 10s and again
//! every 60s, so expiry happened *only while a chat screen was mounted* — and
//! this app hides to the tray and keeps running, so for most of its life
//! "auto-delete after 24h" was a promise the app was not keeping. A self-destruct
//! timer that is enforced only while a particular window is focused is not a
//! self-destruct timer.
//!
//! The storage cap is worse for the same reason, and additionally because the
//! cap was enforced at exactly one write site (the inbound text handler). Outbound
//! sends and group messages wrote to the store with no ceiling at all, so a
//! caller who could get the store near the cap could push it over at leisure.
//! Both gaps are now closed by [`crate::storage::MessageStore::enforce_storage_cap`],
//! which every write path calls; this task is the backstop, not the primary
//! mechanism.
//!
//! # What the task does
//!
//! Runs [`MessageStore::sweep`] on a fixed interval. `sweep` holds no policy of
//! its own — it is the same code the tests exercise — so the only thing here
//! that can be wrong is the scheduling.
//!
//! The first tick fires immediately (`tokio::time::interval` semantics), which
//! clears anything that expired while the app was closed. The store itself is
//! opened lazily by `ensure_message_store`, so a tick that finds no store is the
//! normal pre-first-message state, not an error: the expiry half of the
//! guarantee is already covered at `MessageStore::open`.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::state::AppState;
use crate::storage::SweepOutcome;

/// How often the sweep runs when nothing else prompts it.
///
/// Long enough not to matter on battery, short enough that a self-destruct
/// timer set to hours does not overshoot by much if the store is opened after
/// this module's start-up tick found no store. The per-tick work is indexed
/// (`idx_messages_expires_at`, `idx_messages_oldest`) and skips entirely when
/// the store has nothing expired and is under the cap.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Start the maintenance task. Returns immediately; the task lives for the
/// process.
///
/// Called from `lib.rs` `setup`, after the security config has been restored —
/// the first tick reads the cap, and reading a default cap that the user's
/// config is about to override would be a first tick that enforces the wrong
/// number.
pub fn spawn(app_handle: AppHandle, state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        // A laptop that suspends for a day must not come back and run the sweep
        // once per missed 15-minute period.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            sweep_once(&app_handle, &state).await;
        }
    });
}

/// Run one sweep pass and report whatever it destroyed.
pub async fn sweep_once(app_handle: &AppHandle, state: &Arc<AppState>) {
    // The cap is read *before* the store lock, and the read guard is released
    // by the statement. Reading it inside the `message_store` scope would nest
    // `security_config` under `message_store` — a pair with no documented
    // acquisition order, which is the shape of both deadlocks this codebase
    // has already had.
    let cap = state.security_config.read().await.effective_storage_cap();

    // The store is opened lazily by `ensure_message_store` (called from ~9
    // command sites, never at startup), so "no store yet" is the normal state
    // of a freshly launched app and is not an error. Anything that expired
    // while the app was closed was already destroyed at `MessageStore::open`.
    let outcome = {
        let ms = state.message_store.lock().await;
        let Some(store) = ms.as_ref() else {
            return;
        };
        match store.sweep(cap) {
            Ok(outcome) => outcome,
            Err(e) => {
                // Swallowed, not silent: a sweep that can never succeed means
                // neither self-destruct nor the cap is being enforced, and the
                // user has no other way to find that out.
                tracing::error!(error = %e, "storage maintenance sweep failed");
                return;
            }
        }
    };

    // The event is emitted outside the store lock — the UI may re-enter and
    // call `get_storage_usage`, which takes the very lock just released.
    report(&outcome, app_handle);
}

/// Tell the user about a sweep that destroyed history.
///
/// Only eviction is reported. Elapsed self-destruct timers are not an event:
/// the user set that timer, and re-notifying them every sweep would train them
/// to ignore the channel that carries the message that matters — that the cap
/// took messages a retention policy was protecting.
fn report(outcome: &SweepOutcome, app_handle: &AppHandle) {
    let report = &outcome.evicted;
    if report.messages_evicted == 0 && report.group_messages_evicted == 0 {
        return;
    }
    // Emitted for a *background* pass too, not just a write-path eviction: the
    // point of the cap is that the user is told history is gone, and a user who
    // never sends a message would otherwise only discover it by noticing
    // missing messages.
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
