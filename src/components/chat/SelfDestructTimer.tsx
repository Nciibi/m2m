import { useEffect, useState } from "react";

/**
 * Countdown for a self-destructing message.
 *
 * `remaining` is synced to `expiresAt` inside the effect, not seeded during
 * render. The old version did `useState(Date.now())`, which was impure (the
 * render body is re-invoked under StrictMode) and, worse, left the displayed
 * value stale whenever `expiresAt` changed: only the 1s interval tick would
 * correct it, so a re-keyed message flashed the old countdown.
 */
export default function SelfDestructTimer({ expiresAt }: { expiresAt: number }) {
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    const readRemaining = () =>
      Math.max(0, expiresAt - Math.floor(Date.now() / 1000));

    setRemaining(readRemaining());

    const timer = setInterval(() => {
      const r = readRemaining();
      setRemaining(r);
      if (r <= 0) clearInterval(timer);
    }, 1000);
    return () => clearInterval(timer);
  }, [expiresAt]);

  if (remaining <= 0) return null;

  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return (
    <span className="msg-timer" title={`Self-destructs in ${mins}m ${secs}s`}>
      🔥 {mins}:{secs.toString().padStart(2, "0")}
    </span>
  );
}
