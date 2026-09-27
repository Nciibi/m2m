import { useNow } from "../../hooks/useNow";

/**
 * Countdown for a self-destructing message.
 *
 * The remaining time is *derived* from a ticking clock, not held in state and
 * updated by a `setInterval`. The previous implementation kept `remaining` in
 * state and pushed updates from an effect, which meant:
 *
 * - one extra render per tick, plus a setState inside an effect;
 * - a render where the countdown disagreed with `expiresAt` (a re-keyed
 *   message flashed the old value until the next tick);
 * - an interval per mounted bubble, none of them cleaned up early once the
 *   message had expired.
 *
 * Deriving it removes the effect and the interval entirely, so the displayed
 * value is correct on every render by construction.
 */
export default function SelfDestructTimer({ expiresAt }: { expiresAt: number }) {
  // Second-level granularity: the display resolution is whole seconds.
  const now = useNow(1000);
  const remaining = Math.max(0, expiresAt - Math.floor(now / 1000));

  if (remaining <= 0) return null;

  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return (
    <span className="msg-timer" title={`Self-destructs in ${mins}m ${secs}s`}>
      🔥 {mins}:{secs.toString().padStart(2, "0")}
    </span>
  );
}
