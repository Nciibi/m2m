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

interface IdleDetectionOptions {
  /** Idle timeout in seconds. 0 or negative = disabled. */
  timeoutSecs: number;
  /** Called when the user has been idle for `timeoutSecs`. */
  onIdle: () => void;
}

const ACTIVITY_EVENTS = ["mousemove", "mousedown", "keydown", "touchstart", "scroll", "wheel", "click"] as const;

export function useIdleDetection({ timeoutSecs, onIdle }: IdleDetectionOptions) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(onIdle);
  callbackRef.current = onIdle;

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
