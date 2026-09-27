import { useEffect, useState } from "react";

/**
 * Blur-on-focus-loss (security roadmap §2).
 *
 * Blurs ALL app content whenever the window loses focus or is hidden,
 * defeating background capture tools and opportunistic shoulder-surfing
 * that rely on the window being visible but unfocused.
 *
 * Signals watched:
 * - `blur` / `focus` — window activation changes.
 * - `visibilitychange` — minimize / virtual-desktop switch / hide-to-tray.
 *
 * NOTE: a focused capture of an infected host still sees everything; this
 * layer covers the unfocused-window gap only. OFF by default — enabled via
 * SecurityConfig.blur_on_focus_loss.
 */
export function useFocusBlur(enabled: boolean): boolean {
  // Lazy initializer: the initial reading comes from an external source
  // (the document), so it is read once on mount rather than during render.
  const [blurred, setBlurred] = useState(
    () => typeof document !== "undefined" && document.visibilityState === "hidden",
  );

  useEffect(() => {
    if (!enabled) return;

    const onBlur = () => setBlurred(true);
    const onFocus = () => setBlurred(false);
    const onVisibility = () => setBlurred(document.visibilityState === "hidden");

    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [enabled]);

  // Derived rather than stored. The previous version called
  // `setBlurred(false)` inside the effect when `enabled` went false, which
  // costs an extra render pass and leaves a window — one render — where the
  // content is still un-blurred from a blur that happened before the feature
  // was turned off. Gating the return value is immediate and exact.
  return enabled && blurred;
}
