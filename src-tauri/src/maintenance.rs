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

use std::sync::atomic::Ordering;
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
    // `tauri::async_runtime::spawn`, not `tokio::spawn`. Both work from a
    // command handler, which is an async fn with a runtime already entered —
    // but this is called from `Builder::setup`, which is synchronous, and a bare
    // `tokio::spawn` there panics if no runtime is ambient. Tauri's spawner
    // targets its own global runtime and does not care. The rest of the crate
    // uses `tokio::spawn`; this is the one site where that would be a
    // difference in kind rather than style.
    tauri::async_runtime::spawn(async move {
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

// ─── Security deadlines: clipboard auto-clear and idle vault lock ───

/// Tick period for the security deadlines.
///
/// One second, because the shortest clipboard setting a user can pick is on the
/// order of seconds and the whole point is that the deadline is honoured without
/// the renderer. The per-tick cost is two relaxed atomic loads plus two
/// comparisons, so a 1s tick over a multi-day run is negligible next to the
/// 15-minute sweep.
pub const SECURITY_TICK: Duration = Duration::from_secs(1);

/// Current wall-clock time in unix seconds, or 0 if the clock is before the
/// epoch.
///
/// A pre-epoch clock yields 0, which the callers treat as "disarmed" — so the
/// failure mode is that the control does not fire, not that it fires constantly.
/// A monotonic source would be wrong here: these deadlines are compared against
/// `SystemTime` values supplied by command handlers, so both sides must use the
/// same clock.
pub fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Arm (or disarm) the clipboard auto-clear deadline.
///
/// `secs == 0` disarms. Called from the frontend when it copies something
/// sensitive, and re-called whenever the setting changes.
pub fn arm_clipboard_deadline(state: &Arc<AppState>, secs: u64) {
    let deadline = if secs == 0 {
        0
    } else {
        now_unix_secs().saturating_add(secs)
    };
    state
        .clipboard_clear_deadline
        .store(deadline, Ordering::Relaxed);
}

/// Push the idle-lock deadline out by `secs` from now.
///
/// Called on user activity. If activity stops — or stops being *reported*, which
/// is the case when the webview is gone — the deadline passes and the vault
/// locks.
pub fn note_activity(state: &Arc<AppState>, secs: u64) {
    if secs == 0 {
        state.idle_lock_deadline.store(0, Ordering::Relaxed);
        return;
    }
    state
        .idle_lock_deadline
        .store(now_unix_secs().saturating_add(secs), Ordering::Relaxed);
}

/// Disarm both deadlines. Used when the vault locks, so a stale deadline cannot
/// fire a second time against an already-locked vault.
pub fn disarm_security_deadlines(state: &Arc<AppState>) {
    state.clipboard_clear_deadline.store(0, Ordering::Relaxed);
    state.idle_lock_deadline.store(0, Ordering::Relaxed);
}

/// Start the security-deadline task. Returns immediately.
///
/// Separate from [`spawn`] because the intervals differ by three orders of
/// magnitude; one loop cannot serve both.
pub fn spawn_security_timers(app_handle: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(SECURITY_TICK);
        // A suspended laptop must not return and fire 86 400 catch-up ticks,
        // each of which would re-check a deadline that has long since passed.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let now = now_unix_secs();

            let clip = state.clipboard_clear_deadline.load(Ordering::Relaxed);
            if clip != 0 && now >= clip {
                state.clipboard_clear_deadline.store(0, Ordering::Relaxed);
                if let Err(e) = crate::commands::security::clear_clipboard().await {
                    // NOT swallowed. Failing to clear the clipboard is the whole
                    // failure this deadline exists to prevent, and a passphrase
                    // may be sitting in it.
                    tracing::error!(
                        error = %e,
                        "clipboard auto-clear FAILED — a sensitive value may still be on the clipboard"
                    );
                    let _ = app_handle.emit(
                        "m2m://security-error",
                        serde_json::json!({
                            "source": "clipboard_auto_clear",
                            "message": format!("{e}"),
                        }),
                    );
                } else {
                    tracing::info!("clipboard auto-cleared on deadline");
                }
            }

            let idle = state.idle_lock_deadline.load(Ordering::Relaxed);
            if idle != 0 && now >= idle {
                state.idle_lock_deadline.store(0, Ordering::Relaxed);
                tracing::info!("idle deadline reached — locking vault");
                // Re-arm the deadline to match the configured setting so the
                // vault keeps locking on every subsequent idle period rather
                // than once. If the setting is 0 the caller disarms it, which
                // means "do not auto-lock".
                let secs = state.security_config.read().await.idle_lock_secs;
                match crate::commands::vault::lock_vault_inner(&app_handle, &state).await {
                    Ok(()) => note_activity(&state, secs),
                    Err(e) => {
                        tracing::error!(error = %e, "idle auto-lock FAILED");
                        let _ = app_handle.emit(
                            "m2m://security-error",
                            serde_json::json!({
                                "source": "idle_auto_lock",
                                "message": format!("{e}"),
                            }),
                        );
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    /// `AppState::new` is cheap and touches no I/O, so the deadline helpers can
    /// be exercised without a Tauri runtime or an `AppHandle`.
    fn state() -> Arc<AppState> {
        Arc::new(AppState::new(String::new()))
    }

    #[test]
    fn clipboard_deadline_is_armed_in_the_future_and_disarmed_by_zero() {
        let s = state();
assert_eq!(
            s.clipboard_clear_deadline.load(Ordering::Relaxed),
            0,
            "a fresh state must have no deadline, or the task would fire at once"
        );

        arm_clipboard_deadline(&s, 30);
        let armed = s.clipboard_clear_deadline.load(Ordering::Relaxed);
        assert!(
            armed >= now_unix_secs() + 29,
            "30s must arm ~30s out, got {armed}"
        );

        // The documented disarm path. Getting this wrong is the "auto-clear
        // fires with nothing to clear" bug.
        arm_clipboard_deadline(&s, 0);
        assert_eq!(
            s.clipboard_clear_deadline.load(Ordering::Relaxed),
            0,
            "zero means disarmed, not 'fires now'"
        );
    }

    #[test]
    fn idle_deadline_follows_activity_and_zero_disarms() {
        let s = state();
        assert_eq!(s.idle_lock_deadline.load(Ordering::Relaxed), 0);

        note_activity(&s, 600);
        let first = s.idle_lock_deadline.load(Ordering::Relaxed);
        assert!(first >= now_unix_secs() + 599, "activity must push it out");

        // Every activity event re-arms from *now*, so it must not be additive:
        // a second call moves the deadline forward, never further into a
        // doubling horizon.
        note_activity(&s, 600);
        let second = s.idle_lock_deadline.load(Ordering::Relaxed);
        assert!(
            second - first < 5,
            "re-arming must replace the deadline, not add to it \
             (first={first}, second={second})"
        );

        // Idle lock switched off.
        note_activity(&s, 0);
        assert_eq!(s.idle_lock_deadline.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn disarm_clears_both_deadlines() {
        let s = state();
        arm_clipboard_deadline(&s, 60);
        note_activity(&s, 600);
        disarm_security_deadlines(&s);
        assert_eq!(s.clipboard_clear_deadline.load(Ordering::Relaxed), 0);
        assert_eq!(
            s.idle_lock_deadline.load(Ordering::Relaxed),
            0,
            "a stale idle deadline would re-lock an already-locked vault"
        );
    }

    /// The two deadlines are separate settings. Arming one must never disturb
    /// the other — sharing a field would mean copying a secret arms auto-lock.
    #[test]
    fn the_two_deadlines_are_independent() {
        let s = state();
        arm_clipboard_deadline(&s, 30);
        assert_eq!(
            s.idle_lock_deadline.load(Ordering::Relaxed),
            0,
            "arming the clipboard deadline must not arm idle lock"
        );
        note_activity(&s, 600);
        assert!(
            s.clipboard_clear_deadline.load(Ordering::Relaxed) != 0,
            "reporting activity must not disarm the clipboard deadline"
        );
    }

    #[test]
    fn a_deadline_in_the_past_is_visible_as_reached() {
        // The loop's own condition is `now >= deadline`. This asserts the
        // comparison a re-arm has to keep satisfying after the clock advances,
        // without sleeping for the configured duration.
        let s = state();
        arm_clipboard_deadline(&s, 1);
        let armed = s.clipboard_clear_deadline.load(Ordering::Relaxed);
        assert!(
            armed > 0,
            "1s must still arm a real deadline rather than reading as 0/disarmed"
        );
        // Simulate the clock having passed it.
        s.clipboard_clear_deadline
            .store(now_unix_secs() - 1, Ordering::Relaxed);
        assert!(now_unix_secs() >= s.clipboard_clear_deadline.load(Ordering::Relaxed));
    }
}
