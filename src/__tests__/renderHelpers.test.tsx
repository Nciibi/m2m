import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { useState } from "react";
import { errorMessage } from "../utils";
import { useNow } from "../hooks/useNow";
import SelfDestructTimer from "../components/chat/SelfDestructTimer";

describe("errorMessage", () => {
  // Tauri commands are `Result<T, String>`, so a rejected `invoke` rejects
  // with a *bare string*, not an Error. Reading `e.message` off that yields
  // `undefined` — which is what the old `catch (e: any)` + `e?.message` did.
  it("returns a bare string rejection as-is", () => {
    expect(errorMessage("Tauri command failed")).toBe("Tauri command failed");
  });

  it("falls back for an empty string rejection", () => {
    expect(errorMessage("", "fallback")).toBe("fallback");
  });

  it("uses the message of an Error", () => {
    expect(errorMessage(new Error("boom"))).toBe("boom");
  });

  it("prefers a non-empty message over the fallback", () => {
    expect(errorMessage(new Error(""), "fallback")).toBe("fallback");
  });

  it("unwraps a duck-typed { message } object", () => {
    expect(errorMessage({ message: "from object" })).toBe("from object");
  });

  it("falls back when the message is not a usable string", () => {
    expect(errorMessage({ message: 42 }, "fallback")).toBe("fallback");
    expect(errorMessage({ message: "" }, "fallback")).toBe("fallback");
  });

  it("falls back for null, undefined, numbers and arrays", () => {
    expect(errorMessage(null, "fallback")).toBe("fallback");
    expect(errorMessage(undefined, "fallback")).toBe("fallback");
    expect(errorMessage(500, "fallback")).toBe("fallback");
    expect(errorMessage([], "fallback")).toBe("fallback");
  });

  // The whole point of the helper: the caller can always render something.
  it("never returns an empty string", () => {
    for (const input of [null, undefined, "", 0, {}, [], new Error()]) {
      expect(errorMessage(input)).not.toBe("");
    }
  });
});

describe("useNow", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  function Probe({ intervalMs }: { intervalMs: number }) {
    return <span data-testid="now">{useNow(intervalMs)}</span>;
  }

  it("reads the clock without calling Date.now() during render", () => {
    // The hook must not invoke Date.now() on the render path, or StrictMode's
    // double render produces two different answers for one render.
    const spy = vi.spyOn(Date, "now");
    render(<Probe intervalMs={1000} />);
    const duringRender = spy.mock.calls.length;
    act(() => { vi.advanceTimersByTime(1000); });
    expect(spy.mock.calls.length).toBeGreaterThan(duringRender);
    spy.mockRestore();
  });

  it("ticks as time advances", () => {
    render(<Probe intervalMs={1000} />);
    expect(screen.getByTestId("now").textContent).toBe(String(Date.now()));
    act(() => { vi.advanceTimersByTime(5000); });
    expect(screen.getByTestId("now").textContent).toBe(String(Date.now()));
  });

  it("does not tick when the interval is disabled", () => {
    render(<Probe intervalMs={0} />);
    const initial = screen.getByTestId("now").textContent;
    act(() => { vi.advanceTimersByTime(60_000); });
    expect(screen.getByTestId("now").textContent).toBe(initial);
  });

  it("clears its interval on unmount", () => {
    const clearSpy = vi.spyOn(globalThis, "clearInterval");
    const { unmount } = render(<Probe intervalMs={1000} />);
    unmount();
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });
});

describe("SelfDestructTimer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const nowSecs = () => Math.floor(Date.now() / 1000);

  it("renders nothing for an already-elapsed message", () => {
    const { container } = render(<SelfDestructTimer expiresAt={nowSecs() - 5} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing for a message expiring now", () => {
    const { container } = render(<SelfDestructTimer expiresAt={nowSecs()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("formats the remaining time as m:ss", () => {
    render(<SelfDestructTimer expiresAt={nowSecs() + 65} />);
    // 65s → 1:05
    expect(screen.getByText(/1:05/)).toBeInTheDocument();
  });

  it("pads seconds below ten", () => {
    render(<SelfDestructTimer expiresAt={nowSecs() + 5} />);
    expect(screen.getByText(/0:05/)).toBeInTheDocument();
  });

  it("counts down each second", () => {
    render(<SelfDestructTimer expiresAt={nowSecs() + 3} />);
    expect(screen.getByText(/0:03/)).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(1000); });
    expect(screen.getByText(/0:02/)).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(1000); });
    expect(screen.getByText(/0:01/)).toBeInTheDocument();
  });

  it("unmounts itself once the message expires", () => {
    const { container } = render(<SelfDestructTimer expiresAt={nowSecs() + 2} />);
    expect(container).not.toBeEmptyDOMElement();
    act(() => { vi.advanceTimersByTime(2000); });
    expect(container).toBeEmptyDOMElement();
  });

  // Regression: the old implementation seeded `remaining` from `Date.now()`
  // during the initial render, so a changed `expiresAt` showed the previous
  // countdown until the next 1s tick.
  it("resets immediately when expiresAt changes, without waiting for a tick", () => {
    function Host() {
      const [expiresAt, setExpiresAt] = useState(nowSecs() + 300);
      return (
        <>
          <SelfDestructTimer expiresAt={expiresAt} />
          <button onClick={() => setExpiresAt(nowSecs() + 5)}>change</button>
        </>
      );
    }
    render(<Host />);
    expect(screen.getByText(/5:00/)).toBeInTheDocument();
    act(() => { screen.getByText("change").click(); });
    // Synchronous correction — no `vi.advanceTimersByTime` in between.
    expect(screen.getByText(/0:05/)).toBeInTheDocument();
  });

  it("stops ticking after expiry", () => {
    const clearSpy = vi.spyOn(globalThis, "clearInterval");
    render(<SelfDestructTimer expiresAt={nowSecs() + 1} />);
    act(() => { vi.advanceTimersByTime(1000); });
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });
});
