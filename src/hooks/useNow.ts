import { useEffect, useState } from "react";

/**
 * A ticking wall-clock reading, for rendering relative times.
 *
 * `Date.now()` called directly during render is impure: it makes the output
 * non-deterministic, so a render can be thrown away and replayed with a
 * different answer, and React 18/19 in StrictMode will call the render body
 * twice. It is also *stale* — an expiry or "2 days left" label computed that
 * way never updates, because nothing re-renders the component as time passes.
 *
 * This hook holds the clock in state and re-reads it on an interval. Renders
 * stay pure, and the label actually counts down.
 *
 * `intervalMs` sets the granularity; callers that only need minute-level
 * accuracy should pass a large interval so a long list does not re-render
 * once a second.
 */
export function useNow(intervalMs = 1000): number {
  // Lazy initializer: read the clock once on mount, never during render.
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (intervalMs <= 0) return;
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);

  return now;
}
