import { useState, useEffect } from "react";

export default function SelfDestructTimer({ expiresAt }: { expiresAt: number }) {
  const [remaining, setRemaining] = useState(Math.max(0, expiresAt - Math.floor(Date.now() / 1000)));

  useEffect(() => {
    const timer = setInterval(() => {
      const r = Math.max(0, expiresAt - Math.floor(Date.now() / 1000));
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
