/// M2M — Idle Detection Hook
///
/// Tracks user activity (mouse moves, keyboard, clicks, touch, scroll)
/// and calls a callback when the user has been idle for the specified
/// duration. Used for auto-locking the vault on inactivity.
///
/// Also listens for `visibilitychange` — if the user switches away
/// from the app and the idle timeout passes, the vault locks.
///
/// ## Usage
///
/// ```ts
/// useIdleDetection({
///   timeoutSecs: 300,  // 5 minutes
///   onIdle: () => invoke("lock_vault"),
/// });
/// ```

import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface IdleDetectionOptions {
  /** Idle timeout in seconds. 0 or negative = disabled. */
  timeoutSecs: number;
  /** Called when the user has been idle for `timeoutSecs`. */
  onIdle: () => void;
}

const ACTIVITY_EVENTS = ["mousemove", "mousedown", "keydown", "touchstart", "scroll", "wheel", "click"] as const;

/**
 * Push the idle deadline out on the Rust side too.
 *
 * `SecurityConfig::idle_lock_secs` had no reader in Rust at all — "✅ idle
 * vault lock" was this hook and nothing else. A lock that depends on the
 * webview's timer keeps running is not a lock: tab out, throttle the renderer,
 * or crash it, and the timer stops while the keys stay resident. The Rust task
 * in `maintenance::spawn_security_timers` enforces the same deadline on a 1s
 * tick, so activity has to be reported there too or it fires while the user is
 * actively typing.
 *
 * Best-effort by design: if this invoke fails the local timer still locks the
 * vault, which is the more important of the two.
 */
function reportActivityToRust(timeoutSecs: number): void {
  if (timeoutSecs <= 0) return;
  void invoke("note_activity", { secs: timeoutSecs }).catch(() => {
    /* the webview timer is still authoritative for the primary lock */
  });
}

export function useIdleDetection({ timeoutSecs, onIdle }: IdleDetectionOptions) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(onIdle);
  // Synced in an effect, not assigned during render: a render can be thrown
  // away and replayed, so a render-time write can leave this ref pointing at a
  // callback from a render that never committed.
  useEffect(() => {
    callbackRef.current = onIdle;
  });

  useEffect(() => {
    if (timeoutSecs <= 0) {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = null;
      return;
    }

    const resetTimer = () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        callbackRef.current();
      }, timeoutSecs * 1000);
      reportActivityToRust(timeoutSecs);
    };

    // Reset on any user activity
    for (const evt of ACTIVITY_EVENTS) {
      window.addEventListener(evt, resetTimer, { passive: true });
    }

    // Also reset on visibility change (tab becomes active again)
    const onVisibility = () => {
      if (document.visibilityState === "visible") {
        resetTimer();
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    // Initial start
    resetTimer();

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      for (const evt of ACTIVITY_EVENTS) {
        window.removeEventListener(evt, resetTimer);
      }
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [timeoutSecs]);
}
