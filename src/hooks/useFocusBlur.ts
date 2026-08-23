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
  const [blurred, setBlurred] = useState(false);

  useEffect(() => {
    if (!enabled) {
      setBlurred(false);
      return;
    }

    const onBlur = () => setBlurred(true);
    const onFocus = () => setBlurred(false);
    const onVisibility = () => setBlurred(document.visibilityState === "hidden");

    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);

    // Initial state: if the app starts hidden, blur immediately.
    onVisibility();

    return () => {
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [enabled]);

  return blurred;
}
