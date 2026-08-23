import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useIdleDetection } from "../hooks/useIdleDetection";

describe("useIdleDetection", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not fire when disabled (timeoutSecs = 0)", () => {
    const onIdle = vi.fn();
    renderHook(() => useIdleDetection({ timeoutSecs: 0, onIdle }));

    act(() => {
      vi.advanceTimersByTime(10 * 60 * 1000);
    });
    expect(onIdle).not.toHaveBeenCalled();
  });

  it("fires onIdle after the configured timeout", () => {
    const onIdle = vi.fn();
    renderHook(() => useIdleDetection({ timeoutSecs: 5, onIdle }));

    act(() => {
      vi.advanceTimersByTime(4 * 1000);
    });
    expect(onIdle).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(1 * 1000);
    });
    expect(onIdle).toHaveBeenCalledTimes(1);
  });

  it("resets the timer on user activity", () => {
    const onIdle = vi.fn();
    renderHook(() => useIdleDetection({ timeoutSecs: 5, onIdle }));

    act(() => {
      vi.advanceTimersByTime(3 * 1000);
      window.dispatchEvent(new Event("mousemove"));
      vi.advanceTimersByTime(3 * 1000);
    });
    // 6s total elapsed but only 3s since last activity — must not fire.
    expect(onIdle).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(2 * 1000);
    });
    expect(onIdle).toHaveBeenCalledTimes(1);
  });

  it("resets the timer on keydown and click", () => {
    const onIdle = vi.fn();
    renderHook(() => useIdleDetection({ timeoutSecs: 4, onIdle }));

    act(() => {
      for (let i = 0; i < 5; i++) {
        vi.advanceTimersByTime(2 * 1000);
        window.dispatchEvent(i % 2 === 0 ? new Event("keydown") : new Event("click"));
      }
      vi.advanceTimersByTime(1 * 1000);
    });
    expect(onIdle).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(3 * 1000);
    });
    expect(onIdle).toHaveBeenCalledTimes(1);
  });

  it("uses the latest callback without re-arming", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(
      ({ onIdle }) => useIdleDetection({ timeoutSecs: 5, onIdle }),
      { initialProps: { onIdle: first } },
    );
    rerender({ onIdle: second });

    act(() => {
      vi.advanceTimersByTime(5 * 1000);
    });
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("clears the timer on unmount", () => {
    const onIdle = vi.fn();
    const { unmount } = renderHook(() =>
      useIdleDetection({ timeoutSecs: 5, onIdle }),
    );
    unmount();

    act(() => {
      vi.advanceTimersByTime(10 * 1000);
    });
    expect(onIdle).not.toHaveBeenCalled();
  });
});
