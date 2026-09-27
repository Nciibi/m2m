import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useFocusBlur } from "../hooks/useFocusBlur";

/**
 * Blur-on-focus-loss.
 *
 * A security control: it blurs the whole app when the window loses focus or is
 * hidden, so background capture tools and opportunistic shoulder-surfing only
 * ever see blur. It had **no tests at all**, while its sibling
 * `useIdleDetection` had six — which is backwards for a control that gates
 * whether a coercer standing behind you can read the screen.
 */
describe("useFocusBlur", () => {
  let visibility: string;

  beforeEach(() => {
    visibility = "visible";
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => visibility,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("stays off when disabled, even if the window loses focus", () => {
    const { result } = renderHook(() => useFocusBlur(false));
    expect(result.current).toBe(false);

    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    expect(result.current).toBe(false);
  });

  it("blurs on window blur and clears on focus when enabled", () => {
    const { result } = renderHook(() => useFocusBlur(true));

    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    expect(result.current).toBe(true);

    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(result.current).toBe(false);
  });

  it("blurs when the document is hidden and clears when shown", () => {
    const { result } = renderHook(() => useFocusBlur(true));

    act(() => {
      visibility = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(result.current).toBe(true);

    act(() => {
      visibility = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(result.current).toBe(false);
  });

  it("blurs immediately if the app starts while already hidden", () => {
    // A window restored from the tray, or a virtual-desktop switch, can mount
    // in the hidden state. Failing to blur there is exactly the gap the
    // feature exists to close.
    visibility = "hidden";
    const { result } = renderHook(() => useFocusBlur(true));
    expect(result.current).toBe(true);
  });

  it("removes its listeners on unmount", () => {
    const addSpy = vi.spyOn(window, "addEventListener");
    const removeSpy = vi.spyOn(window, "removeEventListener");
    const docAddSpy = vi.spyOn(document, "addEventListener");
    const docRemoveSpy = vi.spyOn(document, "removeEventListener");

    const { unmount } = renderHook(() => useFocusBlur(true));

    expect(addSpy).toHaveBeenCalledWith("blur", expect.any(Function));
    expect(addSpy).toHaveBeenCalledWith("focus", expect.any(Function));
    expect(docAddSpy).toHaveBeenCalledWith("visibilitychange", expect.any(Function));

    unmount();

    // A leaked listener keeps blurring (or, worse, keeps a closed component
    // alive) after teardown.
    expect(removeSpy).toHaveBeenCalledWith("blur", expect.any(Function));
    expect(removeSpy).toHaveBeenCalledWith("focus", expect.any(Function));
    expect(docRemoveSpy).toHaveBeenCalledWith("visibilitychange", expect.any(Function));
  });

  it("stops reacting to events after unmount", () => {
    const { result, unmount } = renderHook(() => useFocusBlur(true));
    unmount();
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    expect(result.current).toBe(false);
  });

  it("resets to unblurred when disabled while already blurred", () => {
    const { result, rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) => useFocusBlur(enabled),
      { initialProps: { enabled: true } },
    );

    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    expect(result.current).toBe(true);

    // Turning the setting off must take effect immediately — otherwise the
    // screen stays blurred after the user disables the protection.
    rerender({ enabled: false });
    expect(result.current).toBe(false);
  });
});
